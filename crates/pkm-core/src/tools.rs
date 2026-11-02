//! Bộ tool hỏi graph, dạng hàm trả JSON để MCP server và CLI bọc thẳng: `find`, `note`, `neighbors`, `reach`,
//! `paths`, `loops`, `provenance`, `select`, `valid`, `more` (lấy tiếp kết quả dài), và chỗ cho `related`.
//!
//! Đặc tả: `scripts/kg/tools.py` ([`Tools`]) và `Server.call` của `scripts/kg/mcp_server.py` ([`call`]: điểm vào
//! chung theo tên tool và tham số MCP, trước khi MCP thu gọn JSON) ở sha ghim.
//!
//! Quy ước chung:
//! - Mặc định trả cả cạnh `accepted` và `proposed` (máy gợi ý); mỗi cạnh có `status` và `layer`; `accepted_only`
//!   chỉ lấy lớp đã nhận. `as_of` lọc theo cửa sổ hiệu lực.
//! - Kết quả dài hơn `limit` (mặc định 20) trả `count`, `by_type` (số note theo type), `items` (các mục đầu),
//!   `truncated` và `handle`; gọi `more(handle, offset, limit)` để lấy tiếp.
//!
//! Tham số MCP đọc như Python đọc (`a.get(k) or mặc định`, `bool(...)`, `int(...)`, `str(...)` của note id). Khác
//! Python ở tham số sai kiểu mà Python vẫn chạy tiếp với giá trị lạ (ví dụ `as_of` là số, `rels` chứa số,
//! `limit` là số thực): Rust trả lỗi `TypeError` (spec/PORTING.md, mục `query`, `tools`).

pub mod defs;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use indexmap::IndexMap;
use regex::Regex;
use serde_json::{Map, Value, json};

use crate::query::{Dir, Filter, Graph, QueryError, py};
use crate::vault::{fold, nfc};

pub use defs::{ToolDef, definitions, tools_list};

/// Lỗi của một lời gọi tool: chỗ bản Python ném ngoại lệ (MCP trả `error: <repr>`).
pub type ToolError = QueryError;
type Result<T> = std::result::Result<T, ToolError>;

/// Số mục hiện trước khi tóm tắt kết quả.
pub const LIMIT: i64 = 20;
/// Số kết quả dài giữ lại cho `more`.
pub const CACHE: usize = 64;
/// Tên các tool, theo thứ tự của Python.
pub const NAMES: [&str; 11] = [
    "find",
    "note",
    "neighbors",
    "reach",
    "paths",
    "loops",
    "provenance",
    "select",
    "valid",
    "more",
    "related",
];

/// `FM_RE` của `vault.py` (`re.S`): frontmatter ở đầu note, để cắt phần thân cho tool `note`.
static FM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)\A---[ \t]*\r?\n(.*?)\r?\n---[ \t]*(?:\r?\n|\z)").expect("regex tĩnh hợp lệ")
});

fn type_error(msg: impl Into<String>) -> ToolError {
    ToolError::new("TypeError", msg)
}

/// Lọc trên cạnh của một tool: quan hệ, ngày, lớp, `accepted_only`, `system`, như tham số của `Tools` Python.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EdgeArgs {
    pub rels: Option<Vec<String>>,
    pub as_of: Option<String>,
    pub statuses: Option<Vec<String>>,
    pub accepted_only: bool,
    pub system: bool,
}

impl EdgeArgs {
    pub fn filter(&self) -> Filter {
        Filter::new(
            self.rels.as_deref(),
            self.as_of.as_deref(),
            self.statuses.as_deref(),
            self.accepted_only,
            self.system,
        )
    }

    /// Phần của khoá handle: `rels, as_of, statuses` như Python nhận.
    fn key(&self) -> [Value; 3] {
        [json!(self.rels), json!(self.as_of), json!(self.statuses)]
    }
}

// ------------------------------------------------------------------------------------------------ Tools

/// Handle của một kết quả dài: tên tool và 10 ký tự đầu của sha1 của `json.dumps([op, key], sort_keys=True,
/// ensure_ascii=False)`; cùng tham số thì cùng handle, như Python.
fn handle(op: &str, key: &Value) -> String {
    let digest = sha1_smol::Sha1::from(py::json_dumps_sorted(&json!([op, key])))
        .digest()
        .to_string();
    format!("{op}-{}", &digest[..10])
}

/// Bộ tool trên một [`Graph`], giữ các kết quả dài gần nhất cho `more`.
pub struct Tools {
    g: Graph,
    limit: i64,
    cache_size: usize,
    cache: IndexMap<String, Vec<Value>>,
}

impl Tools {
    pub fn new(g: Graph) -> Tools {
        Tools::with_limit(g, LIMIT)
    }

    /// `limit`: số mục hiện trước khi tóm tắt (0 là mặc định).
    pub fn with_limit(g: Graph, limit: i64) -> Tools {
        Tools {
            g,
            limit: if limit == 0 { LIMIT } else { limit },
            cache_size: CACHE,
            cache: IndexMap::new(),
        }
    }

    /// Mở index chỉ đọc.
    pub fn open(db: &Path) -> Result<Tools> {
        Ok(Tools::new(Graph::open(db)?))
    }

    pub fn graph(&self) -> &Graph {
        &self.g
    }

    /// Đổi sang index mới (sau khi build lại), giữ các kết quả dài cho `more`; trả index cũ.
    pub fn replace_graph(&mut self, g: Graph) -> Graph {
        std::mem::replace(&mut self.g, g)
    }

    /// `_env`: vỏ kết quả dài. `ids`: note của các mục (để đếm theo type); `None` thì lấy `id` hoặc `node` của
    /// từng mục.
    fn env(
        &mut self,
        op: &str,
        key: Value,
        items: Vec<Value>,
        ids: Option<Vec<String>>,
        limit: Option<i64>,
    ) -> Result<Map<String, Value>> {
        let limit = limit.filter(|l| *l != 0).unwrap_or(self.limit);
        let mut out = Map::new();
        out.insert("count".into(), items.len().into());
        let ids: Vec<String> = match ids {
            Some(ids) => ids,
            None => items
                .iter()
                .filter_map(|x| match x {
                    Value::String(s) => Some(s.clone()),
                    Value::Object(m) => m
                        .get("id")
                        .filter(|v| py::truthy(v))
                        .or_else(|| m.get("node"))
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    _ => None,
                })
                .collect(),
        };
        let long = i64::try_from(items.len()).unwrap_or(i64::MAX) > limit;
        if !ids.is_empty() && long {
            let set: Vec<String> = ids
                .into_iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let ts = self.g.types_of(&set)?;
            let mut count: IndexMap<String, usize> = IndexMap::new();
            for i in &set {
                let t = ts
                    .get(i)
                    .cloned()
                    .flatten()
                    .filter(|t| !t.is_empty())
                    .unwrap_or_else(|| "(không type)".to_owned());
                *count.entry(t).or_default() += 1;
            }
            // `Counter.most_common()`: số giảm dần; bằng nhau thì Python theo thứ tự băm, ở đây theo tên type.
            let mut count: Vec<(String, usize)> = count.into_iter().collect();
            count.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            let by: Map<String, Value> = count.into_iter().map(|(k, n)| (k, n.into())).collect();
            out.insert("by_type".into(), Value::Object(by));
        }
        if long {
            let h = handle(op, &key);
            let shown = py::head(&items, limit);
            self.cache.shift_remove(&h);
            self.cache.insert(h.clone(), items);
            while self.cache.len() > self.cache_size {
                self.cache.shift_remove_index(0);
            }
            out.insert("items".into(), Value::Array(shown));
            out.insert("truncated".into(), true.into());
            out.insert("handle".into(), h.into());
        } else {
            out.insert("items".into(), Value::Array(items));
        }
        Ok(out)
    }

    /// Các mục tiếp theo của một kết quả dài.
    pub fn more(&self, handle: &str, offset: i64, limit: Option<i64>) -> Map<String, Value> {
        self.more_echo(handle, offset, offset.into(), limit)
    }

    /// `more`, ghi lại `offset` đúng như người gọi đưa (`true` là 1 nhưng được trả lại là `true`, như Python).
    fn more_echo(
        &self,
        handle: &str,
        offset: i64,
        echo: Value,
        limit: Option<i64>,
    ) -> Map<String, Value> {
        let Some(items) = self.cache.get(handle) else {
            let mut m = Map::new();
            m.insert(
                "error".into(),
                format!("không còn kết quả {handle}; gọi lại tool gốc").into(),
            );
            return m;
        };
        let limit = limit.filter(|l| *l != 0).unwrap_or(self.limit);
        let r = py::slice(
            items.len(),
            Some(offset),
            Some(offset.saturating_add(limit)),
        );
        let mut m = Map::new();
        m.insert("count".into(), items.len().into());
        m.insert("offset".into(), echo);
        m.insert("items".into(), Value::Array(items[r].to_vec()));
        m.insert("handle".into(), handle.into());
        m
    }

    pub fn find(
        &mut self,
        name: &str,
        type_: Option<&str>,
        limit: Option<i64>,
    ) -> Result<Map<String, Value>> {
        let items = self.g.find(name, type_, None)?;
        self.find_items(json!([name, type_]), items, limit)
    }

    fn find_items(
        &mut self,
        key: Value,
        items: Vec<Map<String, Value>>,
        limit: Option<i64>,
    ) -> Result<Map<String, Value>> {
        let items = items.into_iter().map(Value::Object).collect();
        self.env("find", key, items, None, limit)
    }

    /// Thẻ một note, hoặc `{"error": ...}` khi không có.
    pub fn note(&self, node: &str) -> Result<Map<String, Value>> {
        Ok(self.g.note(node)?.unwrap_or_else(|| {
            let mut m = Map::new();
            m.insert(
                "error".into(),
                format!("không có note {node}; dùng find").into(),
            );
            m
        }))
    }

    pub fn neighbors(
        &mut self,
        node: &str,
        dir: &str,
        a: &EdgeArgs,
        limit: Option<i64>,
    ) -> Result<Map<String, Value>> {
        let [rels, as_of, statuses] = a.key();
        let key = json!([node, dir, rels, as_of, statuses, a.accepted_only, a.system]);
        self.neighbors_with(node, Dir::parse(dir), &a.filter(), key, limit)
    }

    fn neighbors_with(
        &mut self,
        node: &str,
        dir: Dir,
        f: &Filter,
        key: Value,
        limit: Option<i64>,
    ) -> Result<Map<String, Value>> {
        let mut items = self.g.neighbors(node, dir, f)?;
        let nodes: Vec<String> = items
            .iter()
            .map(|x| py::str_of(&x["node"]))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let ts = self.g.types_of(&nodes)?;
        let mut ids = Vec::with_capacity(items.len());
        for x in &mut items {
            let n = py::str_of(&x["node"]);
            x.insert("type".into(), json!(ts.get(&n).cloned().flatten()));
            ids.push(n);
        }
        let items = items.into_iter().map(Value::Object).collect();
        self.env("neighbors", key, items, Some(ids), limit)
    }

    pub fn reach(
        &mut self,
        node: &str,
        dir: &str,
        max_depth: Option<i64>,
        a: &EdgeArgs,
        limit: Option<i64>,
    ) -> Result<Map<String, Value>> {
        let [rels, as_of, statuses] = a.key();
        let key = json!([
            node,
            rels,
            dir,
            max_depth,
            as_of,
            statuses,
            a.accepted_only,
            a.system
        ]);
        self.reach_with(node, Dir::parse(dir), max_depth, &a.filter(), key, limit)
    }

    fn reach_with(
        &mut self,
        node: &str,
        dir: Dir,
        max_depth: Option<i64>,
        f: &Filter,
        key: Value,
        limit: Option<i64>,
    ) -> Result<Map<String, Value>> {
        let rows = self.g.reach(node, dir, max_depth, f)?;
        let ids: Vec<String> = rows.iter().map(|(i, _)| i.clone()).collect();
        let items = rows
            .into_iter()
            .map(|(i, d)| match d {
                Some(d) => json!({"id": i, "depth": d}),
                None => json!({"id": i}),
            })
            .collect();
        self.env("reach", key, items, Some(ids), limit)
    }

    pub fn paths(
        &mut self,
        frm: &str,
        to: &str,
        max_len: i64,
        a: &EdgeArgs,
        limit: Option<i64>,
    ) -> Result<Map<String, Value>> {
        let [rels, as_of, statuses] = a.key();
        let key = json!([
            frm,
            to,
            rels,
            max_len,
            as_of,
            statuses,
            a.accepted_only,
            a.system
        ]);
        self.paths_with(frm, to, max_len, &a.filter(), key, limit)
    }

    fn paths_with(
        &mut self,
        frm: &str,
        to: &str,
        max_len: i64,
        f: &Filter,
        key: Value,
        limit: Option<i64>,
    ) -> Result<Map<String, Value>> {
        let items = self.g.paths(frm, to, max_len, f, None)?;
        let items = items.into_iter().map(Value::Object).collect();
        self.env("paths", key, items, Some(Vec::new()), limit)
    }

    pub fn loops(
        &mut self,
        node: &str,
        max_len: i64,
        a: &EdgeArgs,
        limit: Option<i64>,
    ) -> Result<Map<String, Value>> {
        let [rels, as_of, statuses] = a.key();
        let key = json!([
            node,
            max_len,
            rels,
            as_of,
            statuses,
            a.accepted_only,
            a.system
        ]);
        self.loops_with(node, max_len, &a.filter(), key, limit)
    }

    fn loops_with(
        &mut self,
        node: &str,
        max_len: i64,
        f: &Filter,
        key: Value,
        limit: Option<i64>,
    ) -> Result<Map<String, Value>> {
        let items = self.g.loops(node, max_len, f)?;
        let mut kinds: Map<String, Value> = Map::new();
        for x in &items {
            let k = py::str_of(&x["kind"]);
            let n = kinds.get(&k).and_then(Value::as_u64).unwrap_or(0) + 1;
            kinds.insert(k, n.into());
        }
        let items = items.into_iter().map(Value::Object).collect();
        let mut out = self.env("loops", key, items, Some(Vec::new()), limit)?;
        out.insert("kinds".into(), Value::Object(kinds));
        Ok(out)
    }

    pub fn provenance(&self, claim: &str) -> Result<Map<String, Value>> {
        self.g.provenance(claim)
    }

    /// Note có mọi cặp khoá = giá trị trong `where_` (giá trị so như `str()` của Python).
    pub fn select(
        &mut self,
        where_: &Map<String, Value>,
        limit: Option<i64>,
        system: bool,
    ) -> Result<Map<String, Value>> {
        let items = self.g.select(where_, system)?;
        let key = json!([where_, system]);
        let ids = items.clone();
        let items = items.into_iter().map(Value::String).collect();
        self.env("select", key, items, Some(ids), limit)
    }

    pub fn valid(
        &mut self,
        type_: &str,
        as_of: &str,
        limit: Option<i64>,
        system: bool,
    ) -> Result<Map<String, Value>> {
        let items = self.g.valid(type_, as_of, system)?;
        self.valid_items(json!([type_, as_of, system]), items, limit)
    }

    fn valid_items(
        &mut self,
        key: Value,
        items: Vec<String>,
        limit: Option<i64>,
    ) -> Result<Map<String, Value>> {
        let ids = items.clone();
        let items = items.into_iter().map(Value::String).collect();
        self.env("valid", key, items, Some(ids), limit)
    }
}

// ------------------------------------------------------------------------------------------------ điểm vào chung

/// Phiên tool của một vault: bộ tool trên index, thư mục vault (đọc phần thân cho `note`), tên vault trong
/// Obsidian (link `obsidian://open`; `None` để tắt).
pub struct Context {
    tools: Tools,
    vault: PathBuf,
    vault_name: Option<String>,
}

/// Tên vault mặc định cho link `obsidian://open`: tên thư mục vault sau `os.path.abspath` (bỏ `.`, `..` theo chữ),
/// `None` khi rỗng (gốc hệ thống file).
pub fn default_vault_name(vault: &Path) -> Option<String> {
    use std::path::Component;
    let abs = std::path::absolute(vault).unwrap_or_else(|_| vault.to_path_buf());
    let mut parts: Vec<Component<'_>> = Vec::new();
    for c in abs.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(parts.last(), Some(Component::Normal(_))) {
                    parts.pop();
                }
            }
            c => parts.push(c),
        }
    }
    match parts.last() {
        Some(Component::Normal(n)) => Some(n.to_string_lossy().into_owned()),
        _ => None,
    }
}

impl Context {
    pub fn new(tools: Tools, vault: &Path, vault_name: Option<String>) -> Context {
        Context {
            tools,
            vault: vault.to_path_buf(),
            vault_name: vault_name.filter(|n| !n.is_empty()),
        }
    }

    pub fn tools(&self) -> &Tools {
        &self.tools
    }

    pub fn tools_mut(&mut self) -> &mut Tools {
        &mut self.tools
    }

    pub fn vault(&self) -> &Path {
        &self.vault
    }

    pub fn vault_name(&self) -> Option<&str> {
        self.vault_name.as_deref()
    }

    /// Link mở note trong Obsidian (vault đã mở trong app, tên = `vault_name`).
    pub fn url(&self, node: &str) -> Option<String> {
        let name = self.vault_name.as_deref()?;
        let file = node.strip_suffix(".md").unwrap_or(node);
        Some(format!(
            "obsidian://open?vault={}&file={}",
            quote(name),
            quote(file)
        ))
    }

    /// `_need`: `None` khi note có trong index; không thì lỗi kèm tối đa 5 note tên gần giống.
    fn need(&self, node: &str) -> Result<Option<Value>> {
        let g = self.tools.graph();
        if !g.has_node(node)? {
            let base = if node.ends_with(".md") {
                let b = basename(node);
                b.strip_suffix(".md").unwrap_or(b)
            } else {
                node
            };
            let hits = g.find(base, None, None)?;
            let fb = fold(base);
            let mut cand: Vec<(bool, usize, String)> = hits
                .iter()
                .map(|x| {
                    let title = x.get("title").map(py::str_of).unwrap_or_default();
                    (
                        fold(&title) != fb,
                        title.chars().count(),
                        py::str_of(&x["id"]),
                    )
                })
                .collect();
            cand.sort();
            let cand: Vec<String> = cand.into_iter().take(5).map(|(_, _, id)| id).collect();
            return Ok(Some(json!({
                "error": format!("no note with id {}; ids are exact vault paths", py::repr(&Value::String(node.to_owned()))),
                "did_you_mean": cand,
            })));
        }
        Ok(None)
    }
}

/// `posixpath.basename` (thêm `\` trên Windows như `ntpath`).
fn basename(p: &str) -> &str {
    let sep = |c: char| c == '/' || (cfg!(windows) && c == '\\');
    p.rsplit(sep).next().unwrap_or(p)
}

/// `urllib.parse.quote(s, safe='')`: giữ chữ, số và `_.-~`, còn lại mã hoá từng byte UTF-8 (`%XX` hoa).
fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Tham số MCP của một lời gọi, đọc như `Server.call`.
struct Args(Map<String, Value>);

impl Args {
    fn get(&self, k: &str) -> &Value {
        self.0.get(k).unwrap_or(&Value::Null)
    }

    /// `a[k]` (KeyError khi thiếu).
    fn req(&self, k: &str) -> Result<&Value> {
        self.0
            .get(k)
            .ok_or_else(|| ToolError::new("KeyError", py::repr(&Value::String(k.to_owned()))))
    }

    /// `a.get(k) or default`.
    fn or(&self, k: &str, default: Value) -> Value {
        let v = self.get(k);
        if py::truthy(v) { v.clone() } else { default }
    }

    fn flag(&self, k: &str) -> bool {
        py::truthy(self.get(k))
    }

    /// Số dùng làm `limit`, `k`, `offset`, `body_chars`: số nguyên hoặc bool; rỗng/0/`false` là `None`.
    fn int_opt(&self, k: &str) -> Result<Option<i64>> {
        index_int(self.get(k), k)
    }

    /// Bộ lọc trên cạnh (`rels`, `as_of`, `statuses`, `accepted_only`, `system`) và phần khoá handle tương ứng.
    fn edge(&self) -> Result<(Filter, [Value; 3])> {
        let rels = py_list(self.get("rels"), "rels")?;
        let statuses = py_list(self.get("statuses"), "statuses")?;
        let as_of = match self.get("as_of") {
            v if !py::truthy(v) => None,
            Value::String(s) => Some(s.clone()),
            v => {
                return Err(type_error(format!(
                    "as_of phải là chuỗi YYYY-MM-DD, không phải {}",
                    py::type_name(v)
                )));
            }
        };
        let f = Filter::new(
            rels.as_deref(),
            as_of.as_deref(),
            statuses.as_deref(),
            self.flag("accepted_only"),
            self.flag("system"),
        );
        Ok((
            f,
            [
                self.get("rels").clone(),
                self.get("as_of").clone(),
                self.get("statuses").clone(),
            ],
        ))
    }
}

/// Số nguyên dùng làm chỉ số (`xs[:n]`): `int` hoặc `bool`; giá trị "sai" là `None`.
fn index_int(v: &Value, k: &str) -> Result<Option<i64>> {
    if !py::truthy(v) {
        return Ok(None);
    }
    match v {
        Value::Bool(b) => Ok(Some(i64::from(*b))),
        Value::Number(n) if n.as_i64().is_some() => Ok(n.as_i64()),
        _ => Err(type_error(format!(
            "{k} phải là số nguyên, không phải {}",
            py::type_name(v)
        ))),
    }
}

/// Số đem so với độ sâu, độ dài (`r.d < ?`, `p.k < ?`): số nguyên, bool, hoặc số thực (làm tròn lên: với số đếm
/// nguyên `k < x` đúng khi và chỉ khi `k < ceil(x)`).
fn bound_int(v: &Value, k: &str) -> Result<Option<i64>> {
    if !py::truthy(v) {
        return Ok(None);
    }
    match v {
        Value::Bool(b) => Ok(Some(i64::from(*b))),
        Value::Number(n) => Ok(Some(match n.as_i64() {
            Some(i) => i,
            None => n.as_f64().unwrap_or(0.0).ceil() as i64,
        })),
        _ => Err(type_error(format!(
            "{k} phải là số nguyên, không phải {}",
            py::type_name(v)
        ))),
    }
}

/// `list(x) if x else None` với danh sách chuỗi: chuỗi thành danh sách ký tự, object thành danh sách khoá như
/// Python.
fn py_list(v: &Value, k: &str) -> Result<Option<Vec<String>>> {
    if !py::truthy(v) {
        return Ok(None);
    }
    match v {
        Value::String(s) => Ok(Some(s.chars().map(String::from).collect())),
        Value::Object(m) => Ok(Some(m.keys().cloned().collect())),
        Value::Array(a) => a
            .iter()
            .map(|x| match x {
                Value::String(s) => Ok(s.clone()),
                _ => Err(type_error(format!(
                    "{k} phải là danh sách chuỗi, có phần tử {}",
                    py::type_name(x)
                ))),
            })
            .collect::<Result<Vec<_>>>()
            .map(Some),
        _ => Err(type_error(format!(
            "'{}' object is not iterable",
            py::type_name(v)
        ))),
    }
}

fn opt_str(v: &Value, k: &str) -> Result<Option<String>> {
    match v {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        _ => Err(type_error(format!(
            "{k} phải là chuỗi, không phải {}",
            py::type_name(v)
        ))),
    }
}

/// `Server.call(name, args)`: một lời gọi tool với tham số MCP, trả JSON như Python trước khi MCP thu gọn
/// (`compact`). Lỗi người gọi hay gặp (note không có, thiếu tham số bắt buộc của `loops`, `select` không lọc gì,
/// handle hết hạn, tool lạ) là kết quả `{"error": ...}` như Python; `Err` là chỗ Python ném ngoại lệ.
pub fn call(ctx: &mut Context, name: &str, args: Value) -> Result<Value> {
    let mut a = match args {
        Value::Object(m) => Args(m),
        Value::Null => Args(Map::new()),
        v => {
            return Err(type_error(format!(
                "arguments phải là object, không phải {}",
                py::type_name(&v)
            )));
        }
    };
    for k in ["node", "from", "to", "claim"] {
        if let Some(v) = a.0.get(k) {
            let s = nfc(&py::str_of(v));
            a.0.insert(k.into(), Value::String(s.clone()));
            if let Some(e) = ctx.need(&s)? {
                return Ok(e);
            }
        }
    }
    let node = |a: &Args, k: &str| -> Result<String> { a.req(k).map(py::str_of) };
    let t = &mut ctx.tools;
    let out = match name {
        "find" => {
            let raw = a.req("name")?;
            let Value::String(n) = raw else {
                return Err(ToolError::new(
                    "AttributeError",
                    format!("'{}' object has no attribute 'strip'", py::type_name(raw)),
                ));
            };
            let ty = a.get("type");
            let items = match ty {
                v if !py::truthy(v) => t.graph().find(n, None, None)?,
                Value::String(s) => t.graph().find(n, Some(s), None)?,
                // Python so `type` khác chuỗi với type của note: không note nào khớp.
                _ => Vec::new(),
            };
            let limit = a.int_opt("limit")?;
            t.find_items(json!([raw, ty]), items, limit)?
        }
        "note" => {
            let n = node(&a, "node")?;
            let mut d = t.note(&n)?;
            let found = !d.contains_key("error");
            if found && let Some(url) = ctx.url(&n) {
                d.insert("url".into(), url.into());
            }
            let chars = match a.0.get("body_chars") {
                None => Some(800),
                Some(v) => index_int(v, "body_chars")?,
            };
            if let Some(n_chars) = chars
                && found
                && let Ok(txt) = crate::vault::read(&ctx.vault.join(&n))
            {
                let body = match FM_RE.find(&txt) {
                    Some(m) => &txt[m.end()..],
                    None => txt.as_str(),
                };
                let body = py::strip(body);
                let cs: Vec<char> = body.chars().collect();
                let r = py::slice(cs.len(), None, Some(n_chars));
                let mut s: String = cs[r].iter().collect();
                if i64::try_from(cs.len()).unwrap_or(i64::MAX) > n_chars {
                    s.push_str(&format!(" … [{} chars]", cs.len()));
                }
                d.insert("body".into(), s.into());
            }
            d
        }
        "neighbors" => {
            let n = node(&a, "node")?;
            let dir = a.or("dir", "both".into());
            let (f, [rels, as_of, statuses]) = a.edge()?;
            let key = json!([
                n,
                dir,
                rels,
                as_of,
                statuses,
                a.flag("accepted_only"),
                a.flag("system")
            ]);
            let d = dir.as_str().map_or(Dir::Both, Dir::parse);
            t.neighbors_with(&n, d, &f, key, a.int_opt("limit")?)?
        }
        "reach" => {
            let n = node(&a, "node")?;
            let dir = a.or("dir", "out".into());
            let (f, [rels, as_of, statuses]) = a.edge()?;
            let depth_raw = a.get("max_depth").clone();
            let depth = bound_int(&depth_raw, "max_depth")?;
            let key = json!([
                n,
                rels,
                dir,
                depth_raw,
                as_of,
                statuses,
                a.flag("accepted_only"),
                a.flag("system")
            ]);
            let d = dir.as_str().map_or(Dir::Both, Dir::parse);
            t.reach_with(&n, d, depth, &f, key, a.int_opt("limit")?)?
        }
        "paths" => {
            let frm = node(&a, "from")?;
            let to = node(&a, "to")?;
            let max_len_raw = a.or("max_len", 6.into());
            let max_len = bound_int(&max_len_raw, "max_len")?.unwrap_or(6);
            let limit = index_int(&a.or("k", 10.into()), "k")?;
            let (f, [rels, as_of, statuses]) = a.edge()?;
            let key = json!([
                frm,
                to,
                rels,
                max_len_raw,
                as_of,
                statuses,
                a.flag("accepted_only"),
                a.flag("system")
            ]);
            t.paths_with(&frm, &to, max_len, &f, key, limit)?
        }
        "loops" => {
            if !py::truthy(a.get("max_len")) {
                return Ok(json!({"error": "loops needs max_len (e.g. 6)"}));
            }
            let n = node(&a, "node")?;
            let max_len = py::int(a.get("max_len")).map_err(|(k, m)| ToolError::new(k, m))?;
            let (f, [rels, as_of, statuses]) = a.edge()?;
            let key = json!([
                n,
                max_len,
                rels,
                as_of,
                statuses,
                a.flag("accepted_only"),
                a.flag("system")
            ]);
            t.loops_with(&n, max_len, &f, key, a.int_opt("limit")?)?
        }
        "provenance" => {
            let c = node(&a, "claim")?;
            t.provenance(&c)?
        }
        "select" => {
            let mut w = match a.get("where") {
                v if !py::truthy(v) => Map::new(),
                Value::Object(m) => m.clone(),
                v => {
                    return Err(type_error(format!(
                        "where phải là object, không phải {}",
                        py::type_name(v)
                    )));
                }
            };
            for k in ["type", "status", "proposed_by"] {
                if a.flag(k) {
                    w.insert(k.into(), a.get(k).clone());
                }
            }
            if w.is_empty() {
                return Ok(
                    json!({"error": "select needs at least one filter (type, status, proposed_by or where)"}),
                );
            }
            let limit = a.int_opt("limit")?;
            t.select(&w, limit, a.flag("system"))?
        }
        "valid" => {
            let ty_raw = a.req("type")?.clone();
            let as_of_raw = a.req("as_of")?.clone();
            let ty = opt_str(&ty_raw, "type")?;
            let as_of = opt_str(&as_of_raw, "as_of")?;
            let system = a.flag("system");
            let items = match (ty, as_of) {
                (Some(ty), Some(as_of)) => t.graph().valid(&ty, &as_of, system)?,
                // So với NULL trong SQL: không note nào khớp.
                _ => Vec::new(),
            };
            let limit = a.int_opt("limit")?;
            t.valid_items(json!([ty_raw, as_of_raw, system]), items, limit)?
        }
        "related" => return related(ctx, &a),
        "more" => {
            // Như `Tools.more`: tra handle trước, rồi mới dùng `offset`, `limit`.
            let h = match a.req("handle")? {
                Value::String(h) if t.cache.contains_key(h) => h.clone(),
                v @ (Value::Array(_) | Value::Object(_)) => {
                    return Err(type_error(format!(
                        "unhashable type: '{}'",
                        py::type_name(v)
                    )));
                }
                v => {
                    let h = py::str_of(v);
                    return Ok(
                        json!({"error": format!("không còn kết quả {h}; gọi lại tool gốc")}),
                    );
                }
            };
            let echo = a.or("offset", 0.into());
            let offset = index_int(&echo, "offset")?.unwrap_or(0);
            let limit = a.int_opt("limit")?;
            t.more_echo(&h, offset, echo, limit)
        }
        _ => return Ok(json!({"error": format!("unknown tool {name}")})),
    };
    Ok(Value::Object(out))
}

/// Chỗ của tool `related` (PageRank cá nhân hoá từ hit OHS): chưa có trong bản Rust.
fn related(_ctx: &mut Context, _a: &Args) -> Result<Value> {
    Err(ToolError::new(
        "NotImplementedError",
        "tool related chưa có trong bản Rust",
    ))
}
