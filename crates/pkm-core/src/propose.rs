//! Đề xuất của máy thành hàng chờ duyệt: tính đề xuất từ vault và ghi mỗi đề xuất mới thành một note trong
//! `_Đề xuất/` (`type: proposal`, `status: chờ`). Không sửa note nào đang có; nhận, bỏ, áp dụng, hoàn tác và đo
//! duyệt là việc của plugin.
//!
//! Điểm xuất phát là `scripts/e11/propose.py` của repo nghiên cứu (`9061581`); chỗ cố ý khác ghi ở spec/PORTING.md,
//! mục "Module `propose`". Bảy kiểu, luật cố định, không gọi model:
//! - `duplicate`: note cùng thân (bỏ khác biệt khoảng trắng), dài hơn 40 ký tự;
//! - `same-topic`: trang chủ đề (`type: toc`) cùng tên khi bỏ dấu;
//! - `fix-link`: `[[link]]` gãy mà đúng một note trùng tên khi bỏ dấu và dấu câu;
//! - `type-batch`: mỗi thư mục một lô note có `proposed_type`;
//! - `related`: cặp gần nghĩa đọc từ file (đầu ra tìm cặp gần nghĩa của OHS);
//! - `idea`: note có đoạn tô `==...==`, nhiều đoạn trước;
//! - `person`: tên người Việt đầy đủ ở từ hai note trở lên (bỏ ngữ cảnh địa chỉ, che dãy số dài).
//!
//! `idea` và `person` giữ tối đa [`CAP`] đề xuất đang chờ mỗi kiểu, để hàng chờ không thành một hộp thư chưa đọc.

mod dump;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::Path;
use std::sync::LazyLock;

use indexmap::IndexMap;
use regex::Regex;
use serde_json::{Map, Value, json};
use unicode_normalization::UnicodeNormalization;

use crate::vault::yaml::{self, Doc, Loader, Val};
use crate::vault::{self, nfc};
use dump::Node;

/// Thư mục hàng chờ trong vault.
pub const QUEUE: &str = "_Đề xuất";
/// Thư mục note nháp mà đề xuất `idea` tạo khi được nhận.
pub const IDEAS: &str = "Ý";
/// Thư mục trang người mà đề xuất `person` tạo khi được nhận.
pub const PEOPLE: &str = "Người";
/// Số đề xuất đang chờ tối đa của mỗi kiểu có trần.
pub const CAP: [(Kind, usize); 2] = [(Kind::Idea, 20), (Kind::Person, 20)];
/// Trạng thái của đề xuất mới.
pub const OPEN: &str = "chờ";

const OHS_MODEL: &str = "multilingual-e5-small";
/// Nhiều tên ở đúng cùng một nhóm note từ chừng này trở lên là một danh sách (danh sách lớp, nhân vật mẫu), không
/// phải người dùng quen.
const ROSTER: usize = 5;
const EMBED_MAX: usize = 3;
const IDEA_QUOTES: usize = 12;
const NAME_MAX: usize = 90;
const SKIP_DIRS: [&str; 5] = ["_E11", "_Templates", "_Xưởng", "_ohs", QUEUE];

fn re(p: &str) -> Regex {
    Regex::new(p).expect("regex tĩnh hợp lệ")
}

static FM_RE: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?s)\A---[ \t]*\r?\n(.*?)\r?\n---[ \t]*\r?\n"));
static HIGHLIGHT: LazyLock<Regex> = LazyLock::new(|| re(r"==([^=\n]{8,}?)=="));
/// `[[đích`; chỗ khớp đứng sau `!` (nhúng) bị loại trong [`wikilink_targets`] (Python: `(?<!!)`).
static WIKILINK: LazyLock<Regex> = LazyLock::new(|| re(r"\[\[([^\]|#]+)"));
static ATTACH: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)\.(png|jpe?g|gif|pdf|mp3|mp4|pptx|docx|bin|base|webp|svg)$"));
static NAME: LazyLock<Regex> = LazyLock::new(|| {
    let surnames = "Nguyễn Trần Lê Phạm Hoàng Huỳnh Phan Vũ Võ Đặng Bùi Đỗ Hồ Ngô Dương Lý"
        .split(' ')
        .collect::<Vec<_>>()
        .join("|");
    re(&format!(r"\b(?:{surnames})(?: [A-ZĐ][a-zà-ỹ]+){{2,3}}\b"))
});
/// `\s` của Python có thêm U+001C..U+001F.
static ADDRESS: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"(?i)(đường|phố|quận|huyện|phường|tp\.?|thành phố|trường|thpt|thcs|tiểu học|số \d+)[\s\x1c-\x1f]*$",
    )
});
/// Dãy số dài (số giấy tờ, số điện thoại) trong đoạn trích.
static DIGITS: LazyLock<Regex> = LazyLock::new(|| re(r"\d[\d .-]{5,}\d"));
/// Phần bỏ khi so tên: mọi thứ không phải chữ, gồm `_` và chữ số thập phân (Python `[\W_\d]+`).
static NOT_LETTER: LazyLock<Regex> = LazyLock::new(|| re(r"[^\p{L}\p{Nl}\p{No}]+"));
static MARK: LazyLock<Regex> = LazyLock::new(|| re(r"\p{Mn}"));
static UNSAFE_NAME: LazyLock<Regex> = LazyLock::new(|| re(r#"[\\/:*?"<>|#^\[\]]"#));

/// Kiểu đề xuất.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Duplicate,
    SameTopic,
    FixLink,
    TypeBatch,
    Related,
    Idea,
    Person,
}

impl Kind {
    pub const ALL: [Kind; 7] = [
        Kind::Duplicate,
        Kind::SameTopic,
        Kind::FixLink,
        Kind::TypeBatch,
        Kind::Related,
        Kind::Idea,
        Kind::Person,
    ];

    /// Tên trong `kind` của đề xuất.
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Duplicate => "duplicate",
            Kind::SameTopic => "same-topic",
            Kind::FixLink => "fix-link",
            Kind::TypeBatch => "type-batch",
            Kind::Related => "related",
            Kind::Idea => "idea",
            Kind::Person => "person",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.as_str() == s)
    }

    /// Tiêu đề trong tên file và dòng đầu của note đề xuất.
    pub fn title(self) -> &'static str {
        match self {
            Kind::Duplicate => "Trùng",
            Kind::SameTopic => "Cùng chủ đề",
            Kind::FixLink => "Sửa liên kết",
            Kind::TypeBatch => "Gán loại",
            Kind::Related => "Liên quan",
            Kind::Idea => "Rút ý",
            Kind::Person => "Người",
        }
    }

    /// Trần số đề xuất đang chờ của kiểu này (nếu có).
    pub fn cap(self) -> Option<usize> {
        CAP.iter().find(|(k, _)| *k == self).map(|(_, n)| *n)
    }

    /// `proposed_by` và `extracted_by` (schema v3): embedding của OHS cho `related`, luật cố định cho kiểu khác.
    /// Giữ tên luật `propose.py` để đề xuất cũ và mới của cùng một luật gộp được khi đo.
    pub fn provenance(self) -> (&'static str, String) {
        match self {
            Kind::Related => ("model", format!("{OHS_MODEL}@schema_v3")),
            k => ("rule", format!("propose.py {}@schema_v3", k.as_str())),
        }
    }
}

/// Một đề xuất. Trường tuỳ kiểu: `from_text`, `to` (`fix-link`); `value` (`type-batch`, `person`); `score`
/// (`related`, có thể là `null`); `lines` (`idea`, `person`: phần thân thay cho nhúng note).
#[derive(Debug, Clone, PartialEq)]
pub struct Proposal {
    pub kind: Kind,
    /// Đường dẫn note liên quan (tương đối, `/`, NFC). `duplicate`: bản giữ trước; `fix-link`: note nguồn rồi đích.
    pub notes: Vec<String>,
    pub from_text: Option<String>,
    pub to: Option<String>,
    pub value: Option<String>,
    pub score: Option<Value>,
    pub reason: String,
    pub on_accept: String,
    pub lines: Option<Vec<String>>,
}

impl Proposal {
    fn new(kind: Kind, notes: Vec<String>, reason: String, on_accept: String) -> Proposal {
        Proposal {
            kind,
            notes,
            from_text: None,
            to: None,
            value: None,
            score: None,
            reason,
            on_accept,
            lines: None,
        }
    }

    /// Khoá chống trùng: kiểu và các note, như khoá đọc lại từ note đề xuất trong hàng chờ.
    pub fn key(&self) -> String {
        nfc(&format!("{}|{}", self.kind.as_str(), self.notes.join("|")))
    }

    /// Tên note của đề xuất (chưa có `.md`, chưa chống trùng tên).
    pub fn subject(&self) -> &str {
        match (&self.value, self.kind) {
            (Some(v), Kind::Person) => v,
            _ => self.notes.first().map_or("", |n| vault::title(n)),
        }
    }

    /// JSON của đề xuất: `kind`, `notes`, trường tuỳ kiểu, `reason`, `on_accept`, `lines`.
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("kind".into(), json!(self.kind.as_str()));
        m.insert("notes".into(), json!(self.notes));
        for (k, v) in [
            ("from_text", &self.from_text),
            ("to", &self.to),
            ("value", &self.value),
        ] {
            if let Some(v) = v {
                m.insert(k.into(), json!(v));
            }
        }
        if let Some(s) = &self.score {
            m.insert("score".into(), s.clone());
        }
        m.insert("reason".into(), json!(self.reason));
        m.insert("on_accept".into(), json!(self.on_accept));
        if let Some(l) = &self.lines {
            m.insert("lines".into(), json!(l));
        }
        Value::Object(m)
    }
}

/// Một cặp gần nghĩa: `{"a": path, "b": path, "score": số}` (`score` có thể thiếu).
#[derive(Debug, Clone, PartialEq)]
pub struct SimilarPair {
    pub a: String,
    pub b: String,
    pub score: Value,
}

/// Đọc file cặp gần nghĩa: một mảng JSON các object có `a`, `b` là chuỗi.
pub fn parse_similar(text: &str) -> Result<Vec<SimilarPair>, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("không đọc được JSON: {e}"))?;
    let Value::Array(items) = v else {
        return Err("cần một mảng JSON các cặp {\"a\", \"b\", \"score\"}".into());
    };
    items
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let field = |k: &str| {
                p.get(k)
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| format!("cặp thứ {i}: thiếu chuỗi `{k}`"))
            };
            Ok(SimilarPair {
                a: field("a")?,
                b: field("b")?,
                score: p.get("score").cloned().unwrap_or(Value::Null),
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------------------
// Đọc vault

struct Note {
    fm: Option<Doc>,
    body: String,
}

impl Note {
    fn get(&self, key: &str) -> Option<Val<'_>> {
        self.fm.as_ref()?.root().get(key)
    }

    fn str_of(&self, key: &str) -> Option<&str> {
        self.get(key)?.as_str()
    }

    /// `type` là `toc` hay `proposal`: trang hệ thống, không phải nội dung của người dùng.
    fn is_system(&self) -> bool {
        matches!(self.str_of("type"), Some("toc" | "proposal"))
    }
}

/// Tách frontmatter như `propose.split`: frontmatter không phải dict (rỗng, lỗi YAML) coi như không có.
fn split(text: &str) -> (Option<Doc>, &str) {
    match FM_RE.captures(text) {
        Some(c) => {
            let doc = yaml::load(&c[1], Loader::Safe)
                .ok()
                .flatten()
                .filter(|d| d.root().is_dict());
            (doc, &text[c.get(0).map_or(0, |m| m.end())..])
        }
        None => (None, text),
    }
}

/// Mọi note `.md` của vault theo đường dẫn đã sắp, trừ thư mục chấm, thư mục hệ thống ([`SKIP_DIRS`]), thư mục là
/// symlink và `.excalidraw.md`. File hay thư mục đọc lỗi bị bỏ qua.
fn load(vault: &Path) -> io::Result<BTreeMap<String, Note>> {
    let mut notes = BTreeMap::new();
    let mut stack = vec![(vault.to_path_buf(), String::new())];
    let mut top = true;
    while let Some((dir, rel)) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if top => return Err(e),
            Err(_) => continue,
        };
        top = false;
        for e in entries.flatten() {
            let name = nfc(&e.file_name().to_string_lossy());
            let path = e.path();
            let Ok(ft) = e.file_type() else { continue };
            let child = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            if ft.is_dir() {
                if !name.starts_with('.') && !SKIP_DIRS.contains(&name.as_str()) {
                    stack.push((path, child));
                }
                continue;
            }
            let is_file = ft.is_file() || (ft.is_symlink() && path.is_file());
            if !is_file || !name.ends_with(".md") || name.ends_with(".excalidraw.md") {
                continue;
            }
            let Ok(text) = vault::read(&path) else {
                continue;
            };
            let (fm, body) = split(&text);
            let body = body.to_owned();
            notes.insert(child, Note { fm, body });
        }
    }
    Ok(notes)
}

// ---------------------------------------------------------------------------------------------------------------
// Chuỗi kiểu Python

/// `str.isspace()`: khác `char::is_whitespace` ở U+001C..U+001F.
fn is_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// `' '.join(s.split())`.
fn squash(s: &str) -> String {
    s.split(is_space)
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn strip(s: &str) -> &str {
    s.trim_matches(is_space)
}

fn basename(p: &str) -> &str {
    p.rsplit_once('/').map_or(p, |(_, b)| b)
}

/// Khoá so tên: chữ thường, bỏ dấu, mọi thứ không phải chữ thành một khoảng trắng.
fn norm(t: &str) -> String {
    let t: String = t.to_lowercase().nfd().collect();
    let t = MARK.replace_all(&t, "");
    NOT_LETTER.replace_all(&t, " ").trim().to_owned()
}

/// `[[đường dẫn bỏ .md|tên]]`.
fn link(rel: &str) -> String {
    format!(
        "[[{}|{}]]",
        rel.strip_suffix(".md").unwrap_or(rel),
        vault::title(rel)
    )
}

fn folder_or_root(rel: &str) -> &str {
    match vault::folder(rel) {
        "" => "(gốc)",
        d => d,
    }
}

/// Chỉ số byte lùi `n` ký tự từ `at` (không qua đầu chuỗi).
fn back(text: &str, at: usize, n: usize) -> usize {
    if n == 0 {
        return at;
    }
    text[..at]
        .char_indices()
        .rev()
        .nth(n - 1)
        .map_or(0, |(i, _)| i)
}

/// Chỉ số byte tiến `n` ký tự từ `at` (không qua cuối chuỗi).
fn forward(text: &str, at: usize, n: usize) -> usize {
    text[at..]
        .char_indices()
        .nth(n)
        .map_or(text.len(), |(i, _)| at + i)
}

/// Đích của các `[[...` không phải nhúng (`![[...`).
fn wikilink_targets(body: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(c) = WIKILINK.captures_at(body, pos) {
        let Some(m) = c.get(0).map(|m| m.range()) else {
            break;
        };
        if body[..m.start].ends_with('!') {
            // Như `(?<!!)`: thử lại ngay ký tự sau, `![[[x` vẫn khớp ở `[[x`.
            pos = m.start + 1;
            continue;
        }
        if let Some(g) = c.get(1) {
            out.push(g.as_str());
        }
        pos = m.end;
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Luật

/// Mọi đề xuất tính được từ vault (chưa trừ đề xuất đã có trong hàng chờ, chưa áp trần). `similar`: cặp gần
/// nghĩa cho kiểu `related` (rỗng: không có kiểu đó).
pub fn proposals(vault: &Path, similar: &[SimilarPair]) -> io::Result<Vec<Proposal>> {
    let notes = load(vault)?;
    let mut out = Vec::new();
    duplicates(&notes, &mut out);
    same_topics(&notes, &mut out);
    fix_links(&notes, &mut out);
    type_batches(&notes, &mut out);
    related(&notes, similar, &mut out);
    ideas(&notes, &mut out);
    persons(&notes, &mut out);
    Ok(out)
}

fn duplicates(notes: &BTreeMap<String, Note>, out: &mut Vec<Proposal>) {
    let mut by_body: IndexMap<String, Vec<&str>> = IndexMap::new();
    for (rel, n) in notes {
        let text = squash(&n.body);
        if text.chars().count() > 40 && n.str_of("type") != Some("toc") {
            let digest = sha1_smol::Sha1::from(text.as_bytes()).digest().to_string();
            by_body.entry(digest).or_default().push(rel);
        }
    }
    for group in by_body.values().filter(|g| g.len() > 1) {
        let keep = *group
            .iter()
            .min_by_key(|r| (r.chars().count(), **r))
            .unwrap_or(&group[0]);
        let mut rest: Vec<&str> = group.iter().copied().filter(|r| *r != keep).collect();
        rest.sort_unstable();
        let folders: BTreeSet<&str> = group.iter().map(|r| folder_or_root(r)).collect();
        let mut notes = vec![keep.to_owned()];
        notes.extend(rest.iter().map(|r| (*r).to_owned()));
        out.push(Proposal::new(
            Kind::Duplicate,
            notes,
            format!(
                "{} note có nội dung giống hệt nhau, ở: {}.",
                group.len(),
                folders.into_iter().collect::<Vec<_>>().join("; ")
            ),
            format!(
                "Các bản còn lại được ghi `duplicate_of` trỏ về bản giữ ({}). Không xoá note nào.",
                vault::title(keep)
            ),
        ));
    }
}

fn same_topics(notes: &BTreeMap<String, Note>, out: &mut Vec<Proposal>) {
    let mut topics: IndexMap<String, Vec<String>> = IndexMap::new();
    for (rel, n) in notes {
        if n.str_of("type") == Some("toc") {
            let key = norm(&basename(rel).replace("(chủ đề)", ""));
            topics.entry(key).or_default().push(rel.clone());
        }
    }
    for group in topics.into_values().filter(|g| g.len() > 1) {
        let mut group = group;
        group.sort_unstable();
        out.push(Proposal::new(
            Kind::SameTopic,
            group,
            "Hai thư mục cùng tên (bỏ dấu), có thể là cùng một dự án hay chủ đề ở hai thời kỳ."
                .into(),
            "Hai trang chủ đề được nối với nhau qua `related`.".into(),
        ));
    }
}

fn fix_links(notes: &BTreeMap<String, Note>, out: &mut Vec<Proposal>) {
    let mut names: HashMap<String, Vec<&str>> = HashMap::new();
    for rel in notes.keys() {
        names
            .entry(vault::title(rel).to_lowercase())
            .or_default()
            .push(rel);
    }
    let mut by_norm: HashMap<String, usize> = HashMap::new();
    let mut only: HashMap<String, &str> = HashMap::new();
    for (k, v) in &names {
        let n = norm(k);
        *by_norm.entry(n.clone()).or_default() += v.len();
        only.insert(n, v[0]);
    }
    let mut fixes: BTreeMap<(String, String), BTreeSet<&str>> = BTreeMap::new();
    for (rel, n) in notes {
        for target in wikilink_targets(&n.body) {
            let target = strip(target);
            let base = basename(target);
            let key = nfc(base.strip_suffix(".md").unwrap_or(base)).to_lowercase();
            if ATTACH.is_match(target) || target.starts_with('@') || names.contains_key(&key) {
                continue;
            }
            let k = norm(&key);
            if !k.is_empty() && by_norm.get(&k) == Some(&1) {
                fixes
                    .entry((target.to_owned(), only[&k].to_owned()))
                    .or_default()
                    .insert(rel);
            }
        }
    }
    for ((target, cand), sources) in fixes {
        let mut notes: Vec<String> = sources.into_iter().map(str::to_owned).collect();
        notes.push(cand.clone());
        let mut p = Proposal::new(
            Kind::FixLink,
            notes,
            format!(
                "Liên kết `[[{target}]]` không trỏ tới note nào; một note có tên gần giống: {}.",
                vault::title(&cand)
            ),
            format!(
                "Trong note nguồn, `[[{target}` đổi thành `[[{}`.",
                cand.strip_suffix(".md").unwrap_or(&cand)
            ),
        );
        p.from_text = Some(target);
        p.to = Some(cand);
        out.push(p);
    }
}

fn type_batches(notes: &BTreeMap<String, Note>, out: &mut Vec<Proposal>) {
    let mut batches: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for (rel, n) in notes {
        let Some(v) = n.get("proposed_type").filter(Val::truthy) else {
            continue;
        };
        let Some(value) = v.as_str().map(str::to_owned).or_else(|| v.py_str().ok()) else {
            continue;
        };
        batches
            .entry((vault::folder(rel).to_owned(), value))
            .or_default()
            .push(rel.clone());
    }
    for ((folder, value), group) in batches {
        let place = match basename(&folder) {
            "" => "(gốc)",
            b => b,
        };
        let mut p = Proposal::new(
            Kind::TypeBatch,
            group.clone(),
            format!(
                "{} note trong thư mục {place} có `Source URL` của Evernote: đoán là bài cắt từ web.",
                group.len()
            ),
            format!(
                "Cả lô được gán `type: {value}` và bỏ `proposed_type`. Note nào không phải bài cắt thì sửa tay sau, hoặc bỏ cả lô."
            ),
        );
        p.value = Some(value);
        out.push(p);
    }
}

/// `str(x)` của Python cho giá trị JSON.
fn py_str_json(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Number(n) if n.is_f64() => yaml::repr::float_repr(n.as_f64().unwrap_or(f64::NAN)),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn related(notes: &BTreeMap<String, Note>, similar: &[SimilarPair], out: &mut Vec<Proposal>) {
    for pair in similar {
        let (a, b) = (nfc(&pair.a), nfc(&pair.b));
        if notes.contains_key(&a) && notes.contains_key(&b) {
            let mut p = Proposal::new(
                Kind::Related,
                vec![a, b],
                format!(
                    "Gần nghĩa (OHS, điểm {}), khác thư mục: tìm theo từ khoá dễ bỏ sót.",
                    py_str_json(&pair.score)
                ),
                "Hai note được nối với nhau qua `related` (hai chiều).".into(),
            );
            p.score = Some(pair.score.clone());
            out.push(p);
        }
    }
}

fn ideas(notes: &BTreeMap<String, Note>, out: &mut Vec<Proposal>) {
    let mut marked: Vec<(&str, Vec<&str>)> = notes
        .iter()
        .filter(|(_, n)| !n.is_system())
        .map(|(rel, n)| {
            let hs = HIGHLIGHT
                .captures_iter(&n.body)
                .filter_map(|c| c.get(1).map(|m| m.as_str()))
                .collect::<Vec<_>>();
            (rel.as_str(), hs)
        })
        .filter(|(_, hs)| !hs.is_empty())
        .collect();
    marked.sort_by(|x, y| y.1.len().cmp(&x.1.len()).then(x.0.cmp(y.0)));
    for (rel, hs) in marked {
        let mut lines = vec!["## Các đoạn bạn đã tô".to_owned(), String::new()];
        lines.extend(
            hs.iter()
                .take(IDEA_QUOTES)
                .map(|h| format!("> {}\n", strip(h))),
        );
        if hs.len() > IDEA_QUOTES {
            lines.push(format!("*(và {} đoạn nữa)*", hs.len() - IDEA_QUOTES));
            lines.push(String::new());
        }
        lines.push(format!("Đọc trong ngữ cảnh: {}", link(rel)));
        lines.push(String::new());
        let mut p = Proposal::new(
            Kind::Idea,
            vec![rel.to_owned()],
            format!(
                "Bạn đã tô {} đoạn trong note này: dấu hiệu có ý đáng rút ra.",
                hs.len()
            ),
            format!(
                "Tạo một note nháp trong `{IDEAS}/` có `from` trỏ về note này, các đoạn đã tô làm nguồn, trạm \
                 `to-clarify`. Phần ý chính để trống: bạn viết bằng lời mình."
            ),
        );
        p.lines = Some(lines);
        out.push(p);
    }
}

/// `{tên: {note: đoạn trích}}` cho tên đầy đủ ngoài ngữ cảnh địa chỉ, ở từ hai note trở lên; dãy số dài trong
/// đoạn trích bị che.
fn people(notes: &BTreeMap<String, Note>) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut found: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for (rel, n) in notes {
        if n.is_system() {
            continue;
        }
        let text = nfc(&n.body);
        for m in NAME.find_iter(&text) {
            let name = m.as_str();
            if found.get(name).is_some_and(|w| w.contains_key(rel)) {
                continue;
            }
            let before = &text[back(&text, m.start(), 20)..m.start()];
            let after = &text[m.end()..forward(&text, m.end(), 5)];
            if ADDRESS.is_match(before) || after == " City" {
                continue;
            }
            let around = &text[back(&text, m.start(), 60)..forward(&text, m.end(), 60)];
            let snippet = DIGITS.replace_all(&squash(around), "••••").into_owned();
            found
                .entry(name.to_owned())
                .or_default()
                .insert(rel.clone(), snippet.replace(name, &format!("**{name}**")));
        }
    }
    found.retain(|_, w| w.len() >= 2);
    let mut lists: HashMap<Vec<&String>, usize> = HashMap::new();
    for w in found.values() {
        *lists.entry(w.keys().collect()).or_default() += 1;
    }
    let rosters: HashSet<Vec<String>> = lists
        .into_iter()
        .filter(|(_, n)| *n >= ROSTER)
        .map(|(k, _)| k.into_iter().cloned().collect())
        .collect();
    found.retain(|_, w| !rosters.contains(&w.keys().cloned().collect::<Vec<_>>()));
    found
}

fn persons(notes: &BTreeMap<String, Note>, out: &mut Vec<Proposal>) {
    let mut people: Vec<_> = people(notes).into_iter().collect();
    people.sort_by(|x, y| y.1.len().cmp(&x.1.len()).then(x.0.cmp(&y.0)));
    for (name, place) in people {
        let mut lines = vec!["## Ở đâu".to_owned(), String::new()];
        lines.extend(place.iter().map(|(r, s)| format!("- {}: {s}", link(r))));
        lines.push(String::new());
        let mut p = Proposal::new(
            Kind::Person,
            place.keys().cloned().collect(),
            format!("Tên **{name}** xuất hiện trong {} note.", place.len()),
            format!(
                "Tạo trang `{PEOPLE}/{name}` (liệt kê mọi note nhắc tới người này) và ghi `who` vào {} note. Nếu \
                 đây không phải tên người (tên đường, nhân vật lịch sử), chọn bỏ.",
                place.len()
            ),
        );
        p.value = Some(name);
        p.lines = Some(lines);
        out.push(p);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Hàng chờ

/// Đề xuất đã có trong hàng chờ: khoá của mọi note đề xuất và số đề xuất đang chờ của mỗi kiểu.
#[derive(Debug, Default)]
pub struct Queue {
    pub keys: HashSet<String>,
    pub open: HashMap<String, usize>,
}

/// Đọc hàng chờ `VAULT/_Đề xuất/` (chưa có thì rỗng). Note không đọc được hay không phải đề xuất bị bỏ qua.
pub fn queue(vault: &Path) -> io::Result<Queue> {
    let mut q = Queue::default();
    let dir = vault.join(QUEUE);
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(q),
        Err(e) => return Err(e),
    };
    for e in entries.flatten() {
        if !e.path().is_file() {
            continue;
        }
        let Ok(text) = vault::read(&e.path()) else {
            continue;
        };
        let (Some(doc), _) = split(&text) else {
            continue;
        };
        let root = doc.root();
        if root.get("type").and_then(|v| v.as_str()) != Some("proposal") {
            continue;
        }
        let Some(kind) = root.get("kind").and_then(|v| v.as_str()) else {
            continue;
        };
        if root.get("status").and_then(|v| v.as_str()) == Some(OPEN) {
            *q.open.entry(kind.to_owned()).or_default() += 1;
        }
        let targets: Option<Vec<String>> = root.get("notes").and_then(|v| {
            v.list()?
                .map(|n| n.as_str().and_then(link_target).map(|t| format!("{t}.md")))
                .collect()
        });
        if let Some(t) = targets {
            q.keys.insert(nfc(&format!("{kind}|{}", t.join("|"))));
        }
    }
    Ok(q)
}

/// Đích của `[[đích|tên]]` ở đầu chuỗi.
fn link_target(s: &str) -> Option<&str> {
    let rest = s.strip_prefix("[[")?;
    let end = rest.find(['|', ']']).unwrap_or(rest.len());
    (end > 0).then(|| &rest[..end])
}

/// Kết quả của một lượt tính: tổng số đề xuất tính được và các đề xuất mới (chưa có trong hàng chờ, trong trần).
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub total: usize,
    pub new: Vec<Proposal>,
}

impl Plan {
    /// Số đề xuất mới theo kiểu, theo thứ tự xuất hiện.
    pub fn by_kind(&self) -> IndexMap<&'static str, usize> {
        let mut m = IndexMap::new();
        for p in &self.new {
            *m.entry(p.kind.as_str()).or_default() += 1;
        }
        m
    }

    /// Hợp đồng JSON của `pkm VAULT propose --json`: `written[i]` là note đề xuất của `proposals[i]` (rỗng khi
    /// không ghi).
    pub fn to_json(&self, written: &[String]) -> Value {
        json!({
            "total": self.total,
            "new": self.new.len(),
            "by_kind": self.by_kind().into_iter().map(|(k, n)| (k.to_owned(), json!(n))).collect::<Map<_, _>>(),
            "proposals": self.new.iter().map(Proposal::to_json).collect::<Vec<_>>(),
            "written": written,
        })
    }
}

/// Bỏ đề xuất đã có trong hàng chờ và giữ trần của `idea`, `person` (đếm cả đề xuất đang chờ).
pub fn select(all: Vec<Proposal>, queue: &Queue) -> Plan {
    let total = all.len();
    let mut open = queue.open.clone();
    let new = all
        .into_iter()
        .filter(|p| !queue.keys.contains(&p.key()))
        .filter(|p| match p.kind.cap() {
            Some(cap) => {
                let n = open.entry(p.kind.as_str().to_owned()).or_default();
                *n += 1;
                *n <= cap
            }
            None => true,
        })
        .collect();
    Plan { total, new }
}

/// Đề xuất mới của vault: [`proposals`] trừ hàng chờ, trong trần.
pub fn plan(vault: &Path, similar: &[SimilarPair]) -> io::Result<Plan> {
    Ok(select(proposals(vault, similar)?, &queue(vault)?))
}

/// Văn bản note đề xuất; `today` là ngày tạo (`YYYY-MM-DD`).
pub fn proposal_note(p: &Proposal, today: &str) -> String {
    let (by, ex) = p.kind.provenance();
    let s = |v: &str| Node::Str(v.to_owned());
    let mut fm = vec![
        ("type".to_owned(), s("proposal")),
        ("kind".to_owned(), s(p.kind.as_str())),
        ("status".to_owned(), s(OPEN)),
        ("created".to_owned(), s(today)),
        ("proposed_by".to_owned(), s(by)),
        ("extracted_by".to_owned(), Node::Str(ex)),
        (
            "notes".to_owned(),
            Node::Seq(p.notes.iter().map(|r| Node::Str(link(r))).collect()),
        ),
        ("reason".to_owned(), s(&p.reason)),
    ];
    let mut data = Vec::new();
    for (k, v) in [
        ("from_text", &p.from_text),
        ("to", &p.to),
        ("value", &p.value),
    ] {
        if let Some(v) = v {
            data.push((k.to_owned(), s(v)));
        }
    }
    if let Some(score) = &p.score {
        data.push(("score".to_owned(), Node::from_json(score)));
    }
    if !data.is_empty() {
        fm.push(("data".to_owned(), Node::Map(data)));
    }
    let mut body = vec![
        format!("# {}: {}", p.kind.title(), p.subject()),
        String::new(),
        p.reason.clone(),
        String::new(),
        format!("**Nhận** (đổi `status` thành `nhận`): {}", p.on_accept),
        "**Bỏ** (đổi `status` thành `bỏ`): không đổi gì. Muốn máy học thì ghi vài chữ vào thuộc tính `reject_reason` \
         (Lý do bỏ); sổ ghi lại để chỉnh luật."
            .to_owned(),
        "Sau khi chọn, chạy `propose.py --apply` (agent làm).".to_owned(),
        String::new(),
    ];
    match &p.lines {
        Some(lines) => body.extend(lines.iter().cloned()),
        None => {
            for r in p.notes.iter().take(EMBED_MAX) {
                body.extend([
                    format!("## {}", vault::title(r)),
                    String::new(),
                    format!("![[{}]]", r.strip_suffix(".md").unwrap_or(r)),
                    String::new(),
                ]);
            }
            let rest = p.notes.get(EMBED_MAX..).unwrap_or_default();
            if !rest.is_empty() {
                body.push(format!("## Còn {} note nữa", rest.len()));
                body.push(String::new());
                body.extend(rest.iter().map(|r| format!("- {}", link(r))));
                body.push(String::new());
            }
        }
    }
    format!("---\n{}---\n{}", dump::dump(&fm), body.join("\n"))
}

/// Tên file (chưa có `.md`) của note đề xuất: bỏ ký tự cấm trong tên file, tối đa 90 ký tự.
pub fn file_stem(p: &Proposal) -> String {
    let name = format!("{} - {}", p.kind.title(), p.subject());
    let name: String = UNSAFE_NAME
        .replace_all(&name, " ")
        .chars()
        .take(NAME_MAX)
        .collect();
    strip(&name).to_owned()
}

/// Ghi mỗi đề xuất thành một note mới trong `VAULT/_Đề xuất/` (trùng tên thì thêm ` 2`, ` 3`, ...); không bao giờ
/// ghi đè file đã có. Trả đường dẫn tương đối của các note đã ghi, cùng thứ tự với `props`.
pub fn write(vault: &Path, props: &[Proposal], today: &str) -> io::Result<Vec<String>> {
    let dir = vault.join(QUEUE);
    fs::create_dir_all(&dir)?;
    let mut written = Vec::with_capacity(props.len());
    for p in props {
        let stem = file_stem(p);
        let text = proposal_note(p, today);
        let mut n = 1;
        loop {
            let file = if n == 1 {
                format!("{stem}.md")
            } else {
                format!("{stem} {n}.md")
            };
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(dir.join(&file))
            {
                Ok(mut f) => {
                    io::Write::write_all(&mut f, text.as_bytes())?;
                    written.push(format!("{QUEUE}/{file}"));
                    break;
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => n += 1,
                Err(e) => return Err(e),
            }
        }
    }
    Ok(written)
}

#[cfg(test)]
mod tests;
