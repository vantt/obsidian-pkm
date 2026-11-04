//! Nguyên liệu cho bài đang viết: note trong vault nói về các đoạn của bản nháp, xếp hạng, để người viết mở và tự
//! quyết.
//!
//! Xuất phát từ phần tính của `scripts/e11/material.py` (repo nghiên cứu):
//!
//! - **Câu tìm** ([`split_draft`], [`queries`]): mỗi đoạn có chữ của bản nháp một câu, gắn nhãn mục; bỏ mục
//!   `Nguyên liệu` và khối code (khối `base` của mẫu); tên bài là một câu trừ tên mặc định; tối đa [`MAX_QUERIES`].
//! - **Hạt giống:** mỗi câu một request `search` có `id` riêng qua cùng một tiến trình `ohs serve --stdio`, chạy đồng
//!   thời; không gộp thành `queries[]` vì làm vậy mất nhãn đoạn. Điểm chia cho điểm cao nhất của câu đó; một note giữ
//!   điểm cao nhất qua các câu cùng danh sách mục đã khớp. Hit có sẵn (`--hits`) thêm vào với nhãn `OHS (agent)`.
//!   OHS chưa sẵn sàng thì trả [`Error::Ohs`] với [`crate::ohs::OhsError::NotReady`]: không có đường lùi.
//! - **Graph** ([`crate::related::related`]): PageRank cá nhân hoá từ các hạt giống trên mọi quan hệ trừ `topic` (cạnh
//!   thư mục chỉ đưa tới note cùng thư mục và trang chủ đề); bài nháp là hạt giống khi đã có cạnh (`uses`). Note chỉ
//!   graph đưa tới kèm chuỗi cạnh, lớp, và ai đề xuất mỗi bước máy gợi ý.
//!
//! Module chỉ đọc: không ghi bài nháp, không ghi trang nguyên liệu. Phần bấm (dựng trang, giữ ô đã tích, ghi `uses`)
//! thuộc plugin.

use std::collections::{BTreeSet, HashSet};
use std::fmt;
use std::path::Path;
use std::sync::LazyLock;
use std::thread;

use indexmap::IndexMap;
use regex::Regex;
use rusqlite::types::Value as SqlValue;
use serde_json::{Map, Value, json};

use crate::adapter::e11::is_system;
use crate::ohs::{Ohs, OhsError};
use crate::pairs::{FRONTMATTER, py_round};
use crate::query::{Filter, Graph, QueryError, py};
use crate::related;
use crate::vault::{self, casefold, nfc};

/// Số câu tìm tối đa của một bản nháp.
pub const MAX_QUERIES: usize = 8;
/// Mục của mẫu bài viết không phải nội dung bài.
const SKIP_SECTIONS: [&str; 1] = ["nguyên liệu"];
/// Nhãn của hit có sẵn.
const AGENT_LABEL: &str = "OHS (agent)";

static DEFAULT_TITLES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(bài mới|untitled)( \d+)?$").expect("regex tĩnh hợp lệ"));
static CODE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)```.*?```").expect("regex tĩnh hợp lệ"));
static HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^#{1,6}\s+(.*)").expect("regex tĩnh hợp lệ"));
static PARAGRAPH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\n\s*\n").expect("regex tĩnh hợp lệ"));
static BULLET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*[-*>]\s+").expect("regex tĩnh hợp lệ"));
static MARKUP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[`*_#>\[\]]").expect("regex tĩnh hợp lệ"));
/// Wikilink (cùng mẫu với `vault`): nhóm 2 là đích.
static WIKI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(!?)\[\[([^\]|\n#]*)(#[^\]|\n]*)?(?:\|[^\]\n]*)?\]\]").expect("regex tĩnh hợp lệ")
});
/// Đoạn tô `==...==`, ít nhất ba ký tự.
static HL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"==([^=\n]{3,}?)==").expect("regex tĩnh hợp lệ"));

/// Một hit tìm kiếm: đường dẫn note (NFC) và điểm.
pub type Hit = (String, f64);

/// Lỗi của [`gather`].
#[derive(Debug)]
pub enum Error {
    /// OHS chưa sẵn sàng, báo lỗi, quá hạn hoặc trả sai định dạng.
    Ohs(OhsError),
    Query(QueryError),
    /// Bản nháp không dùng được (không có, trong thư mục hệ thống, không đọc được).
    Draft(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ohs(e) => e.fmt(f),
            Self::Query(e) => e.fmt(f),
            Self::Draft(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {}

impl From<QueryError> for Error {
    fn from(e: QueryError) -> Self {
        Self::Query(e)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Self::Query(e.into())
    }
}

/// Tham số của [`gather`] (mặc định như `material.py`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// Số note mỗi danh sách (tìm theo chữ, graph đưa tới).
    pub k: usize,
    /// Số hit mỗi câu tìm.
    pub per_query: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            k: 12,
            per_query: 10,
        }
    }
}

// ---------------------------------------------------------------------------------------------------- bản nháp

/// Đường dẫn bản nháp trong vault từ chữ người gõ (`Bài viết/X` hay `Bài viết\X.md`): NFC, `/`, đuôi `.md`. File
/// phải có và không nằm trong thư mục hệ thống.
pub fn resolve_draft(vault_dir: &Path, raw: &str) -> Result<String, Error> {
    let draft = nfc(&raw.replace('\\', "/"));
    let draft = if draft.ends_with(".md") {
        draft
    } else {
        format!("{draft}.md")
    };
    if !vault_dir.join(&draft).is_file() {
        return Err(Error::Draft(format!("không thấy bài: {draft}")));
    }
    if is_system(&draft) {
        return Err(Error::Draft(
            "bài nằm trong thư mục hệ thống; mở bản nháp (Bài viết/...) rồi chạy lại".into(),
        ));
    }
    Ok(draft)
}

/// `str.splitlines()` của Python.
fn py_lines(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        let brk = matches!(
            c,
            '\n' | '\r'
                | '\x0b'
                | '\x0c'
                | '\x1c'
                | '\x1d'
                | '\x1e'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        );
        if brk {
            out.push(&s[start..i]);
            let mut end = i + c.len_utf8();
            if c == '\r' && it.peek().is_some_and(|(_, n)| *n == '\n') {
                it.next();
                end += 1;
            }
            start = end;
        }
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

/// Frontmatter (chữ) và các mục `(tiêu đề, chữ)` của bản nháp; bỏ khối code và mục `Nguyên liệu` của mẫu. Phần
/// trước tiêu đề đầu tiên là mục có tiêu đề rỗng.
pub fn split_draft(text: &str) -> (String, Vec<(String, String)>) {
    let (fm, body) = match FRONTMATTER.captures(text) {
        Some(c) => (
            c.get(1).map_or("", |m| m.as_str()).to_owned(),
            &text[c.get(0).map_or(0, |m| m.end())..],
        ),
        None => (String::new(), text),
    };
    let body = CODE.replace_all(body, "");
    let mut parts = Vec::new();
    let (mut head, mut buf): (String, Vec<&str>) = (String::new(), Vec::new());
    for line in py_lines(&body) {
        if let Some(h) = HEADING.captures(line) {
            parts.push((std::mem::take(&mut head), buf.join("\n")));
            head = h[1].trim().to_owned();
            buf.clear();
        } else {
            buf.push(line);
        }
    }
    parts.push((head, buf.join("\n")));
    let sections = parts
        .into_iter()
        .filter(|(h, _)| !SKIP_SECTIONS.contains(&casefold(h).as_str()))
        .map(|(h, t)| (h, t.trim().to_owned()))
        .collect();
    (fm, sections)
}

/// Câu tìm `(nhãn, câu)`: tên bài (trừ tên mặc định), rồi mỗi đoạn có từ 15 ký tự chữ thật, nhãn là tiêu đề mục;
/// link thành tên note, bỏ dấu đầu dòng và ký hiệu markdown; mỗi câu tối đa 400 ký tự; tối đa [`MAX_QUERIES`] câu.
pub fn queries(title: &str, sections: &[(String, String)]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if !DEFAULT_TITLES.is_match(title.trim()) {
        out.push(("tên bài".to_owned(), title.to_owned()));
    }
    for (head, txt) in sections {
        for para in PARAGRAPH.split(txt) {
            let plain = WIKI.replace_all(para, |c: &regex::Captures<'_>| {
                c[2].rsplit('/').next().unwrap_or_default().to_owned()
            });
            let plain = BULLET.replace_all(&plain, "");
            let plain = MARKUP.replace_all(&plain, " ");
            let plain = plain.split_whitespace().collect::<Vec<_>>().join(" ");
            if plain.starts_with("Thêm note vào") || plain.chars().count() < 15 {
                continue;
            }
            let label = if head.is_empty() { "đầu bài" } else { head };
            out.push((label.to_owned(), plain.chars().take(400).collect()));
        }
    }
    out.truncate(MAX_QUERIES);
    out
}

/// Đích của các wikilink trong `text` (bỏ đích rỗng).
pub fn links_in(text: &str) -> Vec<String> {
    WIKI.captures_iter(text)
        .map(|c| c[2].trim().to_owned())
        .filter(|t| !t.is_empty())
        .collect()
}

/// Câu tìm của bản nháp `draft` (đường dẫn trong vault), như [`gather`] dùng.
pub fn draft_queries(vault_dir: &Path, draft: &str) -> Result<Vec<(String, String)>, Error> {
    let text = vault::read(&vault_dir.join(draft))
        .map_err(|e| Error::Draft(format!("không đọc được bài {draft}: {e}")))?;
    let (_, sections) = split_draft(&text);
    Ok(queries(vault::title(draft), &sections))
}

// ---------------------------------------------------------------------------------------------------- hạt giống

/// Hit có sẵn (kết quả `search` của OHS mà agent đưa): danh sách, hoặc object có `results`. Mỗi hit là đường dẫn,
/// hoặc object có `path` và `score`; không có điểm thì điểm 1.
pub fn hits_from_value(v: &Value) -> Result<Vec<Hit>, String> {
    let list = match v {
        Value::Array(a) => a,
        Value::Object(m) => match m.get("results") {
            Some(Value::Array(a)) => a,
            _ => return Err("file hit là object nhưng không có mảng results".into()),
        },
        _ => return Err("file hit phải là danh sách hoặc object có results".into()),
    };
    list.iter()
        .map(|h| match h {
            Value::String(p) => Ok((nfc(p), 1.0)),
            Value::Object(m) => {
                let p = m
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| format!("hit không có path: {h}"))?;
                let s = m.get("score").and_then(Value::as_f64).unwrap_or(0.0);
                Ok((nfc(p), if s == 0.0 { 1.0 } else { s }))
            }
            other => Err(format!("hit không hợp lệ: {other}")),
        })
        .collect()
}

/// Tìm một câu bằng OHS: tối đa `limit` hit, không lấy đoạn trích.
pub fn ohs_search(ohs: &Ohs, query: &str, limit: usize) -> Result<Vec<Hit>, OhsError> {
    let results = ohs.search(query, json!({"limit": limit, "snippetLength": 0}))?;
    Ok(results
        .iter()
        .filter_map(|r| {
            let p = r["path"].as_str()?;
            Some((p.to_owned(), r["score"].as_f64().unwrap_or(0.0)))
        })
        .take(limit)
        .collect())
}

/// Một hạt giống: điểm cao nhất (đã chia cho điểm đầu của câu) và các nhãn đã khớp.
struct Seed {
    score: f64,
    sections: Vec<String>,
}

#[derive(Default)]
struct Seeds {
    by_path: IndexMap<String, Seed>,
}

impl Seeds {
    fn add(&mut self, rows: &[Hit], label: &str) {
        let top = rows
            .iter()
            .map(|(_, s)| *s)
            .fold(None, |m: Option<f64>, s| Some(m.map_or(s, |m| m.max(s))));
        let top = match top {
            Some(t) if t != 0.0 => t,
            _ => 1.0,
        };
        for (p, s) in rows {
            let d = self.by_path.entry(p.clone()).or_insert(Seed {
                score: 0.0,
                sections: Vec::new(),
            });
            d.score = d.score.max(s / top);
            if !d.sections.iter().any(|x| x == label) {
                d.sections.push(label.to_owned());
            }
        }
    }
}

/// Chạy mọi câu tìm đồng thời; lỗi đầu tiên (theo thứ tự câu) thắng.
fn search_all<S>(
    qs: &[(String, String)],
    per_query: usize,
    search: &S,
) -> Result<Vec<Vec<Hit>>, OhsError>
where
    S: Fn(&str, usize) -> Result<Vec<Hit>, OhsError> + Sync,
{
    thread::scope(|s| {
        let handles: Vec<_> = qs
            .iter()
            .map(|(_, q)| s.spawn(move || search(q, per_query)))
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err(OhsError::Protocol("luồng tìm kiếm bị panic".into())))
            })
            .collect()
    })
}

// ---------------------------------------------------------------------------------------------------- graph

fn sql_json(v: SqlValue) -> Value {
    py::sql_to_json(v)
}

/// Nguồn gốc của một bước trong chuỗi `via` (cạnh theo chiều nào cũng được, cạnh `accepted` trước).
fn edge_info(g: &Graph, a: &str, b: &str) -> Result<Map<String, Value>, Error> {
    let mut st = g.conn().prepare(
        "SELECT proposed_by, score, origin FROM edges WHERE ((src = ? AND dst = ?) OR (src = ? AND dst = ?)) \
         AND status != 'rejected' ORDER BY status = 'accepted' DESC LIMIT 1",
    )?;
    let mut rows = st.query([a, b, b, a])?;
    let mut out = Map::new();
    if let Some(r) = rows.next()? {
        out.insert("proposed_by".into(), sql_json(r.get(0)?));
        out.insert("score".into(), sql_json(r.get(1)?));
        out.insert("origin".into(), sql_json(r.get(2)?));
    }
    Ok(out)
}

/// Số đoạn tô, hai đoạn tô đầu (160 ký tự), và `wise`, `use`, `gist`, `why` của note.
fn note_facts(vault_dir: &Path, g: &Graph, path: &str) -> Result<Map<String, Value>, Error> {
    let text = vault::read(&vault_dir.join(path)).unwrap_or_default();
    let hls: Vec<String> = HL
        .captures_iter(&text)
        .map(|c| c[1].split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    let mut out = Map::new();
    out.insert("highlights".into(), json!(hls.len()));
    out.insert(
        "first_highlights".into(),
        json!(
            hls.iter()
                .take(2)
                .map(|h| h.chars().take(160).collect::<String>())
                .collect::<Vec<_>>()
        ),
    );
    let mut st = g.conn().prepare(
        "SELECT key, value FROM props WHERE node = ? AND key IN ('wise', 'use', 'gist', 'why')",
    )?;
    let mut rows = st.query([path])?;
    while let Some(r) = rows.next()? {
        let key: String = r.get(0)?;
        out.insert(key, sql_json(r.get(1)?));
    }
    Ok(out)
}

fn strings(g: &Graph, sql: &str, param: &str) -> Result<Vec<String>, Error> {
    let mut st = g.conn().prepare(sql)?;
    let rows = st.query_map([param], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Gom nguyên liệu cho bản nháp `draft` (đường dẫn trong vault, xem [`resolve_draft`]). `search(câu, giới hạn)` tìm
/// một câu ([`ohs_search`] với OHS thật); `extra` là hit có sẵn. Trả object JSON:
///
/// `{draft, queries: [[nhãn, câu]], methods: {ohs-hits?, ohs?}, items: [...], missing, edges: {status: số},
/// draft_seed, topics: [{id, hits}]}`. Mỗi mục của `items`: `{id, type, found_by: ohs|graph|used, rank?, used,
/// sections?, ohs_score?, layer?, via?, highlights, first_highlights, wise?, use?, gist?, why?}`; note tìm theo chữ
/// trước (điểm OHS giảm dần, tối đa `k`), rồi note graph đưa tới (theo PageRank, tối đa `k`), rồi note đã dùng mà hai
/// danh sách không có.
pub fn gather<S>(
    g: &Graph,
    vault_dir: &Path,
    draft: &str,
    o: &Options,
    extra: &[Hit],
    search: &S,
) -> Result<Value, Error>
where
    S: Fn(&str, usize) -> Result<Vec<Hit>, OhsError> + Sync,
{
    let text = vault::read(&vault_dir.join(draft))
        .map_err(|e| Error::Draft(format!("không đọc được bài {draft}: {e}")))?;
    let (fm, sections) = split_draft(&text);
    let qs = queries(vault::title(draft), &sections);
    let body: Vec<&str> = sections.iter().map(|(_, t)| t.as_str()).collect();
    let mut used = BTreeSet::new();
    for name in links_in(&fm).into_iter().chain(links_in(&body.join("\n"))) {
        let r = g.find(&name, None, None)?;
        if let [one] = &r[..]
            && let Some(id) = one.get("id").and_then(Value::as_str)
        {
            used.insert(id.to_owned());
        }
    }

    let mut seeds = Seeds::default();
    let mut methods = Map::new();
    if !extra.is_empty() {
        seeds.add(extra, AGENT_LABEL);
        methods.insert("ohs-hits".into(), json!(1));
    }
    if !qs.is_empty() {
        let found = search_all(&qs, o.per_query, search).map_err(Error::Ohs)?;
        for ((label, _), rows) in qs.iter().zip(&found) {
            seeds.add(rows, label);
        }
        methods.insert("ohs".into(), json!(qs.len()));
    }
    seeds.by_path.retain(|p, _| p != draft && !is_system(p));
    let mut ranked: Vec<(&String, &Seed)> = seeds.by_path.iter().collect();
    ranked.sort_by(|a, b| b.1.score.total_cmp(&a.1.score));
    let mut hit_list: Vec<(String, f64)> = ranked
        .iter()
        .map(|(p, d)| ((*p).clone(), d.score))
        .collect();
    let has_edges = g
        .conn()
        .prepare("SELECT 1 FROM edges WHERE (src = ? OR dst = ?) AND status != ? LIMIT 1")?
        .exists([draft, draft, "rejected"])?;
    if has_edges {
        hit_list.insert(0, (draft.to_owned(), 1.0));
    }
    // Cạnh thư mục (`topic`) nối một trang chủ đề với mọi note trong thư mục: đi theo nó chỉ ra note cùng thư mục và
    // trang chủ đề, không ra quan hệ. Trang chủ đề của các hit liệt kê riêng.
    let rels: Vec<String> = {
        let mut st = g
            .conn()
            .prepare("SELECT DISTINCT rel FROM edges WHERE rel NOT IN ('topic')")?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let res = if hit_list.is_empty() {
        let mut m = Map::new();
        m.insert("seeds".into(), json!([]));
        m.insert("related".into(), json!([]));
        m.insert("missing".into(), json!([]));
        m
    } else {
        let hits: Vec<(Value, Value)> =
            hit_list.iter().map(|(p, s)| (json!(p), json!(s))).collect();
        let ro = related::Options {
            k: i64::try_from(o.k).unwrap_or(i64::MAX),
            filter: Filter::new(Some(&rels), None, None, false, false),
            ..related::Options::default()
        };
        related::related(g, &hits, &ro)?
    };
    let mut topics: IndexMap<String, u64> = IndexMap::new();
    for (p, _) in &hit_list {
        for t in strings(
            g,
            "SELECT dst FROM edges WHERE src = ? AND rel = 'topic' AND dst IS NOT NULL",
            p,
        )? {
            *topics.entry(t).or_insert(0) += 1;
        }
    }

    let rows = ["seeds", "related"].iter().flat_map(|k| {
        res.get(*k)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    });
    let mut items: Vec<Map<String, Value>> = Vec::new();
    for row in rows {
        let Some(p) = row["id"].as_str() else {
            continue;
        };
        if p == draft || is_system(p) {
            continue;
        }
        let mut it = Map::new();
        it.insert("id".into(), json!(p));
        it.insert("type".into(), row["type"].clone());
        it.insert("found_by".into(), row["found_by"].clone());
        it.insert("rank".into(), row["score"].clone());
        it.insert("used".into(), json!(used.contains(p)));
        if let Some(d) = seeds.by_path.get(p) {
            it.insert("sections".into(), json!(d.sections));
            it.insert("ohs_score".into(), json!(py_round(d.score, 3)));
        }
        if row["found_by"] == "graph" {
            it.insert("layer".into(), row["layer"].clone());
            let mut via = Vec::new();
            for s in row["via"].as_array().into_iter().flatten() {
                let mut step = s.as_object().cloned().unwrap_or_default();
                let (from, to) = (
                    step.get("from").and_then(Value::as_str).unwrap_or_default(),
                    step.get("to").and_then(Value::as_str).unwrap_or_default(),
                );
                let info = edge_info(g, from, to)?;
                step.extend(info);
                via.push(Value::Object(step));
            }
            it.insert("via".into(), Value::Array(via));
        }
        it.extend(note_facts(vault_dir, g, p)?);
        items.push(it);
    }
    // Hạt giống giữ thứ tự điểm tìm kiếm, cắt ở k; note graph đưa tới giữ thứ tự PageRank.
    let score = |i: &Map<String, Value>| i.get("ohs_score").and_then(Value::as_f64).unwrap_or(0.0);
    let mut by_ohs: Vec<Map<String, Value>> = items
        .iter()
        .filter(|i| i["found_by"] == "ohs")
        .cloned()
        .collect();
    by_ohs.sort_by(|a, b| score(b).total_cmp(&score(a)));
    by_ohs.truncate(o.k);
    let by_graph: Vec<Map<String, Value>> = items
        .into_iter()
        .filter(|i| i["found_by"] == "graph")
        .collect();
    let listed: HashSet<String> = by_ohs
        .iter()
        .chain(&by_graph)
        .filter_map(|i| i["id"].as_str().map(str::to_owned))
        .collect();
    for u in used.iter().filter(|u| !listed.contains(*u)) {
        let t = g
            .types_of(std::slice::from_ref(u))?
            .get(u)
            .cloned()
            .flatten();
        let mut it = Map::new();
        it.insert("id".into(), json!(u));
        it.insert("type".into(), json!(t));
        it.insert("found_by".into(), json!("used"));
        it.insert("used".into(), json!(true));
        it.extend(note_facts(vault_dir, g, u)?);
        by_ohs.push(it);
    }
    let mut edges = Map::new();
    {
        let mut st = g
            .conn()
            .prepare("SELECT status, count(*) FROM edges GROUP BY 1")?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let status: Option<String> = r.get(0)?;
            let n: i64 = r.get(1)?;
            edges.insert(status.unwrap_or_else(|| "null".into()), json!(n));
        }
    }
    let mut top: Vec<(String, u64)> = topics.into_iter().collect();
    top.sort_by_key(|t| std::cmp::Reverse(t.1));
    let topics: Vec<Value> = top
        .into_iter()
        .take(5)
        .map(|(t, c)| json!({"id": t, "hits": c}))
        .collect();
    let items: Vec<Value> = by_ohs
        .into_iter()
        .chain(by_graph)
        .map(Value::Object)
        .collect();
    Ok(json!({
        "draft": draft,
        "queries": qs.iter().map(|(l, q)| json!([l, q])).collect::<Vec<_>>(),
        "methods": methods,
        "items": items,
        "missing": res.get("missing").cloned().unwrap_or(json!([])),
        "edges": edges,
        "draft_seed": has_edges,
        "topics": topics,
    }))
}

#[cfg(test)]
mod tests;
