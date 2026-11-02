//! Phần chung của hai server thử: phiên bản giao thức, `instructions`, khai báo tool `find` (chép nguyên văn
//! từ `scripts/kg/mcp_server.py` ở sha ghim) và cách chạy tool trên thư mục vault.
//!
//! Hai binary chỉ khác tầng giao thức: `s2-hand` tự đọc JSON-RPC theo dòng, `s2-rmcp` đi qua crate `rmcp`.
//! Mọi chữ agent nhìn thấy (instructions, mô tả, schema, kết quả tool) đều sinh ở đây nên hai bản so được.

use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// Phiên bản giao thức bản Python nhận, theo thứ tự ưu tiên; phần tử đầu là giá trị trả khi client xin bản lạ.
pub const PROTOCOLS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
pub const SERVER_NAME: &str = "kg";
pub const SERVER_VERSION: &str = "0.1.0";
const DEFAULT_LIMIT: usize = 20;

/// Giống Python: trả lại đúng bản client xin nếu nằm trong `PROTOCOLS`, không thì bản mới nhất của server.
pub fn negotiate(requested: Option<&str>) -> &'static str {
    PROTOCOLS
        .iter()
        .copied()
        .find(|p| Some(*p) == requested)
        .unwrap_or(PROTOCOLS[0])
}

/// `instructions` của server thử. Cố ý có tiếng Việt, ngoặc kép, backtick và xuống dòng để kiểm câu chữ đi qua
/// nguyên vẹn tới client.
pub fn instructions(vault: &Path) -> String {
    let name = vault
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    [
        format!(
            "Typed knowledge graph (kg) of the user's Obsidian vault \"{name}\": probe server with one tool (`find`)."
        ),
        "Questions the user asks are about the content of this vault. Words like tools / công cụ, projects / dự án, people / người refer to NOTES of that type in the vault, never to your own tools: answer them with `find`.".to_string(),
        "Note ids are vault paths (e.g. \"Khách hàng/Công ty ABC.md\"); pass them exactly as returned.".to_string(),
    ]
    .join("\n")
}

/// Mục `find` trong `tools/list`, cùng khoá, cùng thứ tự, cùng câu chữ với Python.
pub fn tool_defs() -> Value {
    let limit = json!({"type": "integer", "description": "Items shown before the result is summarised (default 20). Long results return count, by_type, the first items and a handle for `more`."});
    json!([{
        "name": "find",
        "description": "Look up notes by name the user typed: every word of `name` must match a word of a note title or alias (case and Vietnamese diacritics ignored); an exact vault path also works. Returns id, title, type, aliases. Use it first whenever the user names something. Several hits that are different things (two people with similar names): ask the user which one, do not pick. Hits that are the same thing (duplicate notes): say so.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "type": {"type": "string", "description": "Optional note type filter (see instructions)."},
                "limit": limit,
            },
            "required": ["name"],
        },
    }])
}

/// Chạy một tool, trả `(text, is_error)` như `tools/call` của Python: JSON gọn, bỏ trường rỗng, lỗi là object có
/// `error` và nhiều nhất hai khoá.
pub fn call_tool(vault: &Path, name: &str, args: &Map<String, Value>) -> (String, bool) {
    let res = match name {
        "find" => find(vault, args),
        _ => json!({"error": format!("unknown tool {name}")}),
    };
    let is_error = res
        .as_object()
        .is_some_and(|o| o.contains_key("error") && o.len() <= 2);
    (compact(res).to_string(), is_error)
}

/// Khoá so tên như `vault.fold` của Python: không phân biệt hoa thường, dấu, `đ`.
pub fn fold(s: &str) -> String {
    let lowered = s.to_lowercase();
    let stripped: String = lowered
        .nfd()
        .filter(|c| !is_combining_mark(*c))
        .map(|c| if c == 'đ' { 'd' } else { c })
        .collect();
    stripped.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn find(vault: &Path, args: &Map<String, Value>) -> Value {
    let Some(raw) = args.get("name").and_then(Value::as_str) else {
        return json!({"error": "find needs name (string)"});
    };
    let type_filter = args.get("type").and_then(Value::as_str);
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .map_or(DEFAULT_LIMIT, |n| usize::try_from(n).unwrap_or(usize::MAX));

    let q: String = raw.trim().nfc().collect();
    let q = q.strip_prefix("[[").unwrap_or(&q);
    let q = q.strip_suffix("]]").unwrap_or(q);
    let q = q.split('|').next().unwrap_or_default();
    let want: Vec<String> = fold(q).split(' ').map(str::to_owned).collect();
    if want.iter().all(String::is_empty) {
        return json!({"error": "find needs a non-empty name"});
    }

    let mut notes = Vec::new();
    if let Err(e) = collect_notes(vault, vault, &mut notes) {
        return json!({"error": format!("cannot read vault: {e}")});
    }
    notes.sort();

    let exact = if q.ends_with(".md") {
        q.to_string()
    } else {
        format!("{q}.md")
    };
    let ids: Vec<&String> = if notes.contains(&exact) {
        notes.iter().filter(|id| **id == exact).collect()
    } else {
        notes
            .iter()
            .filter(|id| {
                let title = fold(title_of(id));
                let words: Vec<&str> = title.split(' ').collect();
                want.iter().all(|w| words.contains(&w.as_str()))
            })
            .collect()
    };

    let mut items = Vec::new();
    for id in ids {
        let note_type = note_type(&vault.join(id));
        if type_filter.is_some_and(|t| note_type.as_deref() != Some(t)) {
            continue;
        }
        items.push(json!({"id": id, "title": title_of(id), "type": note_type}));
    }
    let count = items.len();
    items.truncate(limit);
    json!({"count": count, "items": items})
}

/// Mọi file `.md` dưới vault, id là đường dẫn tương đối dùng `/`, chuẩn NFC. Bỏ thư mục ẩn (`.obsidian`, `.kg`).
fn collect_notes(root: &Path, dir: &Path, out: &mut Vec<String>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let path: PathBuf = entry.path();
        if entry.file_type()?.is_dir() {
            collect_notes(root, &path, out)?;
        } else if path.extension().is_some_and(|e| e == "md") {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let id: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().nfc().collect())
                .collect();
            out.push(id.join("/"));
        }
    }
    Ok(())
}

fn title_of(id: &str) -> &str {
    let base = id.rsplit('/').next().unwrap_or(id);
    base.strip_suffix(".md").unwrap_or(base)
}

/// `type:` ở frontmatter (dòng phẳng `type: x`). Bài thử không cần đọc YAML đủ.
fn note_type(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let rest = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))?;
    for line in rest.lines() {
        if line.trim_end() == "---" {
            break;
        }
        if let Some(v) = line.strip_prefix("type:") {
            let v = v.trim().trim_matches(|c| c == '"' || c == '\'');
            return (!v.is_empty()).then(|| v.to_string());
        }
    }
    None
}

/// Như `compact` của Python: bỏ `null` và chuỗi rỗng trong object, giữ danh sách rỗng.
fn compact(v: Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.into_iter()
                .filter(|(_, x)| !x.is_null() && x.as_str() != Some(""))
                .map(|(k, x)| (k, compact(x)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.into_iter().map(compact).collect()),
        x => x,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiate_echoes_known_versions_and_falls_back_to_newest() {
        assert_eq!(negotiate(Some("2024-11-05")), "2024-11-05");
        assert_eq!(negotiate(Some("2025-03-26")), "2025-03-26");
        assert_eq!(negotiate(Some("2025-11-25")), "2025-06-18");
        assert_eq!(negotiate(None), "2025-06-18");
    }

    #[test]
    fn fold_ignores_case_diacritics_and_d_stroke() {
        assert_eq!(fold("  Công ty   ĐẠI Phát "), "cong ty dai phat");
        assert_eq!(fold("Nguyễn Văn A"), "nguyen van a");
    }

    #[test]
    fn compact_drops_null_and_empty_strings_but_keeps_empty_lists() {
        let v = compact(json!({"a": null, "b": "", "c": [], "d": {"e": null, "f": 1}}));
        assert_eq!(v.to_string(), r#"{"c":[],"d":{"f":1}}"#);
    }

    #[test]
    fn unknown_tool_is_an_error() {
        let (text, err) = call_tool(Path::new("."), "nope", &Map::new());
        assert!(err);
        assert_eq!(text, r#"{"error":"unknown tool nope"}"#);
    }
}
