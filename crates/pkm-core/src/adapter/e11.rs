//! Adapter bản B (vault xưởng E11). Đặc tả: `scripts/e11/kg_adapter.py` ở sha ghim.
//!
//! Lõi đọc cạnh có trong note (frontmatter, wikilink thân note). Phần riêng của bản B nằm ở đây:
//!
//! - thư mục: note thuộc trang chủ đề của thư mục gần nhất có trang `<thư mục> (chủ đề).md` (type toc): rel
//!   `topic`, `accepted`, origin `folder`;
//! - đề xuất trong `_Đề xuất/` (type proposal): `related`, `same-topic` -> `related`; `duplicate` -> `duplicate_of`
//!   (từ bản sau về bản giữ); `person` -> `who` tới `Người/<tên>`; `fix-link` -> `link` tới note đúng. `status: chờ`
//!   -> `proposed`; `nhận`, `đã áp dụng` -> `accepted`; `bỏ`, `đã bỏ` -> `rejected`. Cạnh đã có trong vault (đề xuất
//!   đã áp dụng) thì nhận hồ sơ duyệt của đề xuất. `idea`, `type-batch` không tạo cạnh;
//! - cặp gần nghĩa của OHS (`_E11/similar-pairs.json` đã vào hàng chờ, `.e11/similar-pairs.json` tập rộng) chưa là
//!   đề xuất: `related`, `proposed`, proposed_by `model`, score là cosine, source là file cặp;
//! - nguồn gốc: cạnh frontmatter do script thêm (sổ `_E11/normalize-log.jsonl`, `_E11/migrate-v3-log.jsonl`) ghi
//!   proposed_by `rule` (hoặc `model`) và extracted_by là script; trang do workshop_ui.py sinh cũng vậy;
//! - bỏ wikilink trong thân note đề xuất; ẩn trang hệ thống ([`SYSTEM`]).
//!
//! Chỉ đọc vault.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::SystemTime;

use regex::Regex;
use rusqlite::types::Value as SqlValue;
use serde_json::Value;

use super::Adapter;
use crate::index::{Ctx, EdgeRow, IndexError, io_err, py_float};
use crate::vault::yaml::{self, Loader, Val};
use crate::vault::{self, nfc};

/// File cặp của OHS: hàng chờ, tập rộng.
const SIMILAR: [&str; 2] = ["_E11/similar-pairs.json", ".e11/similar-pairs.json"];
const MANIFEST: &str = "_E11/workshop-ui-manifest.json";
/// Sổ của script sửa frontmatter, theo thứ tự đọc (sổ sau đè sổ trước).
const LOGS: [(&str, &str); 2] = [
    ("_E11/normalize-log.jsonl", "normalize.py"),
    ("_E11/migrate-v3-log.jsonl", "migrate_v3.py"),
];
const OHS_MODEL: &str = "multilingual-e5-small";
/// Trang sinh tự động và hàng chờ: nối tới mọi thứ.
pub const SYSTEM: [&str; 5] = [
    "_E11/",
    "_Xưởng/",
    "_Templates/",
    "_Đề xuất/",
    "Trang chủ.md",
];
const FOLDER_EX: &str = "kg_adapter.py folder@schema_v3";
const INSTRUCTIONS: &str = "This is the E11 capture vault (bản B). Machine-suggested edges (layer proposed) come from two places: \
proposals waiting in the review queue `_Đề xuất/` (origin proposal: related, duplicate_of, who, link; \
source = the proposal note) and OHS near-neighbour pairs (origin ohs, rel related, proposed_by model, \
score = embedding similarity). origin folder = the note is filed under a topic page (rel topic). \
Edges a script added are marked proposed_by rule with the script in extracted_by. \
Proposals the user rejected are stored but never returned.";

/// `vault.WIKI` (cùng mẫu với module `vault`): nhóm 2 là đích.
static WIKI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(!?)\[\[([^\]|\n#]*)(#[^\]|\n]*)?(?:\|[^\]\n]*)?\]\]").expect("regex tĩnh hợp lệ")
});

/// `status` của đề xuất -> status của cạnh (chữ khác: `proposed`).
fn proposal_status(s: Option<&str>) -> &'static str {
    match s {
        Some("nhận" | "đã áp dụng") => "accepted",
        Some("bỏ" | "đã bỏ") => "rejected",
        _ => "proposed",
    }
}

/// Note hệ thống (ẩn khỏi tool trừ khi hỏi `system`).
pub fn is_system(path: &str) -> bool {
    SYSTEM.iter().any(|h| {
        if h.ends_with('/') {
            path.starts_with(h)
        } else {
            path == *h
        }
    })
}

/// Khoá đệm của một file: (mtime, cỡ), như `(st_mtime_ns, st_size)`.
type FileKey = (Option<SystemTime>, u64);
type Cache<T> = Mutex<HashMap<PathBuf, (FileKey, Arc<T>)>>;

/// `{(note, khoá): extractor}` từ sổ.
type LogKeys = HashMap<(String, String), String>;

/// Một cặp gần nghĩa của OHS.
#[derive(Debug, Clone, PartialEq)]
struct Pair {
    a: String,
    b: String,
    score: SqlValue,
}

/// Adapter bản B. Đọc lại sổ, manifest, file cặp chỉ khi file đổi (mtime, cỡ), nên dùng chung được giữa các lần
/// build của một tiến trình sống lâu (MCP server).
#[derive(Default)]
pub struct E11 {
    similar: Option<PathBuf>,
    spec: String,
    logs: Cache<LogKeys>,
    manifest: Cache<HashMap<String, String>>,
    pairs: Cache<Vec<Pair>>,
}

impl E11 {
    /// `Adapter(similar)`: `similar` là một file cặp OHS thay cho hai file mặc định.
    pub fn new(similar: Option<PathBuf>) -> Self {
        E11 {
            similar,
            ..E11::default()
        }
    }

    /// Như [`crate::adapter::load`] trả, nhớ chữ đã dùng để nạp.
    pub(crate) fn loaded(spec: &str) -> Self {
        E11 {
            spec: spec.to_owned(),
            ..E11::default()
        }
    }

    /// Sổ, manifest, rồi file cặp.
    fn files(&self, vault: &Path) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = LOGS
            .iter()
            .map(|(p, _)| join(vault, p))
            .chain([join(vault, MANIFEST)])
            .collect();
        match &self.similar {
            Some(s) => out.push(s.clone()),
            None => out.extend(SIMILAR.iter().map(|p| join(vault, p))),
        }
        out
    }

    /// `{(note, khoá): extractor}` cho khoá frontmatter có wikilink mà script thêm (sổ normalize, migrate), và
    /// `{note: extractor}` cho trang workshop_ui.py sinh.
    fn machine_writers(
        &self,
        vault: &Path,
    ) -> Result<(LogKeys, HashMap<String, String>), IndexError> {
        let mut keys = LogKeys::new();
        for (log, script) in LOGS {
            if let Some(k) = cached(&self.logs, &join(vault, log), |p| log_keys(p, script))? {
                keys.extend(k.iter().map(|(k, v)| (k.clone(), v.clone())));
            }
        }
        let pages = cached(&self.manifest, &join(vault, MANIFEST), manifest)?
            .map(|m| (*m).clone())
            .unwrap_or_default();
        Ok((keys, pages))
    }
}

/// `os.path.join(vault, p)` với `p` viết bằng `/`.
fn join(vault: &Path, p: &str) -> PathBuf {
    p.split('/').fold(vault.to_path_buf(), |acc, s| acc.join(s))
}

/// `_cached`: kết quả của `f(path)` khi file đổi; `None` khi không stat được file.
fn cached<T>(
    cache: &Cache<T>,
    path: &Path,
    f: impl FnOnce(&Path) -> Result<T, IndexError>,
) -> Result<Option<Arc<T>>, IndexError> {
    let Ok(meta) = std::fs::metadata(path) else {
        return Ok(None);
    };
    let key = (meta.modified().ok(), meta.len());
    let poisoned = || IndexError::Adapter("e11: bộ đệm hỏng".into());
    if let Some((k, v)) = cache.lock().map_err(|_| poisoned())?.get(path)
        && *k == key
    {
        return Ok(Some(v.clone()));
    }
    let v = Arc::new(f(path)?);
    cache
        .lock()
        .map_err(|_| poisoned())?
        .insert(path.to_path_buf(), (key, v.clone()));
    Ok(Some(v))
}

/// Đọc file UTF-8 (lỗi như `open(..., encoding='utf-8')`).
fn read_text(path: &Path) -> Result<String, IndexError> {
    let bytes = std::fs::read(path).map_err(io_err(path))?;
    String::from_utf8(bytes)
        .map_err(|e| IndexError::Adapter(format!("{}: không phải UTF-8: {e}", path.display())))
}

fn bad(path: &Path, msg: impl std::fmt::Display) -> IndexError {
    IndexError::Adapter(format!("{}: {msg}", path.display()))
}

fn parse_json(path: &Path, text: &str) -> Result<Value, IndexError> {
    serde_json::from_str(text).map_err(|e| bad(path, format!("JSON hỏng: {e}")))
}

/// `str.strip().strip('-')`.
fn strip_fm(s: &str) -> &str {
    s.trim_matches(crate::index::py_is_space).trim_matches('-')
}

/// `yaml.load(text, CSafeLoader) or {}`: `Ok(None)` khi YAML lỗi (Python bỏ dòng sổ đó).
fn load_fm(text: &str) -> Result<Option<Option<yaml::Doc>>, IndexError> {
    match yaml::load(strip_fm(text), Loader::Safe) {
        Ok(doc) => Ok(Some(doc.filter(|d| d.root().truthy()))),
        Err(yaml::Fail::Error(_)) => Ok(None),
        Err(yaml::Fail::Crash(t)) => Err(IndexError::Adapter(format!("sổ: YAML lỗi: {t}"))),
    }
}

/// `str(x)` của một giá trị YAML.
fn py_str(v: Val<'_>) -> Result<String, IndexError> {
    v.py_str()
        .map_err(|e| IndexError::Adapter(format!("sổ: str() lỗi: {e}")))
}

/// `old.get(k)` với `old` là dict YAML và `k` khoá của một dict khác. Khoá chuỗi so đúng như Python; khoá khác chuỗi
/// so theo `repr` (Python còn coi `1 == 1.0 == True`).
fn dict_get<'a>(old: Val<'a>, k: Val<'_>) -> Result<Option<Val<'a>>, IndexError> {
    if let Some(s) = k.as_str() {
        return Ok(old.get(s));
    }
    let want = k.py_repr().ok();
    Ok(old
        .items()
        .into_iter()
        .flatten()
        .find_map(|(ok, ov)| (ok.as_str().is_none() && ok.py_repr().ok() == want).then_some(ov)))
}

/// Chuỗi của `"+".join(x)` với `x` là giá trị JSON (list chuỗi, chuỗi, dict).
fn join_plus(path: &Path, x: &Value) -> Result<String, IndexError> {
    let parts: Vec<String> = match x {
        Value::Array(a) => a
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| bad(path, "rules phải là chuỗi"))
            })
            .collect::<Result<_, _>>()?,
        Value::String(s) => s.chars().map(String::from).collect(),
        Value::Object(m) => m.keys().cloned().collect(),
        _ => return Err(bad(path, "rules không lặp được")),
    };
    Ok(parts.join("+"))
}

/// Các dòng của file văn bản như `for line in open(...)` (xuống dòng kiểu `\n`, `\r\n`, `\r`).
fn lines(text: &str) -> Vec<String> {
    let t = text.replace("\r\n", "\n").replace('\r', "\n");
    t.split_inclusive('\n').map(str::to_owned).collect()
}

/// `_log_keys`: khoá có wikilink mà script đổi, theo từng dòng sổ.
fn log_keys(path: &Path, script: &str) -> Result<LogKeys, IndexError> {
    let mut keys = LogKeys::new();
    for line in lines(&read_text(path)?) {
        let d = parse_json(path, &line)?;
        let d = match d {
            Value::Object(m) => m,
            // `'new_frontmatter' not in d` rồi `d.get`: chỉ dict mới qua được.
            Value::Array(a) if !a.iter().any(|x| x == "new_frontmatter") => continue,
            Value::String(s) if !s.contains("new_frontmatter") => continue,
            _ => return Err(bad(path, "dòng sổ không phải object")),
        };
        if !d.contains_key("new_frontmatter") {
            continue; // sổ migrate chỉ giữ phía cũ; note giờ mang phía mới
        }
        let fm = |k: &str| -> Result<String, IndexError> {
            match d.get(k) {
                Some(Value::String(s)) => Ok(s.clone()),
                None | Some(Value::Null | Value::Bool(false)) => Ok(String::new()),
                Some(Value::Array(a)) if a.is_empty() => Ok(String::new()),
                Some(Value::Object(m)) if m.is_empty() => Ok(String::new()),
                Some(Value::Number(n)) if n.as_f64() == Some(0.0) => Ok(String::new()),
                Some(_) => Err(bad(path, format!("{k} phải là chuỗi"))),
            }
        };
        let (old_text, new_text) = (fm("old_frontmatter")?, fm("new_frontmatter")?);
        let (Some(old), Some(new)) = (load_fm(&old_text)?, load_fm(&new_text)?) else {
            continue;
        };
        let Some(new) = new else {
            continue;
        };
        let items: Vec<_> = new
            .root()
            .items()
            .ok_or_else(|| bad(path, "new_frontmatter không phải dict"))?
            .collect();
        for (k, v) in items {
            let sv = py_str(v)?;
            if !sv.contains("[[") {
                continue;
            }
            let ov = match &old {
                None => None,
                Some(o) if o.root().is_dict() => dict_get(o.root(), k)?,
                Some(_) => return Err(bad(path, "old_frontmatter không phải dict")),
            };
            let so = match ov {
                Some(x) => py_str(x)?,
                None => "None".to_owned(),
            };
            if so == sv {
                continue;
            }
            let note = d
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| bad(path, "dòng sổ thiếu path"))?;
            let rules = match d.get("rules") {
                None => String::new(),
                Some(r) => join_plus(path, r)?,
            };
            keys.insert(
                (nfc(note), py_str(k)?),
                format!("{script} {rules}@schema_v3"),
            );
        }
    }
    Ok(keys)
}

/// `_manifest`: `{trang: 'workshop_ui.py@schema_v3'}` cho mọi file của manifest.
fn manifest(path: &Path) -> Result<HashMap<String, String>, IndexError> {
    let v = parse_json(path, &read_text(path)?)?;
    let Value::Object(m) = v else {
        return Err(bad(path, "manifest không phải object"));
    };
    let files: Vec<String> = match m.get("files") {
        None => Vec::new(),
        Some(Value::Object(f)) => f.keys().cloned().collect(),
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| {
                x.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| bad(path, "files phải là chuỗi"))
            })
            .collect::<Result<_, _>>()?,
        Some(Value::String(s)) => s.chars().map(String::from).collect(),
        Some(_) => return Err(bad(path, "files không lặp được")),
    };
    Ok(files
        .into_iter()
        .map(|f| {
            let f = if std::path::MAIN_SEPARATOR == '/' {
                f
            } else {
                f.replace(std::path::MAIN_SEPARATOR, "/")
            };
            (nfc(&f), "workshop_ui.py@schema_v3".to_owned())
        })
        .collect())
}

/// Giá trị JSON ghi vào cột `score` như `sqlite3` của Python ghi giá trị `json.load` trả.
fn score_value(path: &Path, v: Option<&Value>) -> Result<SqlValue, IndexError> {
    Ok(match v {
        None | Some(Value::Null) => SqlValue::Null,
        Some(Value::Bool(b)) => SqlValue::Integer(i64::from(*b)),
        Some(Value::Number(n)) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => SqlValue::Integer(i),
            (None, Some(x)) if !n.is_u64() => SqlValue::Real(x),
            _ => return Err(bad(path, "score: số nguyên quá lớn cho SQLite")),
        },
        Some(Value::String(s)) => SqlValue::Text(s.clone()),
        Some(_) => return Err(bad(path, "score phải là số")),
    })
}

/// `json.load(...) or []` của file cặp.
fn pairs(path: &Path) -> Result<Vec<Pair>, IndexError> {
    let v = parse_json(path, &read_text(path)?)?;
    let items = match v {
        Value::Array(a) => a,
        Value::Null | Value::Bool(false) => Vec::new(),
        Value::String(s) if s.is_empty() => Vec::new(),
        Value::Object(m) if m.is_empty() => Vec::new(),
        Value::Number(n) if n.as_f64() == Some(0.0) => Vec::new(),
        _ => return Err(bad(path, "file cặp phải là list")),
    };
    items
        .iter()
        .map(|p| {
            let side = |k: &str| {
                p.get(k)
                    .and_then(Value::as_str)
                    .map(nfc)
                    .ok_or_else(|| bad(path, format!("cặp thiếu {k} (chuỗi)")))
            };
            if !p.is_object() {
                return Err(bad(path, "cặp phải là object"));
            }
            Ok(Pair {
                a: side("a")?,
                b: side("b")?,
                score: score_value(path, p.get("score"))?,
            })
        })
        .collect()
}

/// `os.path.relpath(path, start)` (theo chữ, không đi theo symlink), phân cách `/`.
fn relpath(path: &Path, start: &Path) -> Result<String, IndexError> {
    let abs = |p: &Path| -> Result<Vec<String>, IndexError> {
        let a = std::path::absolute(p).map_err(io_err(p))?;
        let mut out: Vec<String> = Vec::new();
        for c in a.components() {
            match c {
                Component::CurDir => {}
                Component::ParentDir => {
                    if out.len() > 1 {
                        out.pop();
                    }
                }
                c => out.push(c.as_os_str().to_string_lossy().into_owned()),
            }
        }
        Ok(out)
    };
    let (p, s) = (abs(path)?, abs(start)?);
    if p.first() != s.first() {
        return Err(IndexError::Adapter(format!(
            "{} và {} không cùng ổ đĩa",
            path.display(),
            start.display()
        )));
    }
    let common = p.iter().zip(&s).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<&str> = vec![".."; s.len() - common];
    parts.extend(p[common..].iter().map(String::as_str));
    Ok(if parts.is_empty() {
        ".".to_owned()
    } else {
        parts.join("/")
    })
}

/// Cặp không thứ tự (`frozenset((a, b))`).
fn unordered(a: Option<&str>, b: Option<&str>) -> (Option<String>, Option<String>) {
    let (x, y) = if a <= b { (a, b) } else { (b, a) };
    (x.map(str::to_owned), y.map(str::to_owned))
}

fn ids(ctx: &Ctx<'_>, sql: &str) -> Result<Vec<String>, IndexError> {
    let mut st = ctx.db.prepare(sql)?;
    let rows = st
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

impl Adapter for E11 {
    fn name(&self) -> Option<&str> {
        Some("e11")
    }

    fn spec(&self) -> &str {
        &self.spec
    }

    fn hidden(&self) -> &[&'static str] {
        &SYSTEM
    }

    fn instructions(&self) -> Option<&str> {
        Some(INSTRUCTIONS)
    }

    /// Đổi khi sổ, manifest, file cặp đổi: build không đổi gì thì khỏi tính lại cạnh.
    fn digest(&self, vault: &Path) -> Option<String> {
        let parts: Vec<String> = self
            .files(vault)
            .iter()
            .map(|p| match std::fs::metadata(p) {
                Ok(m) => format!("{}.{}", mtime_secs(&m), m.len()),
                Err(_) => "-".to_owned(),
            })
            .collect();
        let mut d = parts.join("-");
        if let Some(s) = &self.similar {
            d.push('|');
            d.push_str(&s.to_string_lossy());
        }
        Some(d)
    }

    fn edges(&self, ctx: &mut Ctx<'_>) -> Result<(), IndexError> {
        let vault = ctx.vault;
        let paths = ids(ctx, "SELECT id FROM nodes")?;
        let proposals: HashSet<String> = ids(ctx, "SELECT id FROM nodes WHERE type = 'proposal'")?
            .into_iter()
            .collect();
        // thân note đề xuất chỉ nhúng lại các note đã ghi trong `notes`
        ctx.drop_where(|r| r.origin == "body" && proposals.contains(&r.src));
        // nguồn gốc cạnh do script thêm
        let (keys, pages) = self.machine_writers(vault)?;
        for r in &mut ctx.rows {
            let by_key = if r.origin == "frontmatter" {
                keys.get(&(r.src.clone(), r.rel.clone()))
            } else {
                None
            };
            let ex = by_key
                .filter(|x| !x.is_empty())
                .or_else(|| pages.get(&r.src))
                .filter(|x| !x.is_empty());
            if let Some(ex) = ex
                && (r.origin == "frontmatter" || r.origin == "body")
            {
                r.extracted_by = Some(ex.clone());
                r.proposed_by = Some(
                    if ex.contains(OHS_MODEL) {
                        "model"
                    } else {
                        "rule"
                    }
                    .to_owned(),
                );
            }
        }
        // thư mục: trang chủ đề của thư mục gần nhất có trang
        let topics: HashMap<String, String> = ids(
            ctx,
            "SELECT id FROM nodes WHERE type = 'toc' AND id LIKE '%(chủ đề).md'",
        )?
        .into_iter()
        .map(|p| (vault::folder(&p).to_owned(), p))
        .collect();
        for p in &paths {
            let mut d = vault::folder(p);
            while !d.is_empty() && !topics.contains_key(d) {
                d = vault::folder(d);
            }
            if let Some(t) = topics.get(d)
                && t != p
                && !is_system(p)
            {
                let mut e = EdgeRow::new(p, "topic", "folder");
                e.dst = Some(t.clone());
                e.dst_text = Some(t.clone());
                e.status = Some("accepted".into());
                e.proposed_by = Some("human".into());
                e.extracted_by = Some(FOLDER_EX.into());
                ctx.add(e);
            }
        }
        // đề xuất: cạnh nó sẽ tạo, với hồ sơ của chính nó
        let mut props: BTreeMap<String, HashMap<String, Vec<Option<String>>>> = BTreeMap::new();
        {
            let mut st = ctx.db.prepare(
                "SELECT p.node, p.key, p.value FROM props p JOIN nodes n ON n.id = p.node WHERE n.type = 'proposal'",
            )?;
            let mut q = st.query([])?;
            while let Some(r) = q.next()? {
                props
                    .entry(r.get(0)?)
                    .or_default()
                    .entry(r.get(1)?)
                    .or_default()
                    .push(r.get(2)?);
            }
        }
        let mut fm_index: HashMap<(String, String, String), usize> = HashMap::new();
        for (i, r) in ctx.rows.iter().enumerate() {
            if let Some(d) = r.dst.as_deref().filter(|d| !d.is_empty())
                && r.origin == "frontmatter"
            {
                fm_index.insert((r.src.clone(), r.rel.clone(), d.to_owned()), i);
            }
        }
        let known: HashSet<&str> = paths.iter().map(String::as_str).collect();
        for (p, f) in &props {
            let list = |k: &str| f.get(k).filter(|v| !v.is_empty());
            let first = |k: &str| list(k).and_then(|v| v[0].clone());
            let kind = first("kind");
            let st = proposal_status(list("status").and_then(|v| v[0].as_deref()));
            let notes: Vec<String> = list("notes")
                .into_iter()
                .flatten()
                .flatten()
                .filter(|x| !x.is_empty())
                .filter_map(|x| WIKI.captures(x))
                .filter_map(|c| {
                    let target = c.get(2).map_or("", |m| m.as_str());
                    ctx.resolver.resolve(p, target, false).0
                })
                .collect();
            let (by, ex) = (first("proposed_by"), first("extracted_by"));
            let score = match first("data.score") {
                None => SqlValue::Null,
                Some(sc) if sc == "None" || sc.is_empty() => SqlValue::Null,
                Some(sc) => SqlValue::Real(py_float(&sc).ok_or_else(|| {
                    IndexError::Adapter(format!(
                        "{p}: could not convert string to float: {}",
                        crate::vault::yaml::repr::str_repr(&sc)
                    ))
                })?),
            };
            let asserted = list("asserted_at")
                .or_else(|| list("closed"))
                .and_then(|v| v[0].clone());
            let mut made: Vec<(String, &str, Option<String>, String)> = Vec::new();
            match kind.as_deref() {
                Some("related" | "same-topic") if notes.len() >= 2 => {
                    made.push((
                        notes[0].clone(),
                        "related",
                        Some(notes[1].clone()),
                        notes[1].clone(),
                    ));
                }
                Some("duplicate") if notes.len() >= 2 => {
                    for n in &notes[1..] {
                        made.push((
                            n.clone(),
                            "duplicate_of",
                            Some(notes[0].clone()),
                            notes[0].clone(),
                        ));
                    }
                }
                Some("fix-link") if list("data.to").is_some() => {
                    let to = nfc(&first("data.to")
                        .ok_or_else(|| IndexError::Adapter(format!("{p}: data.to rỗng")))?);
                    let ok = known.contains(to.as_str());
                    for n in notes.iter().filter(|n| **n != to) {
                        made.push((n.clone(), "link", ok.then(|| to.clone()), to.clone()));
                    }
                }
                Some("person") if list("data.value").is_some() => {
                    let page = format!(
                        "Người/{}",
                        first("data.value").unwrap_or_else(|| "None".to_owned())
                    );
                    for n in &notes {
                        made.push((
                            n.clone(),
                            "who",
                            ctx.resolver.resolve(p, &page, false).0,
                            page.clone(),
                        ));
                    }
                }
                _ => {}
            }
            for (src, rel, dst, text) in made {
                let hit = dst.as_ref().and_then(|d| {
                    fm_index
                        .get(&(src.clone(), rel.to_owned(), d.clone()))
                        .or_else(|| {
                            (rel == "related")
                                .then(|| fm_index.get(&(d.clone(), rel.to_owned(), src.clone())))
                                .flatten()
                        })
                        .copied()
                });
                if let Some(i) = hit {
                    // đã ghi trong vault (đã áp dụng): cạnh của vault nhận hồ sơ duyệt
                    let r = &mut ctx.rows[i];
                    r.proposed_by = by.clone();
                    r.extracted_by = ex.clone();
                    r.score = score.clone();
                    r.source = Some(p.clone());
                    r.asserted_at = asserted.clone();
                } else {
                    let mut e = EdgeRow::new(src, rel, "proposal");
                    e.dst = dst;
                    e.dst_text = Some(text);
                    e.status = Some(st.to_owned());
                    e.proposed_by = by.clone();
                    e.extracted_by = ex.clone();
                    e.score = score.clone();
                    e.source = Some(p.clone());
                    e.asserted_at = if st == "accepted" {
                        asserted.clone()
                    } else {
                        None
                    };
                    ctx.add(e);
                }
            }
        }
        // cặp gần nghĩa OHS chưa là đề xuất (kể cả đề xuất đã bỏ: không gợi ý lại)
        let mut have: HashSet<(Option<String>, Option<String>)> = ctx
            .rows
            .iter()
            .filter(|r| r.rel == "related")
            .map(|r| unordered(Some(&r.src), r.dst.as_deref()))
            .collect();
        for fp in &self.files(vault)[LOGS.len() + 1..] {
            let Some(list) = cached(&self.pairs, fp, pairs)? else {
                continue;
            };
            let ev = relpath(fp, vault)?;
            for pair in list.iter() {
                if known.contains(pair.a.as_str())
                    && known.contains(pair.b.as_str())
                    && have.insert(unordered(Some(&pair.a), Some(&pair.b)))
                {
                    let mut e = EdgeRow::new(&pair.a, "related", "ohs");
                    e.dst = Some(pair.b.clone());
                    e.dst_text = Some(pair.b.clone());
                    e.status = Some("proposed".into());
                    e.proposed_by = Some("model".into());
                    e.extracted_by = Some(format!("{OHS_MODEL}@schema_v3"));
                    e.score = pair.score.clone();
                    e.source = Some(ev.clone());
                    ctx.add(e);
                }
            }
        }
        Ok(())
    }
}

/// `int(st.st_mtime)`: `st_mtime` là số thực `giây + nano * 1e-9`, cắt về 0 như Python.
fn mtime_secs(m: &std::fs::Metadata) -> i64 {
    let secs = match m
        .modified()
        .map(|t| t.duration_since(SystemTime::UNIX_EPOCH))
    {
        Ok(Ok(d)) => d.as_secs() as f64 + f64::from(d.subsec_nanos()) * 1e-9,
        Ok(Err(e)) => {
            -(e.duration().as_secs() as f64 + f64::from(e.duration().subsec_nanos()) * 1e-9)
        }
        Err(_) => 0.0,
    };
    secs as i64
}
