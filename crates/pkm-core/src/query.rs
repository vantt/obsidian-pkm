//! Truy vấn index: các phép của bộ tool trên bảng `edges`, bằng recursive CTE.
//!
//! Đặc tả: `scripts/kg/query.py` ở sha ghim. Câu SQL (recursive CTE, điều kiện lọc, thứ tự) chép nguyên từ Python
//! nên SQLite trả cùng dòng theo cùng thứ tự; phần Rust chỉ ghép câu và đọc kết quả. Kết quả có dạng `dict` của
//! Python được trả thành object JSON với khoá theo đúng thứ tự Python chèn.
//!
//! Lọc chung cho mọi phép đi trên cạnh ([`Filter`]):
//! - `rels`: chỉ các quan hệ này (tên thuận, như ghi trong frontmatter hoặc `relation` của claim).
//! - `as_of`: ngày `YYYY-MM-DD`; chỉ cạnh có cửa sổ hiệu lực chứa ngày đó (cửa sổ đóng hai đầu).
//! - `statuses`: các lớp cạnh được tính. Mặc định `accepted`, `superseded` và `proposed`; `accepted_only` chỉ lấy
//!   `accepted` và `superseded`. Cạnh `rejected` chỉ được trả khi ghi rõ trong `statuses`.
//! - `system`: vault có adapter khai báo note hệ thống thì mặc định không đi qua, không trả các note đó (trừ điểm
//!   đầu, điểm cuối); `system` để tính cả.
//!
//! Đi ngược một cạnh thì tên quan hệ là tên nghịch đảo của schema (`~rel` nếu không có); quan hệ đối xứng đi được
//! cả hai chiều với cùng tên.

pub mod answers;
pub(crate) mod py;
#[cfg(test)]
mod tests;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;
use std::path::Path;

use indexmap::IndexMap;
use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, params_from_iter};
use serde_json::{Map, Value};

use crate::index::{self, IndexError};
use crate::vault::yaml::repr::str_repr;
use crate::vault::{fold, nfc};

/// Lớp cạnh mặc định của tool.
pub const ALL: [&str; 3] = ["accepted", "superseded", "proposed"];
/// Lớp cạnh đã nhận (`accepted_only`).
pub const ACCEPTED: [&str; 2] = ["accepted", "superseded"];
/// Cột của cạnh trong kết quả (`EDGE_FIELDS`).
pub const EDGE_FIELDS: [&str; 15] = [
    "id",
    "src",
    "rel",
    "dst",
    "claim",
    "status",
    "valid_from",
    "valid_to",
    "asserted_at",
    "source",
    "origin",
    "proposed_by",
    "extracted_by",
    "confidence",
    "score",
];
/// Dấu phân cách các bước trong đường đi của CTE (`char(31)`).
const SEP: char = '\x1f';

/// `layer(status)`: `proposed`, `rejected`, hoặc `accepted` cho mọi status khác.
pub fn layer(status: Option<&str>) -> &'static str {
    match status {
        Some("proposed") => "proposed",
        Some("rejected") => "rejected",
        _ => "accepted",
    }
}

// ------------------------------------------------------------------------------------------------ lỗi

/// Chỗ bản Python ném ngoại lệ: `kind` là tên lớp ngoại lệ của Python (`ValueError`, `KeyError`,
/// `OperationalError`, ...), `message` là câu của nó.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryError {
    pub kind: &'static str,
    pub message: String,
}

impl QueryError {
    pub fn new(kind: &'static str, message: impl Into<String>) -> Self {
        QueryError {
            kind,
            message: message.into(),
        }
    }

    /// `repr(e)` của Python với ngoại lệ một tham số chuỗi, ví dụ `ValueError('loops cần ...')`.
    pub fn repr(&self) -> String {
        format!("{}({})", self.kind, str_repr(&self.message))
    }
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl std::error::Error for QueryError {}

impl From<rusqlite::Error> for QueryError {
    fn from(e: rusqlite::Error) -> Self {
        QueryError::new("OperationalError", e.to_string())
    }
}

impl From<IndexError> for QueryError {
    fn from(e: IndexError) -> Self {
        QueryError::new("OperationalError", e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, QueryError>;

// ------------------------------------------------------------------------------------------------ lọc chung

/// Một quan hệ của schema (`schema_edge`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rel {
    pub inverse: Option<String>,
    pub symmetric: bool,
    pub polarity: Option<String>,
}

/// Hướng đi: `out` (cạnh ghi trên note), `in` (cạnh trỏ vào), `both`. Chữ khác `out`, `in` được Python coi như
/// `both`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Out,
    In,
    Both,
}

impl Dir {
    pub fn parse(s: &str) -> Dir {
        match s {
            "out" => Dir::Out,
            "in" => Dir::In,
            _ => Dir::Both,
        }
    }
}

/// Lọc chung (`_f`). Dựng bằng [`Filter::new`] để có mặc định của `statuses` như Python.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// `None` là mọi quan hệ.
    pub rels: Option<Vec<String>>,
    pub as_of: Option<String>,
    pub statuses: Vec<String>,
    pub system: bool,
}

impl Filter {
    /// `_f(rels, as_of, statuses, accepted_only, system)`: danh sách rỗng như không ghi; `statuses` không ghi thì
    /// là [`ALL`], hoặc [`ACCEPTED`] khi `accepted_only`.
    pub fn new(
        rels: Option<&[String]>,
        as_of: Option<&str>,
        statuses: Option<&[String]>,
        accepted_only: bool,
        system: bool,
    ) -> Filter {
        let statuses = match statuses {
            Some(s) if !s.is_empty() => s.to_vec(),
            _ => (if accepted_only {
                &ACCEPTED[..]
            } else {
                &ALL[..]
            })
            .iter()
            .map(|s| (*s).to_owned())
            .collect(),
        };
        Filter {
            rels: rels.filter(|r| !r.is_empty()).map(<[String]>::to_vec),
            as_of: as_of.filter(|a| !a.is_empty()).map(str::to_owned),
            statuses,
            system,
        }
    }
}

/// Một nhánh bước đi: cột gần, cột xa, tên quan hệ, polarity, điều kiện thêm.
struct Part {
    near: &'static str,
    far: &'static str,
    rel: &'static str,
    pol: &'static str,
    extra: &'static str,
}

fn placeholders(n: usize) -> String {
    vec!["?"; n].join(",")
}

fn text(v: &[SqlValue], i: usize) -> Option<String> {
    match v.get(i) {
        Some(SqlValue::Text(s)) => Some(s.clone()),
        _ => None,
    }
}

fn int(v: &[SqlValue], i: usize) -> i64 {
    match v.get(i) {
        Some(SqlValue::Integer(n)) => *n,
        _ => 0,
    }
}

// ------------------------------------------------------------------------------------------------ graph

/// Index mở để đọc (`Graph`).
pub struct Graph {
    db: Connection,
    rels: IndexMap<String, Rel>,
    hidden: Vec<String>,
}

impl Graph {
    /// Mở index chỉ đọc.
    pub fn open(db: &Path) -> Result<Graph> {
        Graph::new(index::connect(db, true)?)
    }

    /// Dùng một kết nối có sẵn.
    pub fn new(db: Connection) -> Result<Graph> {
        let mut rels = IndexMap::new();
        {
            let mut st = db.prepare("SELECT key, inverse, symmetric, polarity FROM schema_edge")?;
            let mut rows = st.query([])?;
            while let Some(r) = rows.next()? {
                let sym: SqlValue = r.get(2)?;
                let symmetric = match sym {
                    SqlValue::Null => false,
                    SqlValue::Integer(i) => i != 0,
                    SqlValue::Real(x) => x != 0.0,
                    SqlValue::Text(s) => !s.is_empty(),
                    SqlValue::Blob(b) => !b.is_empty(),
                };
                rels.insert(
                    r.get::<_, String>(0)?,
                    Rel {
                        inverse: r.get(1)?,
                        symmetric,
                        polarity: r.get(3)?,
                    },
                );
            }
        }
        let mut hidden_json: Option<String> = None;
        {
            let mut st = db.prepare("SELECT key, value FROM meta")?;
            let mut rows = st.query([])?;
            while let Some(r) = rows.next()? {
                if r.get::<_, Option<String>>(0)?.as_deref() == Some("hidden") {
                    hidden_json = r.get(1)?;
                }
            }
        }
        // `json.loads(meta.get('hidden') or '[]')`, bỏ mục rỗng; JSON hỏng thì không có note hệ thống.
        let hidden = hidden_json
            .filter(|s| !s.is_empty())
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .and_then(|v| match v {
                Value::Array(a) => Some(
                    a.into_iter()
                        .filter_map(|h| match h {
                            Value::String(s) if !s.is_empty() => Some(s),
                            _ => None,
                        })
                        .collect(),
                ),
                _ => None,
            })
            .unwrap_or_default();
        Ok(Graph { db, rels, hidden })
    }

    /// Kết nối SQLite của index.
    pub fn conn(&self) -> &Connection {
        &self.db
    }

    /// Quan hệ của schema (`schema_edge`), theo thứ tự dòng.
    pub fn rels(&self) -> &IndexMap<String, Rel> {
        &self.rels
    }

    /// Tiền tố (kết thúc `/`) hoặc đường dẫn đủ của note hệ thống mà adapter khai báo.
    pub fn hidden(&self) -> &[String] {
        &self.hidden
    }

    pub fn is_hidden(&self, node: &str) -> bool {
        self.hidden.iter().any(|h| {
            if h.ends_with('/') {
                node.starts_with(h.as_str())
            } else {
                node == h
            }
        })
    }

    /// `_hid`: điều kiện SQL `col` không là note hệ thống, hoặc là một trong `allow` (điểm đầu, điểm cuối). Rỗng nếu
    /// vault không có note hệ thống.
    fn hid(&self, col: &str, allow: &[&str]) -> String {
        if self.hidden.is_empty() {
            return String::new();
        }
        let lit = |x: &str| format!("'{}'", x.replace('\'', "''"));
        let hid: Vec<String> = self
            .hidden
            .iter()
            .map(|h| {
                if h.ends_with('/') {
                    format!("substr({col}, 1, {}) = {}", h.chars().count(), lit(h))
                } else {
                    format!("{col} = {}", lit(h))
                }
            })
            .collect();
        let ok = if allow.is_empty() {
            String::new()
        } else {
            let a: Vec<String> = allow.iter().map(|x| lit(x)).collect();
            format!(" OR {col} IN ({})", a.join(", "))
        };
        format!(" AND (NOT ({}){ok})", hid.join(" OR "))
    }

    fn hide(&self, f: &Filter, col: &str, allow: &[&str]) -> String {
        if f.system {
            String::new()
        } else {
            self.hid(col, allow)
        }
    }

    /// `_w`: điều kiện trên cạnh `e` (đích đã giải, quan hệ, lớp, cửa sổ hiệu lực); tham số thêm vào `params`.
    pub fn edge_where(&self, f: &Filter, params: &mut Vec<SqlValue>) -> String {
        let mut w = vec!["e.dst IS NOT NULL".to_owned()];
        if let Some(rels) = &f.rels {
            w.push(format!("e.rel IN ({})", placeholders(rels.len())));
            params.extend(rels.iter().cloned().map(SqlValue::Text));
        }
        w.push(format!("e.status IN ({})", placeholders(f.statuses.len())));
        params.extend(f.statuses.iter().cloned().map(SqlValue::Text));
        if let Some(a) = &f.as_of {
            w.push(
                "(e.valid_from IS NULL OR e.valid_from <= ?) AND (e.valid_to IS NULL OR ? <= e.valid_to)"
                    .to_owned(),
            );
            params.push(SqlValue::Text(a.clone()));
            params.push(SqlValue::Text(a.clone()));
        }
        w.join(" AND ")
    }

    fn sym(&self, rels: Option<&[String]>) -> bool {
        match rels {
            Some(rs) => rs
                .iter()
                .any(|r| self.rels.get(r).is_some_and(|x| x.symmetric)),
            None => self.rels.values().any(|x| x.symmetric),
        }
    }

    fn parts(&self, f: &Filter, dir: Dir) -> Vec<Part> {
        let sym = self.sym(f.rels.as_deref());
        let mut parts = Vec::new();
        if dir != Dir::In || sym {
            parts.push(Part {
                near: "e.src",
                far: "e.dst",
                rel: "e.rel",
                pol: "x.polarity",
                extra: if dir != Dir::In {
                    ""
                } else {
                    " AND x.symmetric = 1"
                },
            });
        }
        if dir != Dir::Out || sym {
            parts.push(Part {
                near: "e.dst",
                far: "e.src",
                rel: "CASE WHEN x.symmetric = 1 THEN e.rel ELSE coalesce(x.inverse, '~' || e.rel) END",
                pol: "CASE WHEN x.symmetric = 1 THEN x.polarity END",
                extra: if dir != Dir::Out {
                    ""
                } else {
                    " AND x.symmetric = 1"
                },
            });
        }
        parts
    }

    /// Mọi dòng của một câu, mỗi dòng là các giá trị theo thứ tự cột.
    fn rows(&self, sql: &str, params: &[SqlValue]) -> Result<Vec<Vec<SqlValue>>> {
        let mut st = self.db.prepare(sql)?;
        let n = st.column_count();
        let mut rows = st.query(params_from_iter(params.iter()))?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            let mut v = Vec::with_capacity(n);
            for i in 0..n {
                v.push(r.get::<_, SqlValue>(i)?);
            }
            out.push(v);
        }
        Ok(out)
    }

    fn strings(&self, sql: &str, params: &[SqlValue]) -> Result<Vec<String>> {
        Ok(self
            .rows(sql, params)?
            .into_iter()
            .map(|r| match r.into_iter().next() {
                Some(v) => py::str_of(&py::sql_to_json(v)),
                None => String::new(),
            })
            .collect())
    }

    // ------------------------------------------------------------------------------------------- tìm note

    /// Có note `id` trong index.
    pub fn has_node(&self, id: &str) -> Result<bool> {
        let mut st = self.db.prepare("SELECT 1 FROM nodes WHERE id = ?")?;
        Ok(st.exists([id])?)
    }

    /// Note khớp tên người gõ: đường dẫn đủ, hoặc mọi từ của `name` (không phân biệt hoa thường, dấu) là một từ của
    /// tiêu đề hay một alias. Mỗi mục: `id`, `title`, `type`, `aliases`.
    pub fn find(
        &self,
        name: &str,
        type_: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Vec<Map<String, Value>>> {
        let q = nfc(py::strip(name));
        let q = q.strip_prefix("[[").unwrap_or(&q);
        let q = q.strip_suffix("]]").unwrap_or(q);
        let q = q.split('|').next().unwrap_or_default();
        let exact = if q.ends_with(".md") {
            q.to_owned()
        } else {
            format!("{q}.md")
        };
        let ids: Vec<String> = if self.has_node(&exact)? {
            vec![exact]
        } else {
            let folded = fold(q);
            let want: Vec<&str> = folded.split(' ').collect();
            let mut ids = BTreeSet::new();
            for r in self.rows("SELECT node, norm FROM names", &[])? {
                let norm = text(&r, 1).unwrap_or_default();
                let words: HashSet<&str> = norm.split(' ').collect();
                if want.iter().all(|w| words.contains(w)) {
                    ids.insert(text(&r, 0).unwrap_or_default());
                }
            }
            ids.into_iter().collect()
        };
        let mut out = Vec::new();
        for i in ids {
            let n = self.rows(
                "SELECT id, title, type FROM nodes WHERE id = ?",
                &[SqlValue::Text(i.clone())],
            )?;
            let Some(n) = n.into_iter().next() else {
                return Err(QueryError::new(
                    "TypeError",
                    "'NoneType' object is not subscriptable",
                ));
            };
            if let Some(t) = type_.filter(|t| !t.is_empty())
                && text(&n, 2).as_deref() != Some(t)
            {
                continue;
            }
            let al: Vec<Value> = self
                .rows(
                    "SELECT name FROM names WHERE node = ? AND kind = 'alias'",
                    &[SqlValue::Text(i.clone())],
                )?
                .into_iter()
                .flat_map(|r| r.into_iter().map(py::sql_to_json))
                .collect();
            let mut it = n.into_iter().map(py::sql_to_json);
            let mut m = Map::new();
            for k in ["id", "title", "type"] {
                m.insert(k.into(), it.next().unwrap_or(Value::Null));
            }
            m.insert("aliases".into(), Value::Array(al));
            out.push(m);
        }
        Ok(match limit.filter(|l| *l != 0) {
            Some(l) => py::head(&out, l),
            None => out,
        })
    }

    /// Thẻ một note: type, thuộc tính, alias, cửa sổ hiệu lực, nguồn gốc, số cạnh theo quan hệ và lớp. `None` khi
    /// không có note.
    pub fn note(&self, node: &str) -> Result<Option<Map<String, Value>>> {
        const COLS: [&str; 11] = [
            "id",
            "title",
            "type",
            "folder",
            "status",
            "valid_from",
            "valid_to",
            "proposed_by",
            "extracted_by",
            "asserted_at",
            "yaml_error",
        ];
        let p = [SqlValue::Text(node.to_owned())];
        let Some(r) = self
            .rows(
                "SELECT id, title, type, folder, status, valid_from, valid_to, proposed_by, extracted_by, asserted_at, yaml_error \
                 FROM nodes WHERE id = ?",
                &p,
            )?
            .into_iter()
            .next()
        else {
            return Ok(None);
        };
        let is_claim = matches!(r.get(2), Some(SqlValue::Text(t)) if t == "claim");
        let mut d = py::object(COLS.iter().copied().zip(r.into_iter().map(py::sql_to_json)));
        let mut props: IndexMap<String, Vec<Value>> = IndexMap::new();
        for r in self.rows("SELECT key, value FROM props WHERE node = ?", &p)? {
            let mut it = r.into_iter();
            let k = it.next().map(|k| py::sql_key(&k)).unwrap_or_default();
            props
                .entry(k)
                .or_default()
                .push(it.next().map_or(Value::Null, py::sql_to_json));
        }
        let props: Map<String, Value> = props
            .into_iter()
            .map(|(k, mut v)| {
                let v = if v.len() == 1 {
                    v.remove(0)
                } else {
                    Value::Array(v)
                };
                (k, v)
            })
            .collect();
        d.insert("props".into(), Value::Object(props));
        let blocks: Vec<Value> = self
            .rows("SELECT block FROM blocks WHERE node = ?", &p)?
            .into_iter()
            .flat_map(|r| r.into_iter().map(py::sql_to_json))
            .collect();
        d.insert("blocks".into(), Value::Array(blocks));
        let mut deg: Map<String, Value> = Map::new();
        let mut add = |side: &str, name: String, st: &SqlValue, n: SqlValue| {
            let side = deg.entry(side).or_insert_with(|| Value::Object(Map::new()));
            if let Value::Object(side) = side {
                let rel = side
                    .entry(name)
                    .or_insert_with(|| Value::Object(Map::new()));
                if let Value::Object(rel) = rel {
                    rel.insert(py::sql_key(st), py::sql_to_json(n));
                }
            }
        };
        for r in self.rows(
            "SELECT rel, status, count(*) FROM edges WHERE src = ? AND dst IS NOT NULL GROUP BY 1, 2",
            &p,
        )? {
            let [rel, st, n]: [SqlValue; 3] = r.try_into().unwrap_or([SqlValue::Null, SqlValue::Null, SqlValue::Null]);
            add("out", py::sql_key(&rel), &st, n);
        }
        for r in self.rows(
            "SELECT rel, status, count(*) FROM edges WHERE dst = ? GROUP BY 1, 2",
            &p,
        )? {
            let [rel, st, n]: [SqlValue; 3] =
                r.try_into()
                    .unwrap_or([SqlValue::Null, SqlValue::Null, SqlValue::Null]);
            let rel = py::sql_key(&rel);
            let name = match self.rels.get(&rel) {
                Some(x) if x.symmetric => rel,
                Some(Rel {
                    inverse: Some(inv), ..
                }) if !inv.is_empty() => inv.clone(),
                _ => format!("~{rel}"),
            };
            add("in", name, &st, n);
        }
        d.insert("edges".into(), Value::Object(deg));
        let claim_edge = if is_claim {
            self.edge_of_claim(node)?.map_or(Value::Null, Value::Object)
        } else {
            Value::Null
        };
        d.insert("claim_edge".into(), claim_edge);
        Ok(Some(d))
    }

    /// Cạnh mà một claim khẳng định.
    pub fn edge_of_claim(&self, claim: &str) -> Result<Option<Map<String, Value>>> {
        let sql = format!(
            "SELECT {} FROM edges WHERE claim = ?",
            EDGE_FIELDS.join(", ")
        );
        Ok(self
            .rows(&sql, &[SqlValue::Text(claim.to_owned())])?
            .into_iter()
            .next()
            .map(edge))
    }

    // ------------------------------------------------------------------------------------------- cạnh

    /// Cạnh quanh một note. Mỗi mục: `rel` (tên theo hướng đi), `node` (đầu kia), `dir` (`out` | `in`), cùng các
    /// cột của cạnh, `layer` và `edge_rel` (tên quan hệ ghi trên cạnh).
    pub fn neighbors(&self, node: &str, dir: Dir, f: &Filter) -> Result<Vec<Map<String, Value>>> {
        let cols: Vec<String> = EDGE_FIELDS.iter().map(|c| format!("e.{c}")).collect();
        let cols = cols.join(", ");
        let mut sels = Vec::new();
        let mut params = Vec::new();
        for part in self.parts(f, dir) {
            let mut p = Vec::new();
            let w = self.edge_where(f, &mut p);
            let d = if part.near == "e.src" {
                "'out'"
            } else {
                "'in'"
            };
            sels.push(format!(
                "SELECT {} AS r, {} AS b, {d} AS d, {cols} FROM edges e LEFT JOIN schema_edge x ON x.key = e.rel \
                 WHERE {} = ? AND {w}{}{}",
                part.rel,
                part.far,
                part.near,
                part.extra,
                self.hide(f, part.far, &[])
            ));
            params.push(SqlValue::Text(node.to_owned()));
            params.extend(p);
        }
        let sql = sels.join(" UNION ALL ") + " ORDER BY r, b, status";
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        for mut row in self.rows(&sql, &params)? {
            // Cạnh đối xứng trùng hai nhánh.
            let k = (text(&row, 0), text(&row, 1), int(&row, 3));
            if !seen.insert(k) {
                continue;
            }
            let e = edge(row.split_off(3));
            let mut it = row.into_iter().map(py::sql_to_json);
            let mut m = Map::new();
            for k in ["rel", "node", "dir"] {
                m.insert(k.into(), it.next().unwrap_or(Value::Null));
            }
            let mut edge_rel = Value::Null;
            for (k, v) in e {
                match k.as_str() {
                    "src" | "dst" => {}
                    "rel" => edge_rel = v,
                    _ => {
                        m.insert(k, v);
                    }
                }
            }
            m.insert("edge_rel".into(), edge_rel);
            out.push(m);
        }
        Ok(out)
    }

    /// Mọi note tới được từ `node` (`Out`: xuôi dòng, `In`: ngược dòng), không gồm chính nó: `(id, độ sâu nhỏ
    /// nhất)`, độ sâu chỉ có khi đặt `max_depth`.
    pub fn reach(
        &self,
        node: &str,
        dir: Dir,
        max_depth: Option<i64>,
        f: &Filter,
    ) -> Result<Vec<(String, Option<i64>)>> {
        let max_depth = max_depth.filter(|d| *d != 0);
        let mut p = vec![SqlValue::Text(node.to_owned())];
        let mut sels = Vec::new();
        for part in self.parts(f, dir) {
            let w = self.edge_where(f, &mut p);
            let extra = format!("{}{}", part.extra, self.hide(f, part.far, &[]));
            match max_depth {
                Some(d) => {
                    sels.push(format!(
                        "SELECT {}, r.d + 1 FROM r JOIN edges e ON {} = r.id LEFT JOIN schema_edge x ON x.key = e.rel WHERE {w}{extra} AND r.d < ?",
                        part.far, part.near
                    ));
                    p.push(SqlValue::Integer(d));
                }
                // Không giới hạn: chỉ giữ id để UNION chặn đi lại (giữ độ sâu thì mỗi độ sâu là một dòng mới).
                None => sels.push(format!(
                    "SELECT {} FROM r JOIN edges e ON {} = r.id LEFT JOIN schema_edge x ON x.key = e.rel WHERE {w}{extra}",
                    part.far, part.near
                )),
            }
        }
        let sql = if max_depth.is_some() {
            format!(
                "WITH RECURSIVE r(id, d) AS (SELECT ?, 0 UNION {}) SELECT id, min(d) FROM r WHERE id != ? GROUP BY id",
                sels.join(" UNION ")
            )
        } else {
            format!(
                "WITH RECURSIVE r(id) AS (SELECT ? UNION {}) SELECT id, NULL FROM r WHERE id != ?",
                sels.join(" UNION ")
            )
        };
        p.push(SqlValue::Text(node.to_owned()));
        let mut rows: Vec<(String, Option<i64>)> = self
            .rows(&sql, &p)?
            .into_iter()
            .map(|r| {
                let d = match r.get(1) {
                    Some(SqlValue::Integer(d)) => Some(*d),
                    _ => None,
                };
                (text(&r, 0).unwrap_or_default(), d)
            })
            .collect();
        rows.sort();
        Ok(rows)
    }

    /// Mọi đường đơn có hướng từ `frm` tới `to`, tối đa `max_len` cạnh, ngắn trước. Mỗi đường: `nodes`, `rels`,
    /// `statuses` (từng bước), `sign` (`+` hay `-`, theo polarity), `layer` (`accepted` nếu mọi bước đã nhận, còn
    /// lại `proposed`), `parallel` (số đường song song đã gộp).
    pub fn paths(
        &self,
        frm: &str,
        to: &str,
        max_len: i64,
        f: &Filter,
        limit: Option<i64>,
    ) -> Result<Vec<Map<String, Value>>> {
        let mut p = vec![
            SqlValue::Text(frm.to_owned()),
            SqlValue::Text(frm.to_owned()),
        ];
        let mut sels = Vec::new();
        for part in self.parts(f, Dir::Out) {
            let w = self.edge_where(f, &mut p);
            let extra = format!("{}{}", part.extra, self.hide(f, part.far, &[to]));
            let (far, r, pol, near) = (part.far, part.rel, part.pol, part.near);
            sels.push(format!(
                "SELECT {far}, p.path || {far} || char(31), p.rs || {r} || char(31), p.st || e.status || char(31), \
                 p.neg + ({pol} IS '-'), p.k + 1 FROM p JOIN edges e ON {near} = p.n LEFT JOIN schema_edge x ON x.key = e.rel \
                 WHERE {w}{extra} AND p.k < ? AND (p.n != ? OR p.k = 0) AND instr(p.path, char(31) || {far} || char(31)) = 0"
            ));
            p.push(SqlValue::Integer(max_len));
            p.push(SqlValue::Text(to.to_owned()));
        }
        let sql = format!(
            "WITH RECURSIVE p(n, path, rs, st, neg, k) AS (SELECT ?, char(31) || ? || char(31), '', '', 0, 0 UNION ALL {}) \
             SELECT path, rs, st, neg FROM p WHERE n = ? AND k > 0",
            sels.join(" UNION ALL ")
        );
        p.push(SqlValue::Text(to.to_owned()));
        let walks = self.walks(&sql, &p)?;
        let out: Vec<Map<String, Value>> = collapse(walks)
            .into_iter()
            .map(|(w, n)| {
                let sign = if w.neg % 2 != 0 { "-" } else { "+" };
                let mut m = w.head();
                m.insert("sign".into(), sign.into());
                m.insert("layer".into(), w.layer().into());
                m.insert("parallel".into(), n.into());
                m
            })
            .collect();
        Ok(match limit.filter(|l| *l != 0) {
            Some(l) => py::head(&out, l),
            None => out,
        })
    }

    /// Vòng đơn có hướng đi qua `node`, tối đa `max_len` cạnh (bắt buộc cả hai). Mỗi vòng bắt đầu ở `node`:
    /// `nodes`, `rels`, `statuses`, `negatives` (số cạnh âm), `kind` (`reinforcing` nếu chẵn, `balancing` nếu lẻ),
    /// `layer`, `parallel`.
    pub fn loops(&self, node: &str, max_len: i64, f: &Filter) -> Result<Vec<Map<String, Value>>> {
        if node.is_empty() || max_len == 0 {
            return Err(QueryError::new(
                "ValueError",
                "loops cần node xuất phát và max_len",
            ));
        }
        let n = || SqlValue::Text(node.to_owned());
        let mut p = vec![n(), n()];
        let mut sels = Vec::new();
        for part in self.parts(f, Dir::Out) {
            let mut wp = Vec::new();
            let w = self.edge_where(f, &mut wp);
            let extra = format!("{}{}", part.extra, self.hide(f, part.far, &[node]));
            let (far, r, pol, near) = (part.far, part.rel, part.pol, part.near);
            sels.push(format!(
                "SELECT {far}, c.path || CASE WHEN {far} = ? THEN '' ELSE {far} || char(31) END, c.rs || {r} || char(31), \
                 c.st || e.status || char(31), c.neg + ({pol} IS '-'), c.k + 1, {far} = ? FROM c JOIN edges e ON {near} = c.n \
                 LEFT JOIN schema_edge x ON x.key = e.rel WHERE {w}{extra} AND c.closed = 0 \
                 AND ({far} = ? OR (c.k + 1 < ? AND instr(c.path, char(31) || {far} || char(31)) = 0))"
            ));
            p.push(n());
            p.push(n());
            p.extend(wp);
            p.push(n());
            p.push(SqlValue::Integer(max_len));
        }
        let sql = format!(
            "WITH RECURSIVE c(n, path, rs, st, neg, k, closed) AS (SELECT ?, char(31) || ? || char(31), '', '', 0, 0, 0 UNION ALL {}) \
             SELECT path, rs, st, neg FROM c WHERE closed = 1",
            sels.join(" UNION ALL ")
        );
        let walks = self.walks(&sql, &p)?;
        Ok(collapse(walks)
            .into_iter()
            .map(|(w, n)| {
                let mut m = w.loop_map();
                m.insert("parallel".into(), n.into());
                m
            })
            .collect())
    }

    fn walks(&self, sql: &str, p: &[SqlValue]) -> Result<Vec<Walk>> {
        let split = |s: Option<String>| -> Vec<String> {
            s.unwrap_or_default()
                .trim_matches(SEP)
                .split(SEP)
                .map(str::to_owned)
                .collect()
        };
        Ok(self
            .rows(sql, p)?
            .into_iter()
            .map(|r| Walk {
                nodes: split(text(&r, 0)),
                rels: split(text(&r, 1)),
                statuses: split(text(&r, 2)),
                neg: int(&r, 3),
            })
            .collect())
    }

    /// Mọi vòng của cả vault (không là tool; cho câu đáp án): SQL lấy cạnh còn sống, tìm vòng trong code (Tarjan
    /// tách thành phần liên thông mạnh, rồi DFS). Mỗi vòng bắt đầu ở id nhỏ nhất.
    pub fn all_loops(&self, max_len: i64, f: &Filter) -> Result<Vec<Map<String, Value>>> {
        let mut adj: IndexMap<String, Vec<Step>> = IndexMap::new();
        for part in self.parts(f, Dir::Out) {
            let mut p = Vec::new();
            let w = self.edge_where(f, &mut p);
            let sql = format!(
                "SELECT {}, {}, {}, {}, e.status FROM edges e LEFT JOIN schema_edge x ON x.key = e.rel WHERE {w}{}",
                part.near, part.rel, part.far, part.pol, part.extra
            );
            for r in self.rows(&sql, &p)? {
                adj.entry(text(&r, 0).unwrap_or_default())
                    .or_default()
                    .push(Step {
                        to: text(&r, 2).unwrap_or_default(),
                        rel: text(&r, 1).unwrap_or_default(),
                        pol: text(&r, 3),
                        status: text(&r, 4).unwrap_or_default(),
                    });
            }
        }
        let comp = scc(&adj);
        let mut starts: Vec<&String> = adj.keys().collect();
        starts.sort();
        let mut out = Vec::new();
        for start in starts {
            let mut cx = Cycles {
                adj: &adj,
                comp: &comp,
                start,
                cs: comp.get(start.as_str()).copied(),
                max_len,
                out: &mut out,
            };
            let mut nodes = vec![start.clone()];
            cx.walk(start, &mut nodes, &[], &[], &[]);
        }
        Ok(out)
    }

    // ------------------------------------------------------------------------------------------- nguồn gốc

    /// Claim (hay note bất kỳ) lấy từ đâu: nguồn trích (note, block), note ủng hộ/phản bác, người và máy đề xuất,
    /// thời điểm ghi nhận, chuỗi thay thế (`superseded_by`).
    pub fn provenance(&self, claim: &str) -> Result<Map<String, Value>> {
        let p = [SqlValue::Text(claim.to_owned())];
        let mut srcs = Vec::new();
        for r in self.rows(
            "SELECT dst, target, frag FROM links WHERE src = ? AND key = 'source' AND origin = 'frontmatter' ORDER BY dst, frag",
            &p,
        )? {
            let block = match r.get(2) {
                Some(SqlValue::Text(fr)) => fr
                    .strip_prefix("#^")
                    .map_or(Value::Null, |b| Value::String(b.to_owned())),
                _ => {
                    return Err(QueryError::new(
                        "AttributeError",
                        "'NoneType' object has no attribute 'startswith'",
                    ));
                }
            };
            let mut it = r.into_iter().map(py::sql_to_json);
            let mut m = Map::new();
            m.insert("note".into(), it.next().unwrap_or(Value::Null));
            m.insert("target".into(), it.next().unwrap_or(Value::Null));
            m.insert("block".into(), block);
            srcs.push(Value::Object(m));
        }
        let ev: Vec<Value> = self
            .rows(
                "SELECT rel, src, status FROM edges WHERE dst = ? AND rel IN ('supports', 'disputes') ORDER BY src || rel",
                &p,
            )?
            .into_iter()
            .map(|r| {
                Value::Object(py::object(
                    ["rel", "from", "status"]
                        .into_iter()
                        .zip(r.into_iter().map(py::sql_to_json)),
                ))
            })
            .collect();
        let record = self
            .rows(
                "SELECT status, proposed_by, extracted_by, asserted_at, valid_from, valid_to FROM nodes WHERE id = ?",
                &p,
            )?
            .into_iter()
            .next()
            .map_or(Value::Null, |r| {
                Value::Object(py::object(
                    [
                        "status",
                        "proposed_by",
                        "extracted_by",
                        "asserted_at",
                        "valid_from",
                        "valid_to",
                    ]
                    .into_iter()
                    .zip(r.into_iter().map(py::sql_to_json)),
                ))
            });
        let mut chain = Vec::new();
        let mut cur = claim.to_owned();
        let mut seen = HashSet::from([claim.to_owned()]);
        loop {
            let r = self.rows(
                "SELECT dst FROM links WHERE src = ? AND key = 'superseded_by' AND dst IS NOT NULL",
                &[SqlValue::Text(cur.clone())],
            )?;
            let Some(next) = r
                .first()
                .map(|r| py::str_of(&py::sql_to_json(r[0].clone())))
            else {
                break;
            };
            if !seen.insert(next.clone()) {
                break;
            }
            chain.push(Value::String(next.clone()));
            cur = next;
        }
        let edge = self
            .edge_of_claim(claim)?
            .map_or(Value::Null, Value::Object);
        let mut out = Map::new();
        out.insert("claim".into(), claim.into());
        out.insert("sources".into(), Value::Array(srcs));
        out.insert("evidence".into(), Value::Array(ev));
        out.insert("record".into(), record);
        out.insert("edge".into(), edge);
        out.insert("superseded_by".into(), Value::Array(chain));
        Ok(out)
    }

    // ------------------------------------------------------------------------------------------- bảng

    /// Note có mọi cặp khoá = giá trị trong `where` (so chuỗi như `str()` của Python, `true`/`false` cho bool;
    /// `type` là type của note). Bỏ note hệ thống trừ khi `system`.
    pub fn select(&self, where_: &Map<String, Value>, system: bool) -> Result<Vec<String>> {
        let mut sql = format!(
            "SELECT id FROM nodes n WHERE 1{}",
            if system {
                String::new()
            } else {
                self.hid("n.id", &[])
            }
        );
        let mut p = Vec::new();
        for (k, v) in where_ {
            if k == "type" {
                sql.push_str(" AND n.type = ?");
            } else {
                sql.push_str(
                    " AND EXISTS (SELECT 1 FROM props p WHERE p.node = n.id AND p.key = ? AND p.value = ?)",
                );
                p.push(SqlValue::Text(k.clone()));
            }
            let s = match v {
                Value::Bool(true) => "true".to_owned(),
                Value::Bool(false) => "false".to_owned(),
                v => py::str_of(v),
            };
            p.push(SqlValue::Text(s));
        }
        sql.push_str(" ORDER BY id");
        self.strings(&sql, &p)
    }

    /// Note của một type có cửa sổ hiệu lực (`valid_from`, có thể có `valid_to`) chứa ngày `as_of`. Bỏ note hệ
    /// thống trừ khi `system`.
    pub fn valid(&self, type_: &str, as_of: &str, system: bool) -> Result<Vec<String>> {
        let sql = format!(
            "SELECT id FROM nodes WHERE type = ? AND valid_from IS NOT NULL AND valid_from <= ? AND (valid_to IS NULL OR ? <= valid_to){} ORDER BY id",
            if system {
                String::new()
            } else {
                self.hid("id", &[])
            }
        );
        let t = |s: &str| SqlValue::Text(s.to_owned());
        self.strings(&sql, &[t(type_), t(as_of), t(as_of)])
    }

    /// Type của các note (`None` khi note không có type; note không có trong index thì vắng).
    pub fn types_of(&self, ids: &[String]) -> Result<HashMap<String, Option<String>>> {
        let mut out = HashMap::new();
        for chunk in ids.chunks(500) {
            let sql = format!(
                "SELECT id, type FROM nodes WHERE id IN ({})",
                placeholders(chunk.len())
            );
            let p: Vec<SqlValue> = chunk.iter().cloned().map(SqlValue::Text).collect();
            for r in self.rows(&sql, &p)? {
                out.insert(text(&r, 0).unwrap_or_default(), text(&r, 1));
            }
        }
        Ok(out)
    }
}

/// `_edge`: object của một cạnh theo [`EDGE_FIELDS`], thêm `layer`.
fn edge(r: Vec<SqlValue>) -> Map<String, Value> {
    let status = match r.get(5) {
        Some(SqlValue::Text(s)) => Some(s.clone()),
        _ => None,
    };
    let mut e = py::object(
        EDGE_FIELDS
            .iter()
            .copied()
            .zip(r.into_iter().map(py::sql_to_json)),
    );
    e.insert("layer".into(), layer(status.as_deref()).into());
    e
}

// ------------------------------------------------------------------------------------------------ đường, vòng

/// Một đường (hay vòng) đọc từ CTE.
#[derive(Debug, Clone)]
struct Walk {
    nodes: Vec<String>,
    rels: Vec<String>,
    statuses: Vec<String>,
    neg: i64,
}

impl Walk {
    fn layer(&self) -> &'static str {
        if self.statuses.iter().all(|s| s != "proposed") {
            "accepted"
        } else {
            "proposed"
        }
    }

    fn head(&self) -> Map<String, Value> {
        let mut m = Map::new();
        m.insert("nodes".into(), self.nodes.clone().into());
        m.insert("rels".into(), self.rels.clone().into());
        m.insert("statuses".into(), self.statuses.clone().into());
        m
    }

    fn loop_map(&self) -> Map<String, Value> {
        let mut m = self.head();
        m.insert("negatives".into(), self.neg.into());
        m.insert("kind".into(), kind(self.neg).into());
        m.insert("layer".into(), self.layer().into());
        m
    }
}

fn kind(neg: i64) -> &'static str {
    if neg % 2 != 0 {
        "balancing"
    } else {
        "reinforcing"
    }
}

/// `_collapse`: gộp đường (hay vòng) đi qua cùng dãy note và quan hệ nhưng bằng cạnh song song: giữ một, ưu tiên
/// bản mọi bước đã nhận; kèm số bản đã gộp. Sắp ngắn trước.
fn collapse(items: Vec<Walk>) -> Vec<(Walk, i64)> {
    let mut best: IndexMap<(Vec<String>, Vec<String>), (Walk, i64)> = IndexMap::new();
    for x in items {
        let k = (x.nodes.clone(), x.rels.clone());
        match best.get_mut(&k) {
            None => {
                best.insert(k, (x, 1));
            }
            Some((b, n)) => {
                *n += 1;
                if b.layer() != "accepted" && x.layer() == "accepted" {
                    *b = x;
                }
            }
        }
    }
    let mut out: Vec<(Walk, i64)> = best.into_values().collect();
    out.sort_by(|(a, _), (b, _)| {
        (a.nodes.len(), &a.nodes, &a.rels).cmp(&(b.nodes.len(), &b.nodes, &b.rels))
    });
    out
}

/// Một cạnh trong danh sách kề của [`Graph::all_loops`].
struct Step {
    to: String,
    rel: String,
    pol: Option<String>,
    status: String,
}

/// DFS tìm vòng từ một điểm đầu của [`Graph::all_loops`].
struct Cycles<'a> {
    adj: &'a IndexMap<String, Vec<Step>>,
    comp: &'a HashMap<&'a str, usize>,
    start: &'a str,
    cs: Option<usize>,
    max_len: i64,
    out: &'a mut Vec<Map<String, Value>>,
}

impl Cycles<'_> {
    fn walk(
        &mut self,
        a: &str,
        nodes: &mut Vec<String>,
        rs: &[String],
        ps: &[Option<String>],
        sts: &[String],
    ) {
        let Some(steps) = self.adj.get(a) else {
            return;
        };
        for s in steps {
            if self.comp.get(s.to.as_str()).copied() != self.cs {
                continue;
            }
            let mut rs2 = rs.to_vec();
            rs2.push(s.rel.clone());
            let mut ps2 = ps.to_vec();
            ps2.push(s.pol.clone());
            let mut sts2 = sts.to_vec();
            sts2.push(s.status.clone());
            if s.to == self.start {
                let neg = ps2.iter().filter(|x| x.as_deref() == Some("-")).count();
                let neg = i64::try_from(neg).unwrap_or(i64::MAX);
                let layer = if sts2.iter().any(|x| x == "proposed") {
                    "proposed"
                } else {
                    "accepted"
                };
                let mut m = Map::new();
                m.insert("nodes".into(), nodes.clone().into());
                m.insert("rels".into(), rs2.into());
                m.insert("statuses".into(), sts2.into());
                m.insert("negatives".into(), neg.into());
                m.insert("kind".into(), kind(neg).into());
                m.insert("layer".into(), layer.into());
                self.out.push(m);
            } else if s.to.as_str() > self.start
                && !nodes.contains(&s.to)
                && i64::try_from(rs.len()).unwrap_or(i64::MAX) + 1 < self.max_len
            {
                nodes.push(s.to.clone());
                self.walk(&s.to, nodes, &rs2, &ps2, &sts2);
                nodes.pop();
            }
        }
    }
}

/// Tarjan không đệ quy: note -> số thành phần liên thông mạnh.
fn scc(adj: &IndexMap<String, Vec<Step>>) -> HashMap<&str, usize> {
    let mut nodes: BTreeSet<&str> = adj.keys().map(String::as_str).collect();
    nodes.extend(adj.values().flatten().map(|s| s.to.as_str()));
    let mut index: HashMap<&str, usize> = HashMap::new();
    let mut low: HashMap<&str, usize> = HashMap::new();
    let mut comp: HashMap<&str, usize> = HashMap::new();
    let mut stack: Vec<&str> = Vec::new();
    let mut on: HashSet<&str> = HashSet::new();
    let (mut n, mut c) = (0usize, 0usize);
    for root in nodes {
        if index.contains_key(root) {
            continue;
        }
        let mut work: Vec<(&str, usize)> = vec![(root, 0)];
        while let Some((v, i)) = work.pop() {
            if i == 0 {
                index.insert(v, n);
                low.insert(v, n);
                n += 1;
                stack.push(v);
                on.insert(v);
            }
            let nb: &[Step] = adj.get(v).map_or(&[], Vec::as_slice);
            if i < nb.len() {
                work.push((v, i + 1));
                let w = nb[i].to.as_str();
                if !index.contains_key(w) {
                    work.push((w, 0));
                } else if on.contains(w) {
                    let lw = index[w].min(low[v]);
                    low.insert(v, lw);
                }
                continue;
            }
            if low[v] == index[v] {
                while let Some(w) = stack.pop() {
                    on.remove(w);
                    comp.insert(w, c);
                    if w == v {
                        break;
                    }
                }
                c += 1;
            }
            if let Some(&(u, _)) = work.last() {
                let lu = low[u].min(low[v]);
                low.insert(u, lu);
            }
        }
    }
    comp
}
