//! Đọc vault: liệt kê note, tách frontmatter, link, block; giải link kiểu Obsidian (thêm alias nếu bật).
//!
//! Đặc tả: `scripts/kg/vault.py` ở sha ghim. Frontmatter đọc qua lớp tương thích PyYAML ([`yaml`], docs/DECISIONS.md
//! Q3) nên `props` và câu lỗi YAML giống bản Python. Đường dẫn note trong index là đường dẫn tương đối, phân cách
//! bằng `/`, dạng NFC; note được liệt kê theo đường dẫn đã sắp (docs/DECISIONS.md, "Thứ tự liệt kê file").

mod pystr;
mod resolve;
pub mod yaml;

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::UNIX_EPOCH;

use regex::Regex;
use unicode_normalization::UnicodeNormalization;

pub use resolve::{Resolver, Via};
use yaml::{Fail, Loader, Obj, Val};

/// `FM_RE` với `re.S`. `$` của Python còn khớp trước `\n` cuối, nhưng nhánh `\r?\n` đã phủ trường hợp đó.
static FM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)\A---[ \t]*\r?\n(.*?)\r?\n---[ \t]*(?:\r?\n|\z)").expect("regex tĩnh hợp lệ")
});
/// `[[đích#đoạn|tên hiện]]`, `![[nhúng]]`; nhóm 2 là đích, nhóm 3 là đoạn (`#heading`, `#^block`).
static WIKI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(!?)\[\[([^\]|\n#]*)(#[^\]|\n]*)?(?:\|[^\]\n]*)?\]\]").expect("regex tĩnh hợp lệ")
});
/// `MDLINK` của Python bỏ phần nhìn lùi `(?<!!)`; chỗ khớp đứng sau `!` được loại trong [`md_links`]. `\s` của
/// Python có thêm U+001C..U+001F.
static MDLINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[[^\]\n]*\]\(([^)\s\x1c-\x1f]+\.md)(#[^)]*)?\)").expect("regex tĩnh hợp lệ")
});
/// `^(```|~~~).*?^\1` (`re.S | re.M`) không dùng tham chiếu ngược: hai nhánh tách riêng.
static FENCE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?sm)^(?:```.*?^```|~~~.*?^~~~)").expect("regex tĩnh hợp lệ"));
/// `\r?`: note viết trên Windows (CRLF).
static BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)(?:^|[\s\x1c-\x1f])\^([A-Za-z0-9-]+)[ \t]*\r?$").expect("regex tĩnh hợp lệ")
});
static MARK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\p{Mn}").expect("regex tĩnh hợp lệ"));

/// `unicodedata.normalize('NFC', s)`.
pub fn nfc(s: &str) -> String {
    s.nfc().collect()
}

/// `str.casefold()`.
pub fn casefold(s: &str) -> String {
    caseless::default_case_fold_str(s)
}

/// Khoá so tên không phân biệt hoa thường, dấu, `đ` (luật R8, `find`).
pub fn fold(s: &str) -> String {
    let s: String = casefold(s).nfd().collect::<String>().replace('đ', "d");
    let s = MARK.replace_all(&s, "");
    pystr::split_whitespace(&s).collect::<Vec<_>>().join(" ")
}

/// Tên note: tên file bỏ `.md`.
pub fn title(path: &str) -> &str {
    strip_md(pystr::basename(path))
}

/// Thư mục của note (`""` ở gốc vault).
pub fn folder(path: &str) -> &str {
    pystr::dirname(path)
}

/// `p[:-3]` với đường dẫn note (luôn kết thúc bằng `.md`).
fn strip_md(p: &str) -> &str {
    p.strip_suffix(".md").unwrap_or(p)
}

/// Đọc file như `open(p, encoding='utf-8', errors='replace', newline='')`: giữ CRLF và BOM, byte UTF-8 hỏng thay
/// bằng U+FFFD.
pub fn read(path: &Path) -> io::Result<String> {
    Ok(String::from_utf8_lossy(&std::fs::read(path)?).into_owned())
}

/// Một file note khi liệt kê.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteFile {
    /// Đường dẫn thật trên đĩa.
    pub path: PathBuf,
    /// `int(st_mtime * 1000)`: mtime tính bằng ms, cắt phần lẻ như Python.
    pub mtime: i64,
    pub size: u64,
}

/// `{đường dẫn tương đối (NFC, `/`): file}` theo thứ tự đường dẫn. Bỏ thư mục bắt đầu bằng `.`, thư mục là
/// symlink và file `.excalidraw.md`. `skip(đường dẫn)` đúng thì bỏ note (luật của adapter).
///
/// Hai file khác nhau chỉ ở dạng NFC/NFD (chỉ có được trên Linux) có cùng khoá: Python giữ file `os.scandir` trả
/// sau (thứ tự của hệ thống file, không cố định); Rust giữ file đứng sau theo thứ tự tên đã sắp.
pub fn list_notes(
    vault: &Path,
    skip: Option<&dyn Fn(&str) -> bool>,
) -> io::Result<BTreeMap<String, NoteFile>> {
    let mut out = BTreeMap::new();
    walk(vault, "", &mut out)?;
    if let Some(skip) = skip {
        out.retain(|k, _| !skip(k));
    }
    Ok(out)
}

fn walk(dir: &Path, prefix: &str, out: &mut BTreeMap<String, NoteFile>) -> io::Result<()> {
    let mut entries = std::fs::read_dir(dir)?.collect::<io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.file_name());
    let mut subs = Vec::new();
    for e in entries {
        let name = e.file_name().to_string_lossy().into_owned();
        let path = e.path();
        // `DirEntry.is_dir()` của Python đi theo symlink; thư mục là symlink thì bỏ riêng.
        if path.is_dir() {
            if !name.starts_with('.') && !e.file_type()?.is_symlink() {
                subs.push((path, format!("{prefix}{name}/")));
            }
        } else if name.ends_with(".md") && !name.ends_with(".excalidraw.md") {
            let meta = std::fs::metadata(&path)?;
            out.insert(
                nfc(&format!("{prefix}{name}")),
                NoteFile {
                    mtime: mtime_ms(&meta)?,
                    size: meta.len(),
                    path,
                },
            );
        }
    }
    for (path, prefix) in subs {
        walk(&path, &prefix, out)?;
    }
    Ok(())
}

/// `int(os.stat(p).st_mtime * 1000)`: `st_mtime` là số thực `giây + nano * 1e-9`, nhân 1000 rồi cắt về 0.
pub fn mtime_ms(meta: &std::fs::Metadata) -> io::Result<i64> {
    let t = meta.modified()?;
    let (sec, nsec) = match t.duration_since(UNIX_EPOCH) {
        Ok(d) => (d.as_secs() as f64, f64::from(d.subsec_nanos())),
        Err(e) => {
            let d = e.duration();
            let carry = u64::from(d.subsec_nanos() > 0);
            (
                -((d.as_secs() + carry) as f64),
                f64::from((1_000_000_000 - d.subsec_nanos()) % 1_000_000_000),
            )
        }
    };
    Ok(((sec + nsec * 1e-9) * 1000.0) as i64)
}

/// Nguồn của một link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Frontmatter,
    Body,
    /// Link markdown `[chữ](đường/dẫn.md)` trong thân note.
    BodyMd,
}

impl Origin {
    pub fn as_str(self) -> &'static str {
        match self {
            Origin::Frontmatter => "frontmatter",
            Origin::Body => "body",
            Origin::BodyMd => "body-md",
        }
    }
}

/// Một link chưa giải (bảng `raw_links`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// Khoá frontmatter, hoặc `link` với link trong thân note.
    pub key: String,
    pub target: String,
    /// `#heading`, `#^block` hoặc rỗng.
    pub frag: String,
    pub origin: Origin,
    /// `![[...]]`.
    pub embed: bool,
}

/// Kết quả đọc một note.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parsed {
    /// Lỗi frontmatter (`nodes.yaml_error`).
    pub error: Option<String>,
    /// `(khoá, giá trị)`; dict cấp 1 tách thành `cha.con`, list làm phẳng, `null` là `None`.
    pub props: Vec<(String, Option<String>)>,
    pub links: Vec<Link>,
    /// Block ID (`^id`), đã sắp, không trùng.
    pub blocks: Vec<String>,
}

impl Parsed {
    /// Giá trị khác `None` đầu tiên của một khoá (`index._first`).
    pub fn first(&self, key: &str) -> Option<&str> {
        self.props
            .iter()
            .find(|(k, v)| k == key && v.is_some())
            .and_then(|(_, v)| v.as_deref())
    }

    /// Mọi giá trị của khoá `type` (kể cả `None`).
    pub fn types(&self) -> Vec<Option<&str>> {
        self.props
            .iter()
            .filter(|(k, _)| k == "type")
            .map(|(_, v)| v.as_deref())
            .collect()
    }

    /// Alias khác rỗng.
    pub fn aliases(&self) -> impl Iterator<Item = &str> {
        self.props
            .iter()
            .filter(|(k, _)| k == "aliases")
            .filter_map(|(_, v)| v.as_deref().filter(|a| !a.is_empty()))
    }
}

/// Đọc một note (`vault.parse`).
pub fn parse(text: &str) -> Parsed {
    let mut out = Parsed::default();
    let mut body = text;
    if let Some(m) = FM_RE.captures(text) {
        body = &text[m.get(0).map_or(0, |g| g.end())..];
        match read_frontmatter(m.get(1).map_or("", |g| g.as_str())) {
            Ok((props, links)) => {
                out.props = props;
                out.links = links;
            }
            Err(e) => out.error = Some(e),
        }
    } else if text.starts_with("---\n") || text.starts_with("---\r\n") {
        out.error = Some("frontmatter không đóng".into());
    }
    let clean = FENCE.replace_all(body, "");
    for c in WIKI.captures_iter(&clean) {
        out.links.push(wiki_link(&c, "link", Origin::Body));
    }
    out.links.extend(md_links(&clean));
    let mut blocks: Vec<String> = BLOCK
        .captures_iter(body)
        .filter_map(|c| c.get(1).map(|g| g.as_str().to_owned()))
        .collect();
    blocks.sort();
    blocks.dedup();
    out.blocks = blocks;
    out
}

type Props = Vec<(String, Option<String>)>;

/// Frontmatter -> (`props`, link trong frontmatter), hoặc câu lỗi lưu vào `nodes.yaml_error`. Frontmatter rỗng hay
/// "sai" theo Python (`0`, `[]`, `""`: `yaml.load(...) or {}`) là dict rỗng, không lỗi.
fn read_frontmatter(text: &str) -> Result<(Props, Vec<Link>), String> {
    let doc = yaml::load(text, Loader::Frontmatter).map_err(|e| yaml_error(&e))?;
    let (mut props, mut links) = (Vec::new(), Vec::new());
    match doc.as_ref().map(yaml::Doc::root).filter(Val::truthy) {
        None => {}
        Some(root) if root.is_dict() => {
            frontmatter(root, &mut props, &mut links).map_err(|e| yaml_error(&e))?
        }
        Some(_) => return Err("frontmatter không phải bảng khoá".into()),
    }
    Ok((props, links))
}

/// `'YAML lỗi: ' + str(e).split('\n')[0]`. Với ngoại lệ bản Python không bắt (nó dừng cả lần index), Rust ghi
/// tên kiểu ngoại lệ (spec/PORTING.md, "Lệch có chủ ý").
fn yaml_error(e: &Fail) -> String {
    let msg = e.to_string();
    format!("YAML lỗi: {}", msg.split('\n').next().unwrap_or(""))
}

fn wiki_link(c: &regex::Captures<'_>, key: &str, origin: Origin) -> Link {
    let g = |i| c.get(i).map_or("", |m| m.as_str());
    Link {
        key: key.to_owned(),
        target: pystr::strip(g(2)).to_owned(),
        frag: g(3).to_owned(),
        origin,
        embed: origin == Origin::Body && !g(1).is_empty(),
    }
}

/// `MDLINK.finditer` với `(?<!!)`: chỗ khớp đứng ngay sau `!` bị bỏ và tìm lại từ ký tự kế tiếp, như regex Python
/// thử từng vị trí.
fn md_links(text: &str) -> Vec<Link> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(c) = MDLINK.captures_at(text, at) {
        let m = c.get(0).map_or(at..at, |g| g.range());
        if text[..m.start].ends_with('!') {
            at = m.start + text[m.start..].chars().next().map_or(1, char::len_utf8);
            continue;
        }
        at = m.end;
        let target = c.get(1).map_or("", |g| g.as_str());
        if !target.contains("://") {
            out.push(Link {
                key: "link".into(),
                target: pystr::unquote(target),
                frag: c.get(2).map_or("", |g| g.as_str()).to_owned(),
                origin: Origin::BodyMd,
                embed: false,
            });
        }
    }
    out
}

/// Vòng `props` của `vault.parse` trên frontmatter là dict.
fn frontmatter(root: Val<'_>, props: &mut Props, links: &mut Vec<Link>) -> Result<(), Fail> {
    for (k, v) in root.items().into_iter().flatten() {
        let k = k.py_str()?;
        if let Some(inner) = v.items() {
            for (k2, v2) in inner {
                let key = format!("{k}.{}", k2.py_str()?);
                let mut vals = Vec::new();
                flat(v2, &mut vals, &mut Vec::new())?;
                props.extend(vals.into_iter().map(|x| (key.clone(), x)));
            }
            continue;
        }
        let mut vals = Vec::new();
        flat(v, &mut vals, &mut Vec::new())?;
        for x in vals {
            for c in WIKI.captures_iter(x.as_deref().unwrap_or("")) {
                links.push(wiki_link(&c, &k, Origin::Frontmatter));
            }
            props.push((k.clone(), x));
        }
    }
    Ok(())
}

/// `vault.flat`: mỗi lá của list một giá trị; `None` giữ `None`. List chứa chính nó (alias vòng) làm Python
/// đệ quy tới `RecursionError`.
fn flat(v: Val<'_>, out: &mut Vec<Option<String>>, open: &mut Vec<*const Obj>) -> Result<(), Fail> {
    match v.obj() {
        Obj::None => out.push(None),
        Obj::Bool(b) => out.push(Some(if *b { "true" } else { "false" }.into())),
        Obj::Date(d) => out.push(Some(yaml::repr::date_iso(d))),
        Obj::DateTime(dt) => out.push(Some(yaml::repr::datetime_iso(dt, 'T'))),
        Obj::List(_) => {
            let me: *const Obj = v.obj();
            if open.contains(&me) || open.len() >= yaml::MAX_DEPTH {
                return Err(Fail::Crash("RecursionError"));
            }
            open.push(me);
            for x in v.list().into_iter().flatten() {
                flat(x, out, open)?;
            }
            open.pop();
        }
        _ => out.push(Some(v.py_str()?)),
    }
    Ok(())
}

#[cfg(test)]
mod tests;
