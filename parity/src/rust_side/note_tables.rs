//! Các bảng mà `pkm-core::vault` và `pkm-core::schema` quyết định: `nodes`, `props`, `raw_links`, `blocks`, `names`,
//! `links` và `schema_*`. Dòng được dựng thẳng từ kết quả đọc vault và nạp schema, đúng như `index.build` (build
//! đủ) ghi vào SQLite. Khi `pkm-core::index` có index thật, phần này đổi sang đọc bảng từ DB.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use pkm_core::schema::{self, Cell, Source};
use pkm_core::vault::{self, Origin, Resolver};
use serde_json::{Map, Value, json};

/// Dòng của mỗi bảng, mỗi dòng là cặp (cột, giá trị).
pub type Rows = Vec<Vec<(&'static str, Value)>>;

/// Đọc vault theo cách của dump mẫu (`run`: `packs`, `no_aliases`) và dựng các bảng.
pub fn build(vault_dir: &Path, run: &Value) -> Result<HashMap<&'static str, Rows>> {
    let packs: Vec<&str> = run
        .get("packs")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .split(',')
        .filter(|p| !p.is_empty())
        .collect();
    let aliases_on = !run
        .get("no_aliases")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let files = vault::list_notes(vault_dir, None)
        .with_context(|| format!("không liệt kê được note trong {}", vault_dir.display()))?;
    let mut t: HashMap<&'static str, Rows> = HashMap::new();
    let mut raw = Vec::new();
    let mut aliases = Vec::new();
    for (p, f) in &files {
        let text =
            vault::read(&f.path).with_context(|| format!("không đọc được {}", f.path.display()))?;
        let r = vault::parse(&text);
        let types = r.types();
        let disk = std::fs::metadata(&f.path)
            .and_then(|m| vault::mtime_ms(&m))
            .with_context(|| format!("không đọc được mtime của {}", f.path.display()))?;
        t.entry("nodes").or_default().push(vec![
            ("id", json!(p)),
            ("title", json!(vault::title(p))),
            (
                "type",
                json!(if types.len() == 1 { types[0] } else { None }),
            ),
            ("ntypes", json!(types.len())),
            ("folder", json!(vault::folder(p))),
            ("status", json!(r.first("status"))),
            ("valid_from", json!(r.first("valid_from"))),
            ("valid_to", json!(r.first("valid_to"))),
            ("proposed_by", json!(r.first("proposed_by"))),
            ("extracted_by", json!(r.first("extracted_by"))),
            ("asserted_at", json!(r.first("asserted_at"))),
            // Như dump Python: độ lệch so với mtime đọc từ đĩa lúc dump.
            ("mtime", json!(f.mtime - disk)),
            ("size", json!(f.size)),
            ("yaml_error", json!(r.error)),
        ]);
        for (k, v) in &r.props {
            t.entry("props").or_default().push(vec![
                ("node", json!(p)),
                ("key", json!(k)),
                ("value", json!(v)),
            ]);
        }
        for l in &r.links {
            t.entry("raw_links").or_default().push(vec![
                ("src", json!(p)),
                ("key", json!(l.key)),
                ("target", json!(l.target)),
                ("frag", json!(l.frag)),
                ("origin", json!(l.origin.as_str())),
                ("embed", json!(i32::from(l.embed))),
            ]);
            raw.push((p.clone(), l.clone()));
        }
        for b in &r.blocks {
            t.entry("blocks")
                .or_default()
                .push(vec![("node", json!(p)), ("block", json!(b))]);
        }
        let title = vault::title(p);
        let names = t.entry("names").or_default();
        names.push(vec![
            ("node", json!(p)),
            ("name", json!(title)),
            ("kind", json!("title")),
            ("norm", json!(vault::fold(title))),
        ]);
        for a in r.aliases() {
            names.push(vec![
                ("node", json!(p)),
                ("name", json!(a)),
                ("kind", json!("alias")),
                ("norm", json!(vault::fold(a))),
            ]);
            aliases.push((p.clone(), a.to_owned()));
        }
    }

    let resolver = Resolver::new(
        files.keys().map(String::as_str),
        aliases_on.then_some(aliases.as_slice()),
    );
    let links = t.entry("links").or_default();
    for (src, l) in &raw {
        let (dst, via) = resolver.resolve(src, &l.target, l.origin == Origin::BodyMd);
        links.push(vec![
            ("src", json!(src)),
            ("key", json!(l.key)),
            ("target", json!(l.target)),
            ("frag", json!(l.frag)),
            ("origin", json!(l.origin.as_str())),
            ("embed", json!(i32::from(l.embed))),
            ("dst", json!(dst)),
            ("via", json!(via.map(|v| v.as_str()))),
        ]);
    }

    let (_, tables) = schema::load(&Source::Embedded, &packs).context("không nạp được schema")?;
    for table in tables {
        let rows = table
            .rows
            .into_iter()
            .map(|r| {
                table
                    .columns
                    .iter()
                    .copied()
                    .zip(r.into_iter().map(|c| match c {
                        Cell::Null => Value::Null,
                        Cell::Int(i) => json!(i),
                        Cell::Text(s) => json!(s),
                    }))
                    .collect()
            })
            .collect();
        t.insert(table.name, rows);
    }
    for name in ["nodes", "props", "raw_links", "blocks", "names", "links"] {
        t.entry(name).or_default();
    }
    Ok(t)
}

/// Một bảng theo thứ tự cột của dump mẫu. `None` khi bảng không thuộc phần này. Cột của dump mẫu mà phần này
/// không có là lỗi (định dạng dump đã đổi).
pub fn rows(
    tables: &HashMap<&'static str, Rows>,
    name: &str,
    columns: &[String],
) -> Result<Option<Vec<Map<String, Value>>>> {
    let Some(rows) = tables.get(name) else {
        return Ok(None);
    };
    rows.iter()
        .map(|r| {
            columns
                .iter()
                .map(|c| {
                    let v = r
                        .iter()
                        .find(|(k, _)| k == c)
                        .map(|(_, v)| v.clone())
                        .with_context(|| format!("bảng {name}: Rust không có cột {c}"))?;
                    Ok((c.clone(), v))
                })
                .collect()
        })
        .collect::<Result<Vec<_>>>()
        .map(Some)
}
