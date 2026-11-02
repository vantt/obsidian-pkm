//! `instructions` của `initialize`, ghép như `Server.instructions` của `scripts/kg/mcp_server.py`: mô tả vault, mô tả
//! từng loại note của schema (cả gói), số note và quan hệ đang dùng, lớp cạnh, đoạn của adapter, khối cố định.
//!
//! Số liệu đọc thẳng từ index (như `Graph` của Python: `schema_edge`, `meta.hidden`), nên không cần tầng tool.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use indexmap::IndexMap;
use pkm_core::schema::{self, Source};
use rusqlite::Connection;
use serde_json::Value;

use super::pyjson;

/// Phần không đọc từ index.
pub struct Inputs<'a> {
    /// Thư mục vault, tuyệt đối.
    pub vault: &'a Path,
    /// Gói schema đang bật (cờ `--packs`, hoặc `meta.packs`).
    pub packs: &'a [String],
    /// `--about`.
    pub about: Option<&'a str>,
    /// `--today`, hoặc ngày máy.
    pub today: &'a str,
    /// Tên vault trong Obsidian; `None` là không có khối link.
    pub vault_name: Option<&'a str>,
    /// Câu của adapter.
    pub adapter: Option<&'a str>,
}

/// Khi chưa có index để đọc (`index is still being built`), kèm lỗi build nếu có.
pub fn pending(build_error: Option<&str>) -> String {
    format!(
        "Typed knowledge graph (kg) of an Obsidian vault; the index is still being built. {}",
        build_error.unwrap_or_default()
    )
}

/// `os.path.basename(path.rstrip("/"))`.
pub fn basename(p: &Path) -> String {
    let s = p.to_string_lossy();
    let s = s.trim_end_matches('/');
    s.rsplit('/').next().unwrap_or_default().to_owned()
}

/// `urllib.parse.quote(s, safe="")`: giữ chữ cái, chữ số ASCII và `_.-~`, mọi byte UTF-8 khác thành `%XX`.
pub fn quote(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"_.-~".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Mô tả type từ schema nhúng: gốc rồi các gói đang bật; và mô tả của các gói.
fn type_descriptions(packs: &[String]) -> (HashMap<String, String>, Vec<String>) {
    let mut desc = HashMap::new();
    let mut pack_desc = Vec::new();
    let Ok(doc) = schema::read_schema(&Source::Embedded) else {
        return (desc, pack_desc);
    };
    let text = |v: pkm_core::vault::yaml::Val<'_>| -> String {
        v.as_str()
            .map(str::to_owned)
            .or_else(|| v.py_str().ok())
            .unwrap_or_default()
    };
    let describe = |types: Option<pkm_core::vault::yaml::Val<'_>>,
                    desc: &mut HashMap<String, String>| {
        for (k, v) in types.and_then(|t| t.items()).into_iter().flatten() {
            let Some(k) = k.as_str() else { continue };
            let d = v.get("description").map(text).unwrap_or_default();
            desc.insert(k.to_owned(), d);
        }
    };
    let root = doc.root();
    describe(root.get("node_types"), &mut desc);
    for p in packs {
        let Some(pk) = root.get("packs").and_then(|ps| ps.get(p)) else {
            continue;
        };
        describe(pk.get("node_types"), &mut desc);
        if let Some(d) = pk.get("description").filter(|d| d.truthy()) {
            pack_desc.push(text(d));
        }
    }
    (desc, pack_desc)
}

/// Một quan hệ của `schema_edge`.
struct Rel {
    inverse: Option<String>,
    symmetric: bool,
    polarity: Option<String>,
}

/// Trang hệ thống ghi trong `meta.hidden` (danh sách JSON), bỏ phần tử rỗng; hỏng thì rỗng (như `Graph`).
pub fn hidden(db: &Connection) -> Vec<String> {
    let raw: Option<String> = db
        .query_row("SELECT value FROM meta WHERE key = 'hidden'", [], |r| {
            r.get(0)
        })
        .ok()
        .flatten();
    let Some(Ok(Value::Array(items))) = raw.map(|s| serde_json::from_str::<Value>(&s)) else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter_map(|v| match v {
            Value::String(s) if !s.is_empty() => Some(s),
            _ => None,
        })
        .collect()
}

/// Ghép `instructions` từ index `db`.
pub fn compose(inp: &Inputs<'_>, db: &Connection) -> rusqlite::Result<String> {
    let (desc, pack_desc) = type_descriptions(inp.packs);
    let counts: Vec<(String, i64)> = db
        .prepare(
            "SELECT type, count(*) FROM nodes WHERE type IS NOT NULL GROUP BY 1 ORDER BY 2 DESC",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let n: i64 = db.query_row("SELECT count(*) FROM nodes", [], |r| r.get(0))?;
    let used: BTreeSet<String> = db
        .prepare("SELECT DISTINCT rel FROM edges")?
        .query_map([], |r| r.get::<_, Option<String>>(0))?
        .filter_map(|r| r.transpose())
        .collect::<rusqlite::Result<_>>()?;
    let rels: IndexMap<String, Rel> = db
        .prepare("SELECT key, inverse, symmetric, polarity FROM schema_edge")?
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                Rel {
                    inverse: r.get(1)?,
                    symmetric: r.get::<_, Option<i64>>(2)?.unwrap_or(0) != 0,
                    polarity: r.get(3)?,
                },
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let layers: serde_json::Map<String, Value> = db
        .prepare("SELECT status, count(*) FROM edges GROUP BY 1")?
        .query_map([], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?
                    .unwrap_or_else(|| "null".into()),
                Value::from(r.get::<_, i64>(1)?),
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let hidden = hidden(db);

    let nonempty = |s: &Option<String>| s.as_deref().filter(|x| !x.is_empty()).map(str::to_owned);
    let rel_text: Vec<String> = rels
        .iter()
        .filter(|(r, _)| used.contains(*r))
        .map(|(r, x)| {
            let mut bits = Vec::new();
            if let (Some(inv), false) = (nonempty(&x.inverse), x.symmetric) {
                bits.push(format!("inverse {inv}"));
            }
            if x.symmetric {
                bits.push("symmetric".to_owned());
            }
            if let Some(p) = nonempty(&x.polarity) {
                bits.push(format!("polarity {p}"));
            }
            format!("{r} ({})", bits.join(", "))
        })
        .collect();
    let extra: Vec<&str> = used
        .iter()
        .filter(|r| !rels.contains_key(*r))
        .map(String::as_str)
        .collect();
    let types: Vec<String> = counts
        .iter()
        .map(|(t, c)| match desc.get(t).filter(|d| !d.is_empty()) {
            Some(d) => format!("{t} ({c}, {d})"),
            None => format!("{t} ({c})"),
        })
        .collect();
    let about = match inp.about.filter(|a| !a.is_empty()) {
        Some(a) => a.to_owned(),
        None if !pack_desc.is_empty() => pack_desc.join(" "),
        None => "The user's personal notes.".to_owned(),
    };
    let name = basename(inp.vault);
    let today = inp.today;

    let mut lines = vec![
        format!(
            "Typed knowledge graph (kg) of the user's Obsidian vault \"{name}\": {n} notes. What the vault describes: {about} Today is {today}; use it for \"now\", \"currently\", \"đang\", \"hiện tại\"."
        ),
        "Questions the user asks are about the content of this vault. Words like tools / công cụ, projects / dự án, people / người, decisions, contracts refer to NOTES of that type in the vault, never to your own tools, MCP servers or environment: answer them with select {type: ...}, find or the graph tools.".to_owned(),
        format!("Note types (count, meaning): {}.", types.join("; ")),
        format!(
            "Relations in this vault: {}{}. Walking an edge backwards gives the inverse name.",
            rel_text.join("; "),
            if extra.is_empty() {
                String::new()
            } else {
                format!("; untyped: {}", extra.join(", "))
            }
        ),
        "A claim note (type claim) is itself an edge: relation from cause to effect (or subject to object), with confidence, sources (note#^block), status and validity window; neighbors/paths show it in `claim`. Causal rungs: correlates_with (1) < increases/decreases/prevents (2) < conditions (3); do not present a correlation as a cause.".to_owned(),
        format!(
            "Edge layers: accepted = written or approved by the user; proposed = machine suggestion not yet reviewed (usable, but tell the user it is a suggestion and who proposed it); superseded = replaced, kept for history (use as_of for the state on a day); rejected = never returned unless asked. Counts: {}. Tools return all but rejected by default; accepted_only=true drops suggestions.",
            pyjson::dumps(&Value::Object(layers), pyjson::DEFAULT)
        ),
        "`status` = review state (proposed means waiting for review, by anyone). `proposed_by` = who proposed it (llm, model, rule, human). \"Created/proposed by the AI or LLM\" means proposed_by llm, of any type and any status.".to_owned(),
        "Note ids are vault paths (e.g. \"Folder/Title.md\"); pass them exactly as returned. Start with find when the user gives a name. Prefer one parameterised call (reach, paths, loops, valid, select, neighbors with dir/rels/as_of) over walking neighbors step by step. Long results come summarised (count, by_type, first items, handle): answer from them; call more only if the full list is needed.".to_owned(),
    ];
    if !hidden.is_empty() {
        lines.push(format!(
            "System pages ({}) link to everything, so tools skip them by default; system=true includes them.",
            hidden.join(", ")
        ));
    }
    if let Some(a) = inp.adapter.filter(|a| !a.is_empty()) {
        lines.push(a.to_owned());
    }
    if let Some(v) = inp.vault_name.filter(|v| !v.is_empty()) {
        lines.push(format!(
            "Links: when you name notes in an answer, write each as a markdown link that opens it in Obsidian: [title](obsidian://open?vault={}&file=PATH), PATH = the note id without \".md\", percent-encoding only space %20, ( %28, ) %29, # %23, & %26, ? %3F, % %25 (keep / and letters with diacritics as they are). `note` returns the exact link as `url`.",
            quote(v)
        ));
    }
    lines.push("Search vs graph: if the OHS server (obsidian-hybrid-search) is also connected, use its `search` for words, meaning, full text, similar notes and reading note bodies; use kg for typed relations, multi-hop, paths, loops, time windows and provenance. To combine them, call OHS search, then pass its results to kg `related` as hits (or start neighbors/reach from a hit path).".to_owned());
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_like_urllib_with_no_safe_characters() {
        assert_eq!(quote("kg-solo"), "kg-solo");
        assert_eq!(
            quote("Vault của tôi/a&b"),
            "Vault%20c%E1%BB%A7a%20t%C3%B4i%2Fa%26b"
        );
        assert_eq!(quote("a_b.c~d"), "a_b.c~d");
    }

    #[test]
    fn basename_ignores_trailing_slashes() {
        assert_eq!(basename(Path::new("/x/kg-solo/")), "kg-solo");
        assert_eq!(basename(Path::new("/x/kg-solo")), "kg-solo");
        assert_eq!(basename(Path::new("/")), "");
    }
}
