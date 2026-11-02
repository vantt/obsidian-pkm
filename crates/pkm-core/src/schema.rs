//! `schema.yaml` (v3) cộng các gói bật cho vault -> các bảng `schema_*` của index. View của validator đọc các bảng
//! này, nên sửa `schema.yaml` là đổi luật kiểm mà không sửa code.
//!
//! Đặc tả: `scripts/kg/schema.py` ở sha ghim. Schema của repo đặc tả được nhúng nguyên văn ([`EMBEDDED`]); vẫn nhận
//! file ngoài ([`Source::File`]). File đọc bằng lớp YAML của `vault` với đủ resolver của `CSafeLoader` (ngày thành
//! `date`), như `schema.read_schema`.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;

use crate::vault::yaml::{self, Doc, Loader, Obj, Val};

/// `docs/knowledge-graph/schema/schema.yaml` của repo đặc tả ở sha ghim, chép nguyên từng byte.
pub const EMBEDDED: &str = include_str!("schema/schema.yaml");

/// DDL của các bảng `schema_*`, chép nguyên từ `schema.py`.
pub const DDL: &str = "
CREATE TABLE IF NOT EXISTS schema_node_type(name TEXT PRIMARY KEY, pack TEXT, extends TEXT);
CREATE TABLE IF NOT EXISTS schema_type_ancestor(type TEXT, ancestor TEXT, PRIMARY KEY(type, ancestor));
CREATE TABLE IF NOT EXISTS schema_enum(name TEXT, value TEXT);
CREATE TABLE IF NOT EXISTS schema_edge(key TEXT PRIMARY KEY, pack TEXT, kind TEXT, grp TEXT, inverse TEXT, cardinality TEXT,
  symmetric INTEGER, polarity TEXT, human_only INTEGER, block_ref INTEGER);
CREATE TABLE IF NOT EXISTS schema_link(key TEXT, owner TEXT, src TEXT, dst TEXT);
CREATE TABLE IF NOT EXISTS schema_claim_rel(rel TEXT, src TEXT, dst TEXT);
CREATE TABLE IF NOT EXISTS schema_claim_end(relation TEXT PRIMARY KEY, from_field TEXT, to_field TEXT);
CREATE TABLE IF NOT EXISTS schema_field(owner TEXT, field TEXT, enum_name TEXT, value_type TEXT, pattern TEXT,
  required INTEGER, cardinality TEXT);
CREATE TABLE IF NOT EXISTS schema_required_if(owner TEXT, cond_key TEXT, cond_value TEXT, field TEXT);
CREATE TABLE IF NOT EXISTS schema_excludes(rel TEXT, other TEXT);
CREATE TABLE IF NOT EXISTS schema_problems(detail TEXT);
";

/// Tên các bảng `schema_*`, theo thứ tự của `schema.TABLES`.
pub const TABLES: [&str; 11] = [
    "schema_node_type",
    "schema_type_ancestor",
    "schema_enum",
    "schema_edge",
    "schema_link",
    "schema_claim_rel",
    "schema_claim_end",
    "schema_field",
    "schema_required_if",
    "schema_excludes",
    "schema_problems",
];

/// Lỗi khi đọc hoặc gộp schema (chỗ bản Python ném ngoại lệ và dừng build).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaError(pub String);

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SchemaError {}

fn bad<T>(msg: impl Into<String>) -> Result<T, SchemaError> {
    Err(SchemaError(msg.into()))
}

/// Nơi lấy `schema.yaml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Schema nhúng (mặc định, như `schema.SCHEMA_YAML`).
    Embedded,
    File(PathBuf),
}

impl Source {
    /// `None` là schema nhúng (tham số `path=None` của Python).
    pub fn from_path(path: Option<&Path>) -> Self {
        path.map_or(Source::Embedded, |p| Source::File(p.to_path_buf()))
    }

    fn bytes(&self) -> Result<Vec<u8>, SchemaError> {
        match self {
            Source::Embedded => Ok(EMBEDDED.as_bytes().to_vec()),
            Source::File(p) => std::fs::read(p)
                .map_err(|e| SchemaError(format!("không đọc được schema {}: {e}", p.display()))),
        }
    }

    /// Nội dung file, UTF-8 chặt như `open(path, encoding='utf-8')`.
    pub fn text(&self) -> Result<String, SchemaError> {
        String::from_utf8(self.bytes()?)
            .map_err(|e| SchemaError(format!("schema không phải UTF-8 hợp lệ: {e}")))
    }
}

/// `schema.read_schema`: tài liệu YAML thô (MCP đọc mô tả type từ đây).
pub fn read_schema(src: &Source) -> Result<Doc, SchemaError> {
    let text = src.text()?;
    match yaml::load(&text, Loader::Safe) {
        Ok(Some(doc)) => Ok(doc),
        Ok(None) => bad("schema rỗng"),
        Err(e) => bad(format!("schema YAML lỗi: {e}")),
    }
}

/// `schema.digest`: 12 ký tự đầu của sha1 nội dung file.
pub fn digest(src: &Source) -> Result<String, SchemaError> {
    let hex = sha1_smol::Sha1::from(src.bytes()?).digest().to_string();
    Ok(hex[..12].to_owned())
}

/// Một type node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeType {
    /// Gói khai báo type, `None` với type lõi.
    pub pack: Option<String>,
    pub extends: Option<String>,
}

/// Cạnh của schema: `edge` được ghi trong frontmatter, `fact` chỉ ghi dưới dạng claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    Edge,
    Fact,
}

impl EdgeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EdgeKind::Edge => "edge",
            EdgeKind::Fact => "fact",
        }
    }
}

/// Một cạnh (gồm cả fact của gói) sau khi gộp gói và nới `widen`.
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub pack: Option<String>,
    pub kind: EdgeKind,
    pub inverse: Option<String>,
    pub cardinality: Option<String>,
    pub symmetric: bool,
    pub polarity: Option<String>,
    pub human_only: bool,
    /// `block_ref: allowed`.
    pub block_ref: bool,
    pub domain: Vec<String>,
    pub range: Vec<String>,
    pub excludes_with: Vec<String>,
}

/// Một trường của type (`owner = "*"`: mọi note).
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub owner: String,
    pub field: String,
    /// `enum`, hoặc `relation` khi trường lấy giá trị từ tên cạnh của nhóm (`enum_of_edges`).
    pub enum_name: Option<String>,
    pub value_type: Option<String>,
    pub pattern: Option<String>,
    pub required: bool,
    pub cardinality: Option<String>,
    pub link_to: Vec<String>,
    pub enum_of_edges: Option<Vec<String>>,
}

/// `required_if`: khi mọi điều kiện `when` đúng thì cần các trường `then`.
#[derive(Debug, Clone, PartialEq)]
pub struct RequiredIf {
    pub owner: String,
    /// `(khoá, giá trị dạng str())`.
    pub when: Vec<(String, String)>,
    pub then: Vec<String>,
}

/// Schema đã gộp lõi và các gói (`schema.resolve`), cùng hai đầu của cạnh claim (`claim_ends` của `schema.load`).
#[derive(Debug, Clone, PartialEq)]
pub struct Schema {
    pub types: IndexMap<String, NodeType>,
    pub enums: IndexMap<String, Vec<String>>,
    pub edges: IndexMap<String, Edge>,
    pub groups: IndexMap<String, Vec<String>>,
    /// Nhóm của mỗi cạnh (nhóm sau cùng chứa nó).
    pub group_of: HashMap<String, String>,
    pub fields: Vec<Field>,
    pub required_if: Vec<RequiredIf>,
    /// Lỗi của chính schema (luật R14).
    pub problems: Vec<String>,
    /// `str(schema_version)` (`"None"` khi không ghi).
    pub version: String,
    /// Quan hệ claim -> (trường nguồn, trường đích): `cause`/`effect`, `subject`/`object`.
    pub claim_ends: IndexMap<String, (String, String)>,
}

/// Một ô của bảng SQLite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    Null,
    Int(i64),
    Text(String),
}

impl From<Option<String>> for Cell {
    fn from(v: Option<String>) -> Self {
        v.map_or(Cell::Null, Cell::Text)
    }
}

impl From<String> for Cell {
    fn from(v: String) -> Self {
        Cell::Text(v)
    }
}

impl From<&str> for Cell {
    fn from(v: &str) -> Self {
        Cell::Text(v.to_owned())
    }
}

impl From<bool> for Cell {
    fn from(v: bool) -> Self {
        Cell::Int(i64::from(v))
    }
}

/// Các dòng của một bảng `schema_*`, theo thứ tự cột của DDL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub name: &'static str,
    pub columns: &'static [&'static str],
    pub rows: Vec<Vec<Cell>>,
}

// -------------------------------------------------------------------------------------------------------------
// Đọc giá trị YAML theo cách schema.py dùng chúng.

/// Tên (type, cạnh, nhóm, trường, gói): chuỗi, hoặc số nguyên ghi như chuỗi thập phân.
fn name(v: Val<'_>, what: &str) -> Result<String, SchemaError> {
    match v.obj() {
        Obj::Str(s) => Ok(s.clone()),
        Obj::Int(s) => Ok(s.clone()),
        _ => bad(format!(
            "schema: {what} phải là chuỗi, gặp {}",
            v.py_repr().unwrap_or_default()
        )),
    }
}

fn names(v: Option<Val<'_>>, what: &str) -> Result<Vec<String>, SchemaError> {
    match v {
        None => Ok(Vec::new()),
        Some(v) => list(v, what)?.map(|x| name(x, what)).collect(),
    }
}

fn list<'a>(v: Val<'a>, what: &str) -> Result<impl Iterator<Item = Val<'a>> + 'a, SchemaError> {
    v.list()
        .ok_or_else(|| SchemaError(format!("schema: {what} phải là danh sách")))
}

fn items<'a>(
    v: Val<'a>,
    what: &str,
) -> Result<impl Iterator<Item = (Val<'a>, Val<'a>)> + 'a, SchemaError> {
    v.items()
        .ok_or_else(|| SchemaError(format!("schema: {what} phải là bảng khoá")))
}

fn dict(v: Val<'_>, what: &str) -> Result<(), SchemaError> {
    if v.is_dict() {
        Ok(())
    } else {
        bad(format!("schema: {what} phải là bảng khoá"))
    }
}

/// `d.get(key, {})` lặp theo cặp: thiếu khoá là rỗng, có mà không phải bảng khoá là lỗi.
fn items_or_empty<'a>(
    d: Val<'a>,
    key: &str,
    what: &str,
) -> Result<Vec<(Val<'a>, Val<'a>)>, SchemaError> {
    match d.get(key) {
        None => Ok(Vec::new()),
        Some(v) => Ok(items(v, what)?.collect()),
    }
}

fn required<'a>(d: Val<'a>, key: &str, what: &str) -> Result<Val<'a>, SchemaError> {
    d.get(key)
        .ok_or_else(|| SchemaError(format!("schema: {what} thiếu khoá {key:?}")))
}

/// Giá trị ghi vào cột TEXT của SQLite qua `sqlite3` của Python: `None` là NULL, `bool` là 1/0, số ghi dạng chữ,
/// ngày qua bộ chuyển mặc định (`isoformat`, ngày giờ cách bằng dấu cách).
fn cell(v: Option<Val<'_>>) -> Result<Option<String>, SchemaError> {
    let Some(v) = v else { return Ok(None) };
    Ok(Some(match v.obj() {
        Obj::None => return Ok(None),
        Obj::Str(s) | Obj::Int(s) => s.clone(),
        Obj::Bool(b) => if *b { "1" } else { "0" }.into(),
        Obj::Float(f) => sqlite_real_text(*f),
        Obj::Date(_) | Obj::DateTime(_) => v.py_str().map_err(|e| SchemaError(e.to_string()))?,
        _ => {
            return bad(format!(
                "schema: giá trị {} không ghi được vào SQLite",
                v.py_repr().unwrap_or_default()
            ));
        }
    }))
}

/// Số thực ghi vào cột TEXT: SQLite dùng `%!.15g`.
fn sqlite_real_text(f: f64) -> String {
    if f.is_nan() {
        return String::new();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Inf" } else { "-Inf" }.into();
    }
    let sci = format!("{f:.14e}");
    let (mant, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let trim = |s: &str| -> String {
        let s = s.trim_end_matches('0');
        if s.ends_with('.') {
            format!("{s}0")
        } else if s.contains('.') {
            s.to_owned()
        } else {
            format!("{s}.0")
        }
    };
    if !(-4..15).contains(&exp) {
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{}e{sign}{:02}", trim(mant), exp.unsigned_abs())
    } else {
        let decimals = usize::try_from(14 - exp).unwrap_or(0);
        trim(&format!("{f:.decimals$}"))
    }
}

fn field(owner: &str, f: String, fs: Option<Val<'_>>, req: bool) -> Result<Field, SchemaError> {
    let what = format!("trường {owner}.{f}");
    let fs = match fs {
        None => None,
        Some(v) if v.is_dict() => Some(v),
        Some(_) => return bad(format!("schema: {what} phải là bảng khoá")),
    };
    let get = |k: &str| fs.and_then(|d| d.get(k));
    let enum_of_edges = match fs.and_then(|d| {
        d.items()?
            .find(|(k, _)| k.as_str() == Some("enum_of_edges"))
    }) {
        Some((_, v)) => Some(names(Some(v), &format!("{what}.enum_of_edges"))?),
        None => None,
    };
    let enum_name = match get("enum").filter(Val::truthy) {
        Some(v) => cell(Some(v))?,
        None => enum_of_edges.as_ref().map(|_| "relation".to_owned()),
    };
    Ok(Field {
        owner: owner.to_owned(),
        enum_name,
        value_type: cell(get("type"))?,
        pattern: cell(get("pattern"))?,
        required: req,
        cardinality: cell(get("cardinality"))?,
        link_to: names(get("link_to"), &format!("{what}.link_to"))?,
        enum_of_edges,
        field: f,
    })
}

fn edge(k: &str, v: Val<'_>, pack: Option<&str>, kind: EdgeKind) -> Result<Edge, SchemaError> {
    let what = format!("cạnh {k}");
    dict(v, &what)?;
    let truthy = |key: &str| v.get(key).is_some_and(|x| x.truthy());
    Ok(Edge {
        pack: pack.map(str::to_owned),
        kind,
        inverse: cell(v.get("inverse"))?,
        cardinality: cell(v.get("cardinality"))?,
        symmetric: truthy("symmetric"),
        polarity: cell(v.get("polarity"))?,
        human_only: truthy("human_only"),
        block_ref: v.get("block_ref").and_then(|x| x.as_str()) == Some("allowed"),
        domain: names(
            Some(required(v, "domain", &what)?),
            &format!("{what}.domain"),
        )?,
        range: names(Some(required(v, "range", &what)?), &format!("{what}.range"))?,
        excludes_with: names(v.get("excludes_with"), &format!("{what}.excludes_with"))?,
    })
}

/// `schema.resolve`: gộp lõi và các gói thành một schema phẳng, kèm lỗi của chính schema (R14).
pub fn resolve(doc: &Doc, packs: &[&str]) -> Result<Schema, SchemaError> {
    let s = doc.root();
    if !s.is_dict() {
        return bad("schema: gốc của schema.yaml phải là bảng khoá");
    }
    let all_packs = match s.get("packs") {
        None => IndexMap::new(),
        Some(p) => items(p, "packs")?
            .map(|(k, v)| Ok((name(k, "tên gói")?, v)))
            .collect::<Result<IndexMap<_, _>, SchemaError>>()?,
    };
    let unknown: Vec<&str> = packs
        .iter()
        .copied()
        .filter(|p| !all_packs.contains_key(*p))
        .collect();
    if !unknown.is_empty() {
        return bad(format!(
            "gói không có trong schema.yaml: {}",
            unknown.join(", ")
        ));
    }

    let mut enums = IndexMap::new();
    let enum_values = |v: Val<'_>, k: &str| -> Result<Vec<String>, SchemaError> {
        list(v, &format!("enum {k}"))?
            .map(|x| x.py_str().map_err(|e| SchemaError(e.to_string())))
            .collect()
    };
    for (k, v) in items(required(s, "enums", "schema")?, "enums")? {
        let k = name(k, "tên enum")?;
        let vals = enum_values(v, &k)?;
        enums.insert(k, vals);
    }
    let mut groups: IndexMap<String, Vec<String>> = IndexMap::new();
    for (g, v) in items(required(s, "edge_groups", "schema")?, "edge_groups")? {
        let g = name(g, "tên nhóm cạnh")?;
        let es = names(Some(v), &format!("nhóm {g}"))?;
        groups.insert(g, es);
    }
    let node_types: Vec<(String, Val<'_>)> =
        items(required(s, "node_types", "schema")?, "node_types")?
            .map(|(k, v)| Ok((name(k, "tên type")?, v)))
            .collect::<Result<_, SchemaError>>()?;
    let mut types: IndexMap<String, NodeType> = IndexMap::new();
    for (k, v) in &node_types {
        let extends = match v.get("extends") {
            Some(x) if !x.is_none() => Some(name(x, &format!("{k}.extends"))?),
            _ => None,
        };
        types.insert(
            k.clone(),
            NodeType {
                pack: None,
                extends,
            },
        );
    }
    let mut edges: IndexMap<String, Edge> = IndexMap::new();
    for (k, v) in items(required(s, "edges", "schema")?, "edges")? {
        let k = name(k, "tên cạnh")?;
        let e = edge(&k, v, None, EdgeKind::Edge)?;
        edges.insert(k, e);
    }

    let mut fields = Vec::new();
    let pc = required(s, "properties_common", "schema")?;
    let pc_required = names(pc.get("required"), "properties_common.required")?;
    let pc_fields = pc.get("fields");
    for f in names(pc.get("optional"), "properties_common.optional")?
        .into_iter()
        .chain(pc_required.iter().cloned())
    {
        let fs = spec_of(pc_fields, &f)?;
        let req = pc_required.contains(&f);
        fields.push(field("*", f, fs, req)?);
    }
    let mut required_if = Vec::new();
    for (t, spec) in &node_types {
        let spec = Some(*spec).filter(Val::truthy);
        if let Some(sp) = spec
            && !sp.is_dict()
        {
            return bad(format!("schema: type {t} phải là bảng khoá"));
        }
        let get = |k: &str| spec.and_then(|d| d.get(k));
        let req = names(get("required"), &format!("{t}.required"))?;
        let mut seen = HashSet::new();
        let mut fnames = Vec::new();
        let declared = match get("fields") {
            Some(d) => items(d, &format!("{t}.fields"))?
                .map(|(k, _)| name(k, &format!("{t}.fields")))
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };
        for f in names(get("optional"), &format!("{t}.optional"))?
            .into_iter()
            .chain(req.iter().cloned())
            .chain(declared)
        {
            if seen.insert(f.clone()) {
                fnames.push(f);
            }
        }
        for f in fnames {
            let fs = spec_of(get("fields"), &f)?;
            let r = req.contains(&f);
            fields.push(field(t, f, fs, r)?);
        }
        if let Some(rs) = get("required_if") {
            for r in list(rs, &format!("{t}.required_if"))? {
                let when = items(required(r, "when", &format!("{t}.required_if"))?, "when")?
                    .map(|(k, v)| {
                        Ok((
                            name(k, "khoá của when")?,
                            v.py_str().map_err(|e| SchemaError(e.to_string()))?,
                        ))
                    })
                    .collect::<Result<Vec<_>, SchemaError>>()?;
                let then = names(
                    Some(required(r, "then", &format!("{t}.required_if"))?),
                    "then",
                )?;
                required_if.push(RequiredIf {
                    owner: t.clone(),
                    when,
                    then,
                });
            }
        }
    }

    let mut widen: Vec<(String, Vec<String>, Vec<String>)> = Vec::new();
    for &pname in packs {
        let p = all_packs[pname];
        if !p.is_dict() {
            return bad(format!("schema: gói {pname} phải là bảng khoá"));
        }
        for (k, v) in items_or_empty(p, "enums", &format!("{pname}.enums"))? {
            let k = name(k, "tên enum")?;
            let vals = enum_values(v, &k)?;
            enums.insert(k, vals);
        }
        for (t, spec) in items_or_empty(p, "node_types", &format!("{pname}.node_types"))? {
            let t = name(t, "tên type")?;
            if !spec.is_dict() {
                return bad(format!(
                    "schema: type {t} của gói {pname} phải là bảng khoá"
                ));
            }
            let extends = match spec.get("extends") {
                Some(x) if !x.is_none() => Some(name(x, &format!("{t}.extends"))?),
                _ => None,
            };
            types.insert(
                t.clone(),
                NodeType {
                    pack: Some(pname.to_owned()),
                    extends,
                },
            );
            if let Some(fd) = spec.get("fields").filter(Val::truthy) {
                for (f, fs) in items(fd, &format!("{t}.fields"))? {
                    let f = name(f, "tên trường")?;
                    fields.push(field(&t, f, Some(fs), false)?);
                }
            }
        }
        for (f, fs) in items_or_empty(p, "properties", &format!("{pname}.properties"))? {
            let f = name(f, "tên trường")?;
            fields.push(field("*", f, Some(fs), false)?);
        }
        for (t, fd) in items_or_empty(p, "node_fields", &format!("{pname}.node_fields"))? {
            let t = name(t, "tên type")?;
            for (f, fs) in items(fd, &format!("{pname}.node_fields.{t}"))? {
                let f = name(f, "tên trường")?;
                fields.push(field(&t, f, Some(fs), false)?);
            }
        }
        for (k, v) in items_or_empty(p, "edges", &format!("{pname}.edges"))? {
            let k = name(k, "tên cạnh")?;
            let e = edge(&k, v, Some(pname), EdgeKind::Edge)?;
            edges.insert(k.clone(), e);
            if let Some(g) = v.get("group").filter(Val::truthy) {
                groups.entry(name(g, "tên nhóm cạnh")?).or_default().push(k);
            }
        }
        for (k, v) in items_or_empty(p, "facts", &format!("{pname}.facts"))? {
            let k = name(k, "tên cạnh")?;
            let e = edge(&k, v, Some(pname), EdgeKind::Fact)?;
            edges.insert(k.clone(), e);
            groups.entry("facts".into()).or_default().push(k);
        }
        for (k, w) in items_or_empty(p, "widen", &format!("{pname}.widen"))? {
            let k = name(k, "tên cạnh")?;
            let what = format!("{pname}.widen.{k}");
            dict(w, &what)?;
            widen.push((
                k,
                names(w.get("domain"), &format!("{what}.domain"))?,
                names(w.get("range"), &format!("{what}.range"))?,
            ));
        }
        if let Some(lt) = p.get("lifecycle_types") {
            for t in names(Some(lt), &format!("{pname}.lifecycle_types"))? {
                let base = Field {
                    owner: t.clone(),
                    field: String::new(),
                    enum_name: None,
                    value_type: None,
                    pattern: None,
                    required: false,
                    cardinality: None,
                    link_to: Vec::new(),
                    enum_of_edges: None,
                };
                for (f, ty, en) in [
                    ("valid_from", Some("date"), None),
                    ("valid_to", Some("date"), None),
                    ("status", None, Some("status")),
                ] {
                    fields.push(Field {
                        field: f.into(),
                        value_type: ty.map(str::to_owned),
                        enum_name: en.map(str::to_owned),
                        ..base.clone()
                    });
                }
            }
        }
    }
    for (k, d, r) in widen {
        if let Some(e) = edges.get_mut(&k) {
            e.domain.extend(d);
            e.range.extend(r);
        }
    }
    let group_of = groups
        .iter()
        .flat_map(|(g, es)| es.iter().map(move |e| (e.clone(), g.clone())))
        .collect();

    let mut problems = Vec::new();
    for (t, v) in &types {
        let mut seen = HashSet::from([t.as_str()]);
        let mut cur = v.extends.as_deref();
        while let Some(c) = cur.filter(|c| !c.is_empty()) {
            let Some(next) = types.get(c) else {
                problems.push(format!("extends {t} -> {c}: không có type này"));
                break;
            };
            if !seen.insert(c) {
                problems.push(format!("extends của {t} có vòng"));
                break;
            }
            cur = next.extends.as_deref();
        }
    }
    for (k, e) in &edges {
        for (side, list) in [("domain", &e.domain), ("range", &e.range)] {
            for t in list {
                if t != "*" && t != "same" && !types.contains_key(t) {
                    problems.push(format!("{k}.{side}: type {t} không có (gói chưa bật?)"));
                }
            }
        }
        for x in &e.excludes_with {
            if !edges.contains_key(x) {
                problems.push(format!("{k}.excludes_with: không có cạnh {x}"));
            }
        }
    }

    let mut claim_ends = IndexMap::new();
    for r in &required_if {
        for (ck, cv) in &r.when {
            if ck == "relation_group" && r.then.len() == 2 {
                for v in groups.get(cv).into_iter().flatten() {
                    claim_ends.insert(v.clone(), (r.then[0].clone(), r.then[1].clone()));
                }
            }
        }
    }

    let version = match s.get("schema_version") {
        Some(v) => v.py_str().map_err(|e| SchemaError(e.to_string()))?,
        None => "None".into(),
    };
    Ok(Schema {
        types,
        enums,
        edges,
        groups,
        group_of,
        fields,
        required_if,
        problems,
        version,
        claim_ends,
    })
}

/// `fields.get(f, {})`: đặc tả của một trường (`None` khi không khai).
fn spec_of<'a>(fields: Option<Val<'a>>, f: &str) -> Result<Option<Val<'a>>, SchemaError> {
    match fields {
        None => Ok(None),
        Some(d) if d.is_dict() => Ok(d.get(f)),
        Some(_) => bad(format!("schema: fields chứa {f} phải là bảng khoá")),
    }
}

/// `schema.ancestors`: type, cha, ông, ... (dừng khi gặp type không có hoặc vòng).
pub fn ancestors<'a>(types: &'a IndexMap<String, NodeType>, t: &'a str) -> Vec<&'a str> {
    let mut out = vec![t];
    let mut cur = types.get(t).and_then(|v| v.extends.as_deref());
    while let Some(c) = cur.filter(|c| !c.is_empty() && types.contains_key(*c) && !out.contains(c))
    {
        out.push(c);
        cur = types[c].extends.as_deref();
    }
    out
}

/// Các dòng của bảng `schema_*` như `schema.load` ghi (bảng có khoá chính thì bỏ dòng trùng như `INSERT OR
/// IGNORE`; bảng còn lại giữ nguyên).
pub fn tables(s: &Schema) -> Vec<Table> {
    let mut node_type = Vec::new();
    let mut ancestor = Vec::new();
    let mut seen_ancestor = HashSet::new();
    for (t, v) in &s.types {
        node_type.push(vec![
            t.as_str().into(),
            v.pack.clone().into(),
            v.extends.clone().into(),
        ]);
        for a in ancestors(&s.types, t) {
            if seen_ancestor.insert((t.as_str(), a)) {
                ancestor.push(vec![t.as_str().into(), a.into()]);
            }
        }
    }
    let mut enum_rows: Vec<Vec<Cell>> = s
        .enums
        .iter()
        .flat_map(|(k, vs)| {
            vs.iter()
                .map(move |x| vec![k.as_str().into(), x.as_str().into()])
        })
        .collect();
    for f in &s.fields {
        for g in f.enum_of_edges.iter().flatten() {
            for e in s.groups.get(g).into_iter().flatten() {
                enum_rows.push(vec!["relation".into(), e.as_str().into()]);
            }
        }
    }
    let (mut edge_rows, mut link, mut claim_rel, mut excludes) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (k, v) in &s.edges {
        edge_rows.push(vec![
            k.as_str().into(),
            v.pack.clone().into(),
            v.kind.as_str().into(),
            s.group_of.get(k).cloned().into(),
            v.inverse.clone().into(),
            v.cardinality.clone().into(),
            v.symmetric.into(),
            v.polarity.clone().into(),
            v.human_only.into(),
            v.block_ref.into(),
        ]);
        for a in &v.domain {
            for b in &v.range {
                if v.kind == EdgeKind::Edge {
                    link.push(vec![
                        k.as_str().into(),
                        "*".into(),
                        a.as_str().into(),
                        b.as_str().into(),
                    ]);
                }
                claim_rel.push(vec![
                    k.as_str().into(),
                    a.as_str().into(),
                    b.as_str().into(),
                ]);
            }
        }
        for x in &v.excludes_with {
            excludes.push(vec![k.as_str().into(), x.as_str().into()]);
        }
    }
    let mut field_rows = Vec::new();
    for f in &s.fields {
        field_rows.push(vec![
            f.owner.as_str().into(),
            f.field.as_str().into(),
            f.enum_name.clone().into(),
            f.value_type.clone().into(),
            f.pattern.clone().into(),
            f.required.into(),
            f.cardinality.clone().into(),
        ]);
        let owner = || Cell::from(f.owner.as_str());
        for t in &f.link_to {
            link.push(vec![
                f.field.as_str().into(),
                owner(),
                owner(),
                t.as_str().into(),
            ]);
        }
        if f.field == "source" && !f.link_to.is_empty() && f.owner != "*" {
            for t in s
                .edges
                .get("source")
                .map(|e| e.range.as_slice())
                .unwrap_or_default()
            {
                link.push(vec![
                    f.field.as_str().into(),
                    owner(),
                    owner(),
                    t.as_str().into(),
                ]);
            }
        }
    }
    let mut required_if = Vec::new();
    for r in &s.required_if {
        for (ck, cv) in &r.when {
            let (key, vals): (&str, Vec<&str>) = if ck == "relation_group" {
                (
                    "relation",
                    s.groups
                        .get(cv)
                        .into_iter()
                        .flatten()
                        .map(String::as_str)
                        .collect(),
                )
            } else {
                (ck.as_str(), vec![cv.as_str()])
            };
            for v in vals {
                for f in &r.then {
                    required_if.push(vec![
                        r.owner.as_str().into(),
                        key.into(),
                        v.into(),
                        f.as_str().into(),
                    ]);
                }
            }
        }
    }
    let claim_end = s
        .claim_ends
        .iter()
        .map(|(r, (a, b))| vec![r.as_str().into(), a.as_str().into(), b.as_str().into()])
        .collect();
    let problems = s.problems.iter().map(|p| vec![p.as_str().into()]).collect();
    let t = |name, columns, rows| Table {
        name,
        columns,
        rows,
    };
    vec![
        t("schema_node_type", &["name", "pack", "extends"], node_type),
        t("schema_type_ancestor", &["type", "ancestor"], ancestor),
        t("schema_enum", &["name", "value"], enum_rows),
        t(
            "schema_edge",
            &[
                "key",
                "pack",
                "kind",
                "grp",
                "inverse",
                "cardinality",
                "symmetric",
                "polarity",
                "human_only",
                "block_ref",
            ],
            edge_rows,
        ),
        t("schema_link", &["key", "owner", "src", "dst"], link),
        t("schema_claim_rel", &["rel", "src", "dst"], claim_rel),
        t(
            "schema_claim_end",
            &["relation", "from_field", "to_field"],
            claim_end,
        ),
        t(
            "schema_field",
            &[
                "owner",
                "field",
                "enum_name",
                "value_type",
                "pattern",
                "required",
                "cardinality",
            ],
            field_rows,
        ),
        t(
            "schema_required_if",
            &["owner", "cond_key", "cond_value", "field"],
            required_if,
        ),
        t("schema_excludes", &["rel", "other"], excludes),
        t("schema_problems", &["detail"], problems),
    ]
}

/// `schema.load` không có phần SQLite: đọc schema, gộp các gói, trả schema và các dòng của bảng `schema_*`.
pub fn load(src: &Source, packs: &[&str]) -> Result<(Schema, Vec<Table>), SchemaError> {
    let s = resolve(&read_schema(src)?, packs)?;
    let t = tables(&s);
    Ok((s, t))
}

#[cfg(test)]
mod tests;
