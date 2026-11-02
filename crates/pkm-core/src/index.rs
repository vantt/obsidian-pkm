//! Index SQLite của một vault: note, thuộc tính, link, block, tên, và bảng `edges` hai lớp (accepted, proposed) có
//! cửa sổ hiệu lực, thời điểm ghi nhận và nguồn gốc. Index là bản đệm dựng lại được từ vault; không ghi vào note.
//!
//! Đặc tả: `scripts/kg/index.py` ở sha ghim. DDL chép nguyên (D3); mọi câu đọc lại DB trong lúc build cũng giống
//! Python, nên thứ tự dòng (và `edges.id`) đi theo cùng một SQLite. Note được liệt kê theo đường dẫn đã sắp
//! (docs/DECISIONS.md, "Thứ tự liệt kê file").
//!
//! Cạnh sinh ra:
//! - `frontmatter`: khoá là cạnh của schema (cùng các gói bật), một cạnh cho mỗi đích. Cửa sổ hiệu lực là của note
//!   nguồn. Status: `proposed`/`rejected` nếu note nguồn mang status đó, còn lại `accepted`. `proposed_by`,
//!   `extracted_by`, `asserted_at` lấy từ note nguồn.
//! - `claim`: note `type: claim` là một cạnh `relation` từ cause tới effect (nhóm causal) hoặc subject tới object
//!   (nhóm facts), theo `required_if` của schema; mang status, cửa sổ, confidence, nguồn trích (`note#^block`) và
//!   người đề xuất của claim.
//! - `body` (tuỳ chọn `body_edges`): wikilink trong thân note thành cạnh `link`.
//! - adapter: vault riêng thêm, sửa, bỏ cạnh của mình qua [`crate::adapter::Adapter::edges`].
//!
//! Tăng dần: chỉ đọc lại note đổi mtime hoặc cỡ; link và cạnh tính lại từ phần đã parse (rẻ), vì đổi một tên note
//! có thể đổi cách giải link của note khác.

mod py;

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use regex::Regex;
use rusqlite::functions::FunctionFlags;
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{Connection, OpenFlags, TransactionBehavior, params};

use crate::adapter::Adapter;
use crate::schema::{self, Cell, EdgeKind, Schema, Source};
use crate::validate;
use crate::vault::{self, NoteFile, Resolver};

pub(crate) use py::{float as py_float, is_space as py_is_space};

/// DDL của index, chép nguyên từ `index.py`.
pub const DDL: &str = "
PRAGMA journal_mode=WAL;
CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT);
CREATE TABLE IF NOT EXISTS nodes(id TEXT PRIMARY KEY, title TEXT, type TEXT, ntypes INTEGER, folder TEXT, status TEXT,
  valid_from TEXT, valid_to TEXT, proposed_by TEXT, extracted_by TEXT, asserted_at TEXT, mtime INTEGER, size INTEGER,
  yaml_error TEXT) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS props(node TEXT, key TEXT, value TEXT);
CREATE INDEX IF NOT EXISTS props_node ON props(node, key);
CREATE INDEX IF NOT EXISTS props_key ON props(key, value);
CREATE TABLE IF NOT EXISTS raw_links(src TEXT, key TEXT, target TEXT, frag TEXT, origin TEXT, embed INTEGER);
CREATE INDEX IF NOT EXISTS raw_links_src ON raw_links(src);
CREATE TABLE IF NOT EXISTS links(src TEXT, key TEXT, target TEXT, frag TEXT, origin TEXT, embed INTEGER, dst TEXT, via TEXT);
CREATE INDEX IF NOT EXISTS links_src ON links(src, key);
CREATE INDEX IF NOT EXISTS links_dst ON links(dst);
CREATE TABLE IF NOT EXISTS blocks(node TEXT, block TEXT, PRIMARY KEY(node, block)) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS names(node TEXT, name TEXT, kind TEXT, norm TEXT);
CREATE INDEX IF NOT EXISTS names_norm ON names(norm);
CREATE INDEX IF NOT EXISTS names_node ON names(node);
CREATE TABLE IF NOT EXISTS edges(id INTEGER PRIMARY KEY, src TEXT, rel TEXT, dst TEXT, dst_text TEXT, claim TEXT,
  status TEXT, valid_from TEXT, valid_to TEXT, asserted_at TEXT, source TEXT, origin TEXT, proposed_by TEXT,
  extracted_by TEXT, confidence TEXT, score REAL);
CREATE INDEX IF NOT EXISTS edges_src ON edges(src, rel);
CREATE INDEX IF NOT EXISTS edges_dst ON edges(dst, rel);
CREATE INDEX IF NOT EXISTS edges_claim ON edges(claim) WHERE claim IS NOT NULL;
";

/// Bảng ghi theo từng note (xoá và ghi lại khi note đổi).
pub const NOTE_TABLES: [&str; 5] = ["nodes", "props", "raw_links", "blocks", "names"];

/// Cột của `edges` trừ `id`, theo thứ tự chèn.
pub const EDGE_COLS: [&str; 15] = [
    "src",
    "rel",
    "dst",
    "dst_text",
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

/// Các status của schema v3.
pub const STATUS: [&str; 4] = ["proposed", "accepted", "rejected", "superseded"];

// ------------------------------------------------------------------------------------------------ lỗi

/// Lỗi khi dựng hoặc đọc index (chỗ bản Python ném ngoại lệ).
#[derive(Debug)]
pub enum IndexError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Sql(rusqlite::Error),
    Schema(schema::SchemaError),
    /// Dữ liệu riêng của adapter hỏng (file cặp, sổ, manifest), hoặc adapter không có.
    Adapter(String),
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IndexError::Io { path, source } => write!(f, "{}: {source}", path.display()),
            IndexError::Sql(e) => write!(f, "SQLite: {e}"),
            IndexError::Schema(e) => e.fmt(f),
            IndexError::Adapter(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for IndexError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            IndexError::Io { source, .. } => Some(source),
            IndexError::Sql(e) => Some(e),
            IndexError::Schema(e) => Some(e),
            IndexError::Adapter(_) => None,
        }
    }
}

impl From<rusqlite::Error> for IndexError {
    fn from(e: rusqlite::Error) -> Self {
        IndexError::Sql(e)
    }
}

impl From<schema::SchemaError> for IndexError {
    fn from(e: schema::SchemaError) -> Self {
        IndexError::Schema(e)
    }
}

pub(crate) fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> IndexError + '_ {
    move |source| IndexError::Io {
        path: path.to_path_buf(),
        source,
    }
}

// ------------------------------------------------------------------------------------------------ cách đọc vault

/// Khoá frontmatter nào thành cạnh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgeKeys {
    /// Chỉ khoá là cạnh của schema.
    #[default]
    Schema,
    /// Mọi khoá frontmatter có wikilink (trừ `aliases`).
    Links,
}

impl EdgeKeys {
    pub fn as_str(self) -> &'static str {
        match self {
            EdgeKeys::Schema => "schema",
            EdgeKeys::Links => "links",
        }
    }

    /// Từ chữ của cờ `--edge-keys` hay của `meta.options`.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "schema" => Some(EdgeKeys::Schema),
            "links" => Some(EdgeKeys::Links),
            _ => None,
        }
    }
}

/// Cách đọc vault. Mặc định theo `lib/kg.mjs` của fixture (vault kiểu kg-solo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// Giải link theo alias khi không khớp tên note.
    pub aliases: bool,
    pub edge_keys: EdgeKeys,
    /// Wikilink trong thân note thành cạnh `link`.
    pub body_edges: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            aliases: true,
            edge_keys: EdgeKeys::Schema,
            body_edges: false,
        }
    }
}

impl Options {
    /// Chuỗi ghi vào `meta.options`.
    pub fn key(&self) -> String {
        format!(
            "aliases={},edge_keys={},body_edges={}",
            u8::from(self.aliases),
            self.edge_keys.as_str(),
            u8::from(self.body_edges)
        )
    }

    /// Từ cờ dòng lệnh; không cờ nào được ghi thì lấy cách đọc của lần build trước (như gói, adapter), để lần chạy
    /// sau quên cờ không dựng lại index theo cách khác.
    pub fn pick(
        no_aliases: bool,
        edge_keys: Option<EdgeKeys>,
        body_edges: bool,
        db: Option<&Path>,
    ) -> Self {
        if !(no_aliases || edge_keys.is_some() || body_edges)
            && let Some(db) = db
        {
            let prev = previous(db, "options", "");
            let d: HashMap<&str, &str> =
                prev.split(',').filter_map(|x| x.split_once('=')).collect();
            if !d.is_empty() {
                return Options {
                    aliases: d.get("aliases") != Some(&"0"),
                    // Python giữ nguyên chữ lạ (và đọc như `links`); chữ lạ chỉ có khi sửa DB bằng tay.
                    edge_keys: match d.get("edge_keys") {
                        None | Some(&"schema") => EdgeKeys::Schema,
                        Some(_) => EdgeKeys::Links,
                    },
                    body_edges: d.get("body_edges") == Some(&"1"),
                };
            }
        }
        Options {
            aliases: !no_aliases,
            edge_keys: edge_keys.unwrap_or_default(),
            body_edges,
        }
    }
}

/// Giá trị `meta` của lần build trước (gói, adapter, cách đọc), để lệnh sau không phải ghi lại. DB không có hay
/// không đọc được thì trả `default` (cần phân biệt hai trường hợp: [`try_previous`]).
pub fn previous(db: &Path, key: &str, default: &str) -> String {
    match try_previous(db, key) {
        Ok(Some(v)) => v,
        _ => default.to_owned(),
    }
}

/// Như [`previous`] nhưng báo lỗi: `Ok(None)` khi DB chưa có hay không có khoá, `Err` khi DB có mà không đọc được.
/// Mở chỉ đọc trước; DB WAL trong thư mục không tạo được `-shm` thì thử mở đọc ghi (không tạo DB mới) rồi mới chịu.
pub fn try_previous(db: &Path, key: &str) -> Result<Option<String>, IndexError> {
    if !db.exists() {
        return Ok(None);
    }
    let read = |c: Connection| -> Result<Option<String>, IndexError> {
        let mut st = c.prepare("SELECT key, value FROM meta")?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<String>>(1)?,
            ))
        })?;
        let mut found = None;
        for r in rows {
            let (k, v) = r?;
            if k.as_deref() == Some(key) {
                found = Some(v);
            }
        }
        // `dict(...).get(key, default)`: giá trị NULL thì Python trả `None`; ở đây coi như chuỗi rỗng.
        Ok(found.map(Option::unwrap_or_default))
    };
    match connect(db, true).and_then(read) {
        Ok(v) => Ok(v),
        Err(first) => {
            let rw = Connection::open_with_flags(
                db,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .map_err(IndexError::Sql);
            rw.and_then(read).map_err(|_| first)
        }
    }
}

// ------------------------------------------------------------------------------------------------ kết nối

/// Mở index. `readonly`: chỉ đọc (DB phải có sẵn); không thì tạo thư mục cha khi cần. Mọi kết nối có hàm SQL
/// `regexp(mẫu, chuỗi)` theo cú pháp `re` của Python (view R9, R11 dùng).
pub fn connect(path: &Path, readonly: bool) -> Result<Connection, IndexError> {
    let db = if readonly {
        Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?
    } else {
        let abs = std::path::absolute(path).map_err(io_err(path))?;
        if let Some(dir) = abs.parent() {
            std::fs::create_dir_all(dir).map_err(io_err(dir))?;
        }
        Connection::open(path)?
    };
    db.busy_timeout(BUSY_TIMEOUT)?;
    add_regexp(&db)?;
    Ok(db)
}

/// Thời gian chờ khi DB đang bị kết nối khác khoá (đọc ghi đồng thời của MCP và CLI).
const BUSY_TIMEOUT: Duration = Duration::from_secs(30);
/// Thời gian chờ build khác (cùng DB) xong.
const LOCK_TIMEOUT: Duration = Duration::from_secs(300);
/// Khoá `meta` đánh dấu bảng note đã đổi mà cạnh chưa tính lại; xoá khi ghi `meta` cuối build.
const DIRTY: &str = "dirty";

/// Khoá file `<db>.lock` cạnh DB: mỗi lúc chỉ một build ghi một DB (MCP tự build lại và CLI có thể chạy cùng lúc).
/// Hệ điều hành nhả khoá khi file đóng hay tiến trình chết; file không bị xoá để tránh tranh chấp.
struct BuildLock(Option<std::fs::File>);

impl BuildLock {
    fn acquire(db: &Path) -> Result<Self, IndexError> {
        let abs = std::path::absolute(db).map_err(io_err(db))?;
        if let Some(dir) = abs.parent() {
            std::fs::create_dir_all(dir).map_err(io_err(dir))?;
        }
        let mut name = abs.file_name().unwrap_or_default().to_os_string();
        name.push(".lock");
        let path = abs.with_file_name(name);
        let f = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(io_err(&path))?;
        let start = Instant::now();
        loop {
            match f.try_lock() {
                Ok(()) => return Ok(BuildLock(Some(f))),
                Err(std::fs::TryLockError::WouldBlock) if start.elapsed() < LOCK_TIMEOUT => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    return Err(IndexError::Io {
                        path,
                        source: std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "build khác đang ghi index này quá lâu",
                        ),
                    });
                }
                // Hệ thống file không có khoá (vài ổ mạng): build như bản Python, không khoá.
                Err(std::fs::TryLockError::Error(e))
                    if e.kind() == std::io::ErrorKind::Unsupported =>
                {
                    return Ok(BuildLock(None));
                }
                Err(std::fs::TryLockError::Error(e)) => return Err(io_err(&path)(e)),
            }
        }
    }
}

impl Drop for BuildLock {
    fn drop(&mut self) {
        if let Some(f) = &self.0 {
            let _ = f.unlock();
        }
    }
}

/// `PRAGMA journal_mode=WAL` của DDL, chỉ khi DB chưa ở WAL. Không đổi được (kết nối khác đang mở DB ở chế độ
/// rollback, ổ mạng không có bộ nhớ chung) thì giữ chế độ cũ và build tiếp.
fn set_wal(db: &Connection) {
    let mode: Option<String> = db.query_row("PRAGMA journal_mode", [], |r| r.get(0)).ok();
    if !mode.is_some_and(|m| m.eq_ignore_ascii_case("wal")) {
        // Không chờ: đổi chế độ cần khoá riêng, kết nối khác đang đọc thì bỏ qua ngay.
        let _ = db.busy_timeout(Duration::ZERO);
        let _ = db.query_row("PRAGMA journal_mode=WAL", [], |r| r.get::<_, String>(0));
        let _ = db.busy_timeout(BUSY_TIMEOUT);
    }
}

/// Mẫu `re` của Python thành mẫu của crate `regex`: `$` ngoài lớp ký tự khớp ở cuối chuỗi hoặc trước `\n` cuối
/// (như Python), `\Z` là `\z`. Cú pháp Python mà `regex` không có (nhìn trước, nhìn lùi, tham chiếu ngược) báo lỗi.
pub(crate) fn py_regex(p: &str) -> Result<Regex, regex::Error> {
    let mut out = String::with_capacity(p.len() + 8);
    let mut chars = p.chars().peekable();
    let mut class = false;
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('Z') if !class => out.push_str(r"\z"),
                Some(n) => {
                    out.push('\\');
                    out.push(n);
                }
                None => out.push('\\'),
            },
            '[' if !class => {
                class = true;
                out.push('[');
                // `]` hay `^]` ngay sau `[` là ký tự thường của lớp.
                if chars.peek() == Some(&'^') {
                    out.push('^');
                    chars.next();
                }
                if chars.peek() == Some(&']') {
                    out.push_str(r"\]");
                    chars.next();
                }
            }
            ']' if class => {
                class = false;
                out.push(']');
            }
            '$' if !class => out.push_str(r"(?:\n?\z)"),
            // Trong lớp ký tự, `regex` coi `[` là lớp lồng và `&&`, `~~` là phép tập hợp; Python coi là chữ thường.
            '[' | '&' | '~' if class => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    Regex::new(&out)
}

type RegexCache = Mutex<HashMap<String, Result<Arc<Regex>, String>>>;

fn add_regexp(db: &Connection) -> rusqlite::Result<()> {
    let cache: RegexCache = Mutex::new(HashMap::new());
    db.create_scalar_function(
        "regexp",
        2,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        move |ctx| {
            let text = |i: usize| -> rusqlite::Result<Option<String>> {
                Ok(match ctx.get_raw(i) {
                    ValueRef::Null => None,
                    ValueRef::Text(t) => Some(String::from_utf8_lossy(t).into_owned()),
                    ValueRef::Integer(n) => Some(n.to_string()),
                    ValueRef::Real(x) => Some(x.to_string()),
                    ValueRef::Blob(_) => {
                        return Err(rusqlite::Error::UserFunctionError(
                            "regexp: không nhận blob".into(),
                        ));
                    }
                })
            };
            // `s is not None and re.search(p, s) is not None`.
            let Some(s) = text(1)? else {
                return Ok(false);
            };
            let p = text(0)?.unwrap_or_default();
            let re = {
                let mut c = cache
                    .lock()
                    .map_err(|_| rusqlite::Error::UserFunctionError("regexp: khoá hỏng".into()))?;
                c.entry(p.clone())
                    .or_insert_with(|| {
                        py_regex(&p)
                            .map(Arc::new)
                            .map_err(|e| format!("regexp: mẫu {p:?} không dùng được: {e}"))
                    })
                    .clone()
            };
            match re {
                Ok(re) => Ok(re.is_match(&s)),
                Err(e) => Err(rusqlite::Error::UserFunctionError(e.into())),
            }
        },
    )
}

// ------------------------------------------------------------------------------------------------ cạnh

/// Một dòng của `edges` (không có `id`).
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeRow {
    pub src: String,
    pub rel: String,
    pub dst: Option<String>,
    pub dst_text: Option<String>,
    pub claim: Option<String>,
    pub status: Option<String>,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
    pub asserted_at: Option<String>,
    pub source: Option<String>,
    pub origin: String,
    pub proposed_by: Option<String>,
    pub extracted_by: Option<String>,
    pub confidence: Option<String>,
    /// Cột REAL: SQLite đổi số nguyên và chuỗi dạng số thành số thực khi ghi, như với giá trị Python.
    pub score: SqlValue,
}

impl EdgeRow {
    /// Cạnh chỉ có nguồn, quan hệ, nguồn gốc; các cột khác trống.
    pub fn new(src: impl Into<String>, rel: impl Into<String>, origin: impl Into<String>) -> Self {
        EdgeRow {
            src: src.into(),
            rel: rel.into(),
            dst: None,
            dst_text: None,
            claim: None,
            status: None,
            valid_from: None,
            valid_to: None,
            asserted_at: None,
            source: None,
            origin: origin.into(),
            proposed_by: None,
            extracted_by: None,
            confidence: None,
            score: SqlValue::Null,
        }
    }

    /// Khoá bỏ trùng `(src, rel, dst or dst_text, claim, origin)`.
    fn key(&self) -> EdgeKey {
        let end = self
            .dst
            .as_deref()
            .filter(|d| !d.is_empty())
            .or(self.dst_text.as_deref());
        (
            self.src.clone(),
            self.rel.clone(),
            end.map(str::to_owned),
            self.claim.clone(),
            self.origin.clone(),
        )
    }
}

type EdgeKey = (String, String, Option<String>, Option<String>, String);

/// Cho adapter: thêm, sửa, bỏ cạnh của vault riêng sau khi cạnh lõi đã tính.
pub struct Ctx<'a> {
    pub db: &'a Connection,
    pub vault: &'a Path,
    pub resolver: &'a Resolver,
    pub schema: &'a Schema,
    /// Cạnh theo thứ tự sẽ ghi; sửa cạnh có sẵn thì sửa thẳng ở đây.
    pub rows: Vec<EdgeRow>,
    seen: HashSet<EdgeKey>,
}

impl<'a> Ctx<'a> {
    fn new(
        db: &'a Connection,
        vault: &'a Path,
        resolver: &'a Resolver,
        schema: &'a Schema,
        rows: Vec<EdgeRow>,
    ) -> Self {
        let seen = rows.iter().map(EdgeRow::key).collect();
        Ctx {
            db,
            vault,
            resolver,
            schema,
            rows,
            seen,
        }
    }

    /// Thêm một cạnh; trùng khoá `(src, rel, đích, claim, origin)` thì bỏ, trả `false`.
    pub fn add(&mut self, e: EdgeRow) -> bool {
        if !self.seen.insert(e.key()) {
            return false;
        }
        self.rows.push(e);
        true
    }

    /// Bỏ mọi cạnh mà `pred` đúng; trả số cạnh bỏ.
    pub fn drop_where(&mut self, pred: impl Fn(&EdgeRow) -> bool) -> usize {
        let n = self.rows.len();
        self.rows.retain(|r| !pred(r));
        self.seen = self.rows.iter().map(EdgeRow::key).collect();
        n - self.rows.len()
    }
}

// ------------------------------------------------------------------------------------------------ build

/// Một lần build.
pub struct Build<'a> {
    /// Gói schema bật.
    pub packs: Vec<String>,
    pub opts: Options,
    /// Dựng lại hết thay vì tăng dần.
    pub full: bool,
    pub adapter: Option<&'a dyn Adapter>,
    pub schema: Source,
    /// `skip(đường dẫn)` đúng thì bỏ note khỏi index.
    pub skip: Option<&'a dyn Fn(&str) -> bool>,
    /// Danh sách note đã liệt kê sẵn (thay cho `vault::list_notes`).
    pub files: Option<&'a BTreeMap<String, NoteFile>>,
}

impl Default for Build<'_> {
    fn default() -> Self {
        Build {
            packs: Vec::new(),
            opts: Options::default(),
            full: false,
            adapter: None,
            schema: Source::Embedded,
            skip: None,
            files: None,
        }
    }
}

/// Kết quả một lần build: các dòng của `meta` theo thứ tự của Python, cùng số cạnh theo status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInfo {
    entries: Vec<(&'static str, String)>,
    counts: IndexMap<String, i64>,
}

impl BuildInfo {
    /// Giá trị của một khoá `meta` (`mode`, `notes_read`, `edges`, ...).
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Mọi khoá theo thứ tự (như dict `info` mà `build --json` in).
    pub fn entries(&self) -> &[(&'static str, String)] {
        &self.entries
    }

    /// Số cạnh theo status, theo thứ tự gặp lần đầu.
    pub fn counts(&self) -> &IndexMap<String, i64> {
        &self.counts
    }

    /// Hai dòng `build` in khi không `--json`; `db` là đường dẫn DB như người dùng ghi.
    pub fn summary(&self, db: &str) -> String {
        let g = |k| self.get(k).unwrap_or_default();
        let counts: Vec<String> = self
            .counts
            .iter()
            .map(|(k, v)| format!("{}: {v}", crate::vault::yaml::repr::str_repr(k)))
            .collect();
        format!(
            "{db} ({}): {} note, đọc {}, xoá {}; ms {}\ncạnh theo lớp: {{{}}}\n",
            g("mode"),
            g("notes"),
            g("notes_read"),
            g("notes_removed"),
            g("ms"),
            counts.join(", ")
        )
    }
}

/// `round(ms, 1)` in như `repr(float)` của Python.
fn ms(d: std::time::Duration) -> String {
    let x = (d.as_secs_f64() * 1000.0 * 10.0).round() / 10.0;
    crate::vault::yaml::repr::float_repr(x)
}

/// `json.dumps(counts)`.
fn dumps_counts(counts: &IndexMap<String, i64>) -> String {
    let parts: Vec<String> = counts
        .iter()
        .map(|(k, v)| format!("{}: {v}", py::json_str(k, true)))
        .collect();
    format!("{{{}}}", parts.join(", "))
}

/// `os.path.abspath`: đường dẫn tuyệt đối, bỏ `.` và `..` theo chữ.
fn abspath(p: &Path) -> Result<PathBuf, IndexError> {
    let abs = std::path::absolute(p).map_err(io_err(p))?;
    let mut out = PathBuf::new();
    for c in abs.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    Ok(out)
}

/// Dựng hoặc cập nhật index của `vault` vào `db`. Đổi gói hoặc cách đọc so với lần trước thì dựng lại hết.
pub fn build(vault: &Path, db_path: &Path, b: &Build<'_>) -> Result<BuildInfo, IndexError> {
    let t0 = Instant::now();
    let _lock = BuildLock::acquire(db_path)?;
    let files: Cow<'_, BTreeMap<String, NoteFile>> = match b.files {
        Some(f) => Cow::Borrowed(f),
        None => Cow::Owned(vault::list_notes(vault, b.skip).map_err(io_err(vault))?),
    };
    let t_list = t0.elapsed();
    let mut db = connect(db_path, false)?;
    set_wal(&db);
    db.execute_batch(DDL.replacen("PRAGMA journal_mode=WAL;", "", 1).as_str())?;
    let meta: HashMap<String, Option<String>> = {
        let mut st = db.prepare("SELECT key, value FROM meta")?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
    let meta_get = |k: &str| meta.get(k).map(|v| v.as_deref());
    let packs = b.packs.join(",");
    let mut full = b.full;
    if !meta.is_empty()
        && (meta_get("packs") != Some(Some(&packs))
            || meta_get("options") != Some(Some(&b.opts.key())))
    {
        full = true; // đổi gói hoặc cách đọc: dựng lại hết
    }
    let old: HashMap<String, (Option<i64>, Option<i64>)> = if full {
        HashMap::new()
    } else {
        let mut st = db.prepare("SELECT id, mtime, size FROM nodes")?;
        st.query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?
            .collect::<rusqlite::Result<_>>()?
    };
    let mut changed: Vec<&String> = files
        .iter()
        .filter(|(p, f)| {
            old.get(*p)
                != Some(&(
                    Some(f.mtime),
                    Some(i64::try_from(f.size).unwrap_or(i64::MAX)),
                ))
        })
        .map(|(p, _)| p)
        .collect();

    let t1 = Instant::now();
    // Đọc và parse trước khi mở giao dịch ghi, để không giữ khoá ghi trong lúc đọc đĩa. File biến mất giữa lúc
    // liệt kê và lúc đọc (Obsidian ghi đè bằng rename, công cụ đồng bộ) coi như note đã xoá.
    let mut parsed = Vec::with_capacity(changed.len());
    let mut vanished: HashSet<&String> = HashSet::new();
    for p in &changed {
        let f = &files[*p];
        match vault::read(&f.path) {
            Ok(text) => parsed.push((*p, f, vault::parse(&text))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                vanished.insert(*p);
            }
            Err(e) => return Err(io_err(&f.path)(e)),
        }
    }
    changed.retain(|p| !vanished.contains(p));
    let removed: Vec<&String> = old
        .keys()
        .filter(|p| !files.contains_key(*p) || vanished.contains(p))
        .collect();
    let dirty = meta_get(DIRTY).is_some();
    {
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if full {
            for t in NOTE_TABLES {
                tx.execute(&format!("DELETE FROM {t}"), [])?;
            }
        } else {
            let gone: BTreeSet<&String> = changed.iter().chain(&removed).copied().collect();
            for p in gone {
                for (t, c) in [
                    ("nodes", "id"),
                    ("props", "node"),
                    ("raw_links", "src"),
                    ("blocks", "node"),
                    ("names", "node"),
                ] {
                    tx.execute(&format!("DELETE FROM {t} WHERE {c} = ?"), [p])?;
                }
            }
        }
        for (p, f, r) in &parsed {
            write_note(&tx, p, f, r)?;
        }
        if full || !changed.is_empty() || !removed.is_empty() {
            // Cạnh sẽ được tính lại ở giao dịch sau; build hỏng giữa chừng thì lần sau vẫn phải tính lại.
            tx.execute("INSERT OR REPLACE INTO meta VALUES (?, '1')", [DIRTY])?;
        }
        tx.commit()?;
    }
    let t_parse = t1.elapsed();

    let t2 = Instant::now();
    let mut sdig = schema::digest(&b.schema)?;
    let adapter_digest = b.adapter.map(|a| a.digest(vault));
    if let Some(a) = b.adapter {
        sdig.push('+');
        sdig.push_str(a.name().unwrap_or("adapter"));
        if let Some(Some(d)) = &adapter_digest {
            sdig.push(':');
            sdig.push_str(d);
        }
    }
    let (counts, version, problems) = if !changed.is_empty()
        || !removed.is_empty()
        || full
        || dirty
        || meta_get("schema_digest") != Some(Some(&sdig))
        || matches!(adapter_digest, Some(None))
    {
        let pack_refs: Vec<&str> = b.packs.iter().map(String::as_str).collect();
        let (s, tables) = schema::load(&b.schema, &pack_refs)?;
        db.execute_batch(schema::DDL)?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        write_schema(&tx, &tables)?;
        let counts = derive(&tx, vault, &s, &b.opts, b.adapter)?;
        validate::create_views(&tx)?;
        tx.commit()?;
        (counts, s.version.clone(), s.problems.clone())
    } else {
        // Không có gì đổi: giữ cạnh và view.
        let missing = |k: &str| IndexError::Adapter(format!("meta của index thiếu {k}"));
        let edges = meta_get("edges")
            .flatten()
            .ok_or_else(|| missing("edges"))?;
        let broken = |e: String| IndexError::Adapter(format!("meta.edges hỏng: {e}"));
        let parsed: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(edges).map_err(|e| broken(e.to_string()))?;
        let counts = parsed
            .into_iter()
            .map(|(k, v)| {
                v.as_i64()
                    .map(|n| (k, n))
                    .ok_or_else(|| broken("số đếm".into()))
            })
            .collect::<Result<IndexMap<String, i64>, _>>()?;
        let version = meta_get("schema_version")
            .flatten()
            .ok_or_else(|| missing("schema_version"))?
            .to_owned();
        let problems = meta_get("schema_problems")
            .flatten()
            .ok_or_else(|| missing("schema_problems"))?
            .split("; ")
            .filter(|p| !p.is_empty())
            .map(str::to_owned)
            .collect();
        (counts, version, problems)
    };
    let t_edges = t2.elapsed();

    let (name, spec, hidden) = match b.adapter {
        Some(a) => (
            a.name().unwrap_or_default().to_owned(),
            a.spec().to_owned(),
            py::json_list(a.hidden().iter().copied(), false),
        ),
        None => (String::new(), String::new(), "[]".to_owned()),
    };
    let mode = if full || old.is_empty() {
        "full"
    } else {
        "incremental"
    };
    let timing = format!(
        "{{\"list\": {}, \"parse\": {}, \"edges_schema_views\": {}, \"total\": {}}}",
        ms(t_list),
        ms(t_parse),
        ms(t_edges),
        ms(t0.elapsed())
    );
    let entries = vec![
        ("vault", abspath(vault)?.to_string_lossy().into_owned()),
        ("packs", packs),
        ("options", b.opts.key()),
        ("schema_version", version),
        ("schema_digest", sdig),
        (
            "built_at",
            chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string(),
        ),
        ("mode", mode.to_owned()),
        ("notes", (files.len() - vanished.len()).to_string()),
        ("notes_read", changed.len().to_string()),
        ("notes_removed", removed.len().to_string()),
        ("adapter", name),
        ("adapter_spec", spec),
        ("hidden", hidden),
        ("ms", timing),
        ("edges", dumps_counts(&counts)),
        ("schema_problems", problems.join("; ")),
    ];
    {
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("DELETE FROM meta", [])?;
        {
            let mut st = tx.prepare("INSERT INTO meta VALUES (?, ?)")?;
            for (k, v) in &entries {
                st.execute(params![k, v])?;
            }
        }
        tx.commit()?;
    }
    db.execute_batch("PRAGMA optimize")?;
    db.close().map_err(|(_, e)| IndexError::Sql(e))?;
    Ok(BuildInfo { entries, counts })
}

/// Ghi các dòng của một note đã parse vào `nodes`, `props`, `raw_links`, `blocks`, `names`.
fn write_note(tx: &Connection, p: &str, f: &NoteFile, r: &vault::Parsed) -> Result<(), IndexError> {
    let types = r.types();
    let title = vault::title(p);
    tx.prepare_cached("INSERT INTO nodes VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")?
        .execute(params![
            p,
            title,
            if types.len() == 1 { types[0] } else { None },
            types.len() as i64,
            vault::folder(p),
            r.first("status"),
            r.first("valid_from"),
            r.first("valid_to"),
            r.first("proposed_by"),
            r.first("extracted_by"),
            r.first("asserted_at"),
            f.mtime,
            i64::try_from(f.size).unwrap_or(i64::MAX),
            r.error,
        ])?;
    let mut st = tx.prepare_cached("INSERT INTO props VALUES (?, ?, ?)")?;
    for (k, v) in &r.props {
        st.execute(params![p, k, v])?;
    }
    let mut st = tx.prepare_cached("INSERT INTO raw_links VALUES (?, ?, ?, ?, ?, ?)")?;
    for l in &r.links {
        st.execute(params![
            p,
            l.key,
            l.target,
            l.frag,
            l.origin.as_str(),
            i64::from(l.embed)
        ])?;
    }
    let mut st = tx.prepare_cached("INSERT OR IGNORE INTO blocks VALUES (?, ?)")?;
    for b in &r.blocks {
        st.execute(params![p, b])?;
    }
    let mut st = tx.prepare_cached("INSERT INTO names VALUES (?, ?, ?, ?)")?;
    st.execute(params![p, title, "title", vault::fold(title)])?;
    for a in r.aliases() {
        st.execute(params![p, a, "alias", vault::fold(a)])?;
    }
    Ok(())
}

/// Ghi lại các bảng `schema_*` (như `schema.load`, phần SQLite).
fn write_schema(tx: &Connection, tables: &[schema::Table]) -> Result<(), IndexError> {
    for t in schema::TABLES {
        tx.execute(&format!("DELETE FROM {t}"), [])?;
    }
    for t in tables {
        let verb = if t.name == "schema_type_ancestor" {
            "INSERT OR IGNORE"
        } else {
            "INSERT"
        };
        let marks = vec!["?"; t.columns.len()].join(", ");
        let mut st = tx.prepare(&format!("{verb} INTO {} VALUES ({marks})", t.name))?;
        for row in &t.rows {
            let vals = row.iter().map(|c| match c {
                Cell::Null => SqlValue::Null,
                Cell::Int(i) => SqlValue::Integer(*i),
                Cell::Text(s) => SqlValue::Text(s.clone()),
            });
            st.execute(rusqlite::params_from_iter(vals))?;
        }
    }
    Ok(())
}

/// Cột của `nodes` mà phần tính cạnh dùng.
struct NodeRow {
    ty: Option<String>,
    status: Option<String>,
    valid_from: Option<String>,
    valid_to: Option<String>,
    proposed_by: Option<String>,
    extracted_by: Option<String>,
    asserted_at: Option<String>,
}

impl NodeRow {
    /// `n[5] or 'human'`.
    fn proposer(&self) -> String {
        self.proposed_by
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "human".to_owned())
    }
}

/// Một dòng của `links`.
struct LinkRow {
    src: String,
    key: String,
    target: String,
    frag: String,
    origin: String,
    embed: i64,
    dst: Option<String>,
    via: Option<&'static str>,
}

/// Giải lại mọi link và tính lại mọi cạnh từ phần đã parse; trả số cạnh theo status.
fn derive(
    tx: &Connection,
    vault: &Path,
    s: &Schema,
    opts: &Options,
    adapter: Option<&dyn Adapter>,
) -> Result<IndexMap<String, i64>, IndexError> {
    let nodes: IndexMap<String, NodeRow> = {
        let mut st = tx.prepare(
            "SELECT id, type, status, valid_from, valid_to, proposed_by, extracted_by, asserted_at FROM nodes",
        )?;
        st.query_map([], |r| {
            Ok((
                r.get(0)?,
                NodeRow {
                    ty: r.get(1)?,
                    status: r.get(2)?,
                    valid_from: r.get(3)?,
                    valid_to: r.get(4)?,
                    proposed_by: r.get(5)?,
                    extracted_by: r.get(6)?,
                    asserted_at: r.get(7)?,
                },
            ))
        })?
        .collect::<rusqlite::Result<_>>()?
    };
    let aliases: Option<Vec<(String, String)>> = if opts.aliases {
        let mut st = tx.prepare("SELECT node, name FROM names WHERE kind = 'alias'")?;
        Some(
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?,
        )
    } else {
        None
    };
    let resolver = Resolver::new(nodes.keys().map(String::as_str), aliases.as_deref());
    let edge_keys: HashSet<&str> = s
        .edges
        .iter()
        .filter(|(_, v)| v.kind == EdgeKind::Edge)
        .map(|(k, _)| k.as_str())
        .collect();

    let mut links = Vec::new();
    {
        let mut st = tx.prepare("SELECT src, key, target, frag, origin, embed FROM raw_links")?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let (src, target, origin): (String, String, String) = (r.get(0)?, r.get(2)?, r.get(4)?);
            let (dst, via) = resolver.resolve(&src, &target, origin == "body-md");
            links.push(LinkRow {
                src,
                key: r.get(1)?,
                target,
                frag: r.get(3)?,
                origin,
                embed: r.get(5)?,
                dst,
                via: via.map(|v| v.as_str()),
            });
        }
    }
    tx.execute("DELETE FROM links", [])?;
    {
        let mut st = tx.prepare("INSERT INTO links VALUES (?, ?, ?, ?, ?, ?, ?, ?)")?;
        for l in &links {
            st.execute(params![
                l.src, l.key, l.target, l.frag, l.origin, l.embed, l.dst, l.via
            ])?;
        }
    }

    let mut rows: Vec<EdgeRow> = Vec::new();
    let claims: BTreeSet<&str> = nodes
        .iter()
        .filter(|(_, r)| r.ty.as_deref() == Some("claim"))
        .map(|(n, _)| n.as_str())
        .collect();
    let end_fields: HashSet<&str> = s
        .claim_ends
        .values()
        .flat_map(|(a, b)| [a.as_str(), b.as_str()])
        .collect();
    for l in &links {
        if l.via == Some("attachment") {
            continue;
        }
        let Some(n) = nodes.get(&l.src) else {
            continue;
        };
        let (key, origin) = if l.origin == "frontmatter" {
            if l.key == "aliases"
                || (opts.edge_keys == EdgeKeys::Schema && !edge_keys.contains(l.key.as_str()))
            {
                continue;
            }
            if claims.contains(l.src.as_str()) && end_fields.contains(l.key.as_str()) {
                continue; // hai đầu của cạnh claim, không là cạnh riêng
            }
            (l.key.as_str(), "frontmatter")
        } else {
            if !opts.body_edges || l.via == Some("self") || l.dst.as_deref() == Some(&l.src) {
                continue;
            }
            ("link", "body")
        };
        let status = n
            .status
            .as_deref()
            .filter(|st| matches!(*st, "proposed" | "rejected"))
            .unwrap_or("accepted");
        let mut e = EdgeRow::new(&l.src, key, origin);
        e.dst = l.dst.clone();
        e.dst_text = Some(format!("{}{}", l.target, l.frag));
        e.status = Some(status.to_owned());
        e.valid_from = n.valid_from.clone();
        e.valid_to = n.valid_to.clone();
        e.asserted_at = n.asserted_at.clone();
        e.source = Some(l.src.clone());
        e.proposed_by = Some(n.proposer());
        e.extracted_by = n.extracted_by.clone();
        rows.push(e);
    }

    // Cạnh claim
    let mut cprops: HashMap<String, HashMap<String, Vec<Option<String>>>> = HashMap::new();
    {
        let mut st = tx.prepare(
            "SELECT p.node, p.key, p.value FROM props p JOIN nodes n ON n.id = p.node WHERE n.type = 'claim'",
        )?;
        let mut q = st.query([])?;
        while let Some(r) = q.next()? {
            cprops
                .entry(r.get(0)?)
                .or_default()
                .entry(r.get(1)?)
                .or_default()
                .push(r.get(2)?);
        }
    }
    type End<'l> = (Option<&'l str>, &'l str, &'l str);
    let mut clinks: HashMap<&str, HashMap<&str, Vec<End<'_>>>> = HashMap::new();
    for l in &links {
        if claims.contains(l.src.as_str()) && l.origin == "frontmatter" {
            clinks
                .entry(l.src.as_str())
                .or_default()
                .entry(l.key.as_str())
                .or_default()
                .push((l.dst.as_deref(), &l.target, &l.frag));
        }
    }
    let empty_props = HashMap::new();
    let empty_links = HashMap::new();
    for c in &claims {
        let f = cprops.get(*c).unwrap_or(&empty_props);
        let ends_of = clinks.get(c).unwrap_or(&empty_links);
        let first = |k: &str| f.get(k).and_then(|v| v.first().cloned()).flatten();
        let Some(rel) = first("relation") else {
            continue;
        };
        let Some((from, to)) = s.claim_ends.get(&rel) else {
            continue;
        };
        let (Some(a), Some(b)) = (
            ends_of.get(from.as_str()).and_then(|v| v.first()),
            ends_of.get(to.as_str()).and_then(|v| v.first()),
        ) else {
            continue;
        };
        let (Some(a_dst), Some(b_dst)) = (a.0, b.0) else {
            continue;
        };
        let n = &nodes[*c];
        let source: Vec<String> = ends_of
            .get("source")
            .into_iter()
            .flatten()
            .map(|(d, t, fr)| format!("{}{fr}", d.unwrap_or(t)))
            .collect();
        let source = source.join(" | ");
        let mut e = EdgeRow::new(a_dst, rel, "claim");
        e.dst = Some(b_dst.to_owned());
        e.dst_text = Some(b.1.to_owned());
        e.claim = Some((*c).to_owned());
        e.status = n.status.clone();
        e.valid_from = n.valid_from.clone();
        e.valid_to = n.valid_to.clone();
        e.asserted_at = n.asserted_at.clone();
        e.source = (!source.is_empty()).then_some(source);
        e.proposed_by = Some(n.proposer());
        e.extracted_by = n.extracted_by.clone();
        e.confidence = first("confidence");
        e.score = first("score")
            .and_then(|x| py::float(&x))
            .map_or(SqlValue::Null, SqlValue::Real);
        rows.push(e);
    }
    // bỏ trùng như nhau (một link ghi hai lần)
    let mut seen = HashSet::new();
    rows.retain(|r| seen.insert(r.key()));
    if let Some(a) = adapter {
        let mut ctx = Ctx::new(tx, vault, &resolver, s, rows);
        a.edges(&mut ctx)?;
        rows = ctx.rows;
    }
    tx.execute("DELETE FROM edges", [])?;
    {
        let mut st = tx.prepare(&format!(
            "INSERT INTO edges({}) VALUES ({})",
            EDGE_COLS.join(", "),
            vec!["?"; EDGE_COLS.len()].join(", ")
        ))?;
        for r in &rows {
            st.execute(params![
                r.src,
                r.rel,
                r.dst,
                r.dst_text,
                r.claim,
                r.status,
                r.valid_from,
                r.valid_to,
                r.asserted_at,
                r.source,
                r.origin,
                r.proposed_by,
                r.extracted_by,
                r.confidence,
                r.score,
            ])?;
        }
    }
    let mut out: IndexMap<String, i64> = IndexMap::new();
    for r in &rows {
        let k = r
            .status
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "(none)".to_owned());
        *out.entry(k).or_default() += 1;
    }
    Ok(out)
}

// ------------------------------------------------------------------------------------------------ đọc bảng

/// Một bảng (hoặc view `v3_deviations`) của index, như bộ so khớp dump.
#[derive(Debug, Clone, PartialEq)]
pub struct TableDump {
    pub name: String,
    /// `table` hoặc `view`.
    pub kind: &'static str,
    pub columns: Vec<String>,
    /// Cột khoá chính theo thứ tự trong khoá.
    pub key: Vec<String>,
    /// Dòng, sắp theo mọi cột (thứ tự của SQLite: NULL, số, chuỗi).
    pub rows: Vec<Vec<serde_json::Value>>,
}

/// Mọi bảng của index trừ bảng nội bộ `sqlite_*`, theo tên, rồi view `v3_deviations`.
pub fn dump_tables(db: &Connection) -> Result<Vec<TableDump>, IndexError> {
    let names: Vec<(String, &'static str)> = {
        let mut st = db.prepare(
            "SELECT name, type FROM sqlite_master WHERE (type = 'table' AND name NOT LIKE 'sqlite_%') \
             OR (type = 'view' AND name = 'v3_deviations') ORDER BY type, name",
        )?;
        st.query_map([], |r| {
            let kind: String = r.get(1)?;
            Ok((r.get(0)?, if kind == "view" { "view" } else { "table" }))
        })?
        .collect::<rusqlite::Result<_>>()?
    };
    let mut out = Vec::new();
    for (name, kind) in names {
        let mut cols: Vec<(String, i64)> = {
            let mut st = db.prepare(&format!("PRAGMA table_info(\"{name}\")"))?;
            st.query_map([], |r| Ok((r.get(1)?, r.get(5)?)))?
                .collect::<rusqlite::Result<_>>()?
        };
        let columns: Vec<String> = cols.iter().map(|(c, _)| c.clone()).collect();
        cols.retain(|(_, pk)| *pk > 0);
        cols.sort_by_key(|(_, pk)| *pk);
        let key = cols.into_iter().map(|(c, _)| c).collect();
        let order = (1..=columns.len())
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let mut st = db.prepare(&format!("SELECT * FROM \"{name}\" ORDER BY {order}"))?;
        let n = columns.len();
        let rows = st
            .query_map([], |r| {
                (0..n)
                    .map(|i| {
                        Ok(match r.get_ref(i)? {
                            ValueRef::Null => serde_json::Value::Null,
                            ValueRef::Integer(x) => x.into(),
                            ValueRef::Real(x) => serde_json::Number::from_f64(x)
                                .map_or(serde_json::Value::Null, serde_json::Value::Number),
                            ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned().into(),
                            ValueRef::Blob(_) => {
                                return Err(rusqlite::Error::InvalidColumnType(
                                    i,
                                    "blob".into(),
                                    rusqlite::types::Type::Blob,
                                ));
                            }
                        })
                    })
                    .collect::<rusqlite::Result<Vec<_>>>()
            })?
            .collect::<rusqlite::Result<_>>()?;
        out.push(TableDump {
            name,
            kind,
            columns,
            key,
            rows,
        });
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) mod testutil;

#[cfg(test)]
mod tests;
