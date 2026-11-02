//! Ngoại lệ có tên (`parity/exceptions.toml`): chỗ Rust cố ý làm khác Python. Mỗi ngoại lệ phải
//! có mục cùng tên trong phần "Lệch có chủ ý" của `spec/PORTING.md`; thiếu thì bộ so khớp từ chối chạy.

use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::dump::LAYERS;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exception {
    /// Tên ngoại lệ, cũng là tên mục trong "Lệch có chủ ý" của `spec/PORTING.md`.
    pub name: String,
    /// `index`, `answers`, `tools`, `mcp` hoặc `cli`.
    pub layer: String,
    /// Bảng, op, tool, file MCP hay id lệnh CLI; `*` là mọi mục của tầng.
    pub item: String,
    /// Chỉ tha khi mọi chỗ lệch của một đơn vị nằm dưới một trong các trường này (JSON pointer,
    /// `*` khớp một đoạn bất kỳ; dòng bảng: `/tên cột`; văn bản: `/line`). Rỗng là mọi trường.
    #[serde(default)]
    pub fields: Vec<String>,
    /// Lý do ngắn; chi tiết ở `spec/PORTING.md`.
    pub reason: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    exception: Vec<Exception>,
}

const PORTING_SECTION: &str = "## Lệch có chủ ý";

/// Đọc và kiểm ngoại lệ: tầng hợp lệ, tên không trùng, có mục cùng tên ở `spec/PORTING.md`.
pub fn load(path: &Path, porting: &Path) -> Result<Vec<Exception>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("không đọc được {}", path.display()))?;
    let file: File =
        toml::from_str(&text).with_context(|| format!("{} sai định dạng", path.display()))?;
    if file.exception.is_empty() {
        return Ok(Vec::new());
    }
    let porting_text = std::fs::read_to_string(porting)
        .with_context(|| format!("không đọc được {}", porting.display()))?;
    let section = porting_text
        .split_once(PORTING_SECTION)
        .map(|(_, rest)| rest)
        .with_context(|| format!("{} không có mục \"Lệch có chủ ý\"", porting.display()))?;
    let mut names = std::collections::BTreeSet::new();
    for e in &file.exception {
        if !LAYERS.contains(&e.layer.as_str()) || e.layer == "perf" {
            bail!(
                "ngoại lệ {}: tầng {:?} không hợp lệ (index, answers, tools, mcp, cli)",
                e.name,
                e.layer
            );
        }
        if e.name.trim().is_empty() || e.reason.trim().is_empty() {
            bail!("ngoại lệ thiếu name hoặc reason trong {}", path.display());
        }
        if !names.insert(e.name.as_str()) {
            bail!("ngoại lệ {} ghi hai lần", e.name);
        }
        if !section.contains(&e.name) {
            bail!(
                "ngoại lệ {} không có mục cùng tên trong phần \"Lệch có chủ ý\" của {}",
                e.name,
                porting.display()
            );
        }
    }
    Ok(file.exception)
}

impl Exception {
    /// Ngoại lệ này tha một đơn vị lệch có các chỗ lệch `fields` (JSON pointer) không.
    pub fn covers(&self, layer: &str, item: &str, fields: &[String]) -> bool {
        if self.layer != layer || (self.item != "*" && self.item != item) {
            return false;
        }
        self.fields.is_empty()
            || fields
                .iter()
                .all(|f| self.fields.iter().any(|p| pointer_matches(p, f)))
    }
}

/// `pattern` khớp `path` hoặc một tổ tiên của nó; `*` khớp một đoạn.
fn pointer_matches(pattern: &str, path: &str) -> bool {
    let pat: Vec<&str> = pattern.split('/').collect();
    let seg: Vec<&str> = path.split('/').collect();
    pat.len() <= seg.len() && pat.iter().zip(&seg).all(|(p, s)| *p == "*" || p == s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exc(item: &str, fields: &[&str]) -> Exception {
        Exception {
            name: "x".into(),
            layer: "tools".into(),
            item: item.into(),
            fields: fields.iter().map(|s| (*s).to_owned()).collect(),
            reason: "r".into(),
        }
    }

    #[test]
    fn field_patterns() {
        let e = exc("neighbors", &["/items/*/claim"]);
        assert!(e.covers("tools", "neighbors", &["/items/3/claim".into()]));
        assert!(e.covers("tools", "neighbors", &["/items/3/claim/x".into()]));
        assert!(!e.covers(
            "tools",
            "neighbors",
            &["/items/3/claim".into(), "/count".into()]
        ));
        assert!(!e.covers("tools", "reach", &["/items/3/claim".into()]));
        assert!(!e.covers("index", "neighbors", &["/items/3/claim".into()]));
        assert!(exc("*", &[]).covers("tools", "find", &["/a".into()]));
    }

    #[test]
    fn exception_needs_porting_entry() {
        let dir = tempfile::tempdir().unwrap();
        let exc = dir.path().join("exceptions.toml");
        let porting = dir.path().join("PORTING.md");
        std::fs::write(
            &exc,
            "[[exception]]\nname = \"lech-a\"\nlayer = \"tools\"\nitem = \"find\"\nreason = \"r\"\n",
        )
        .unwrap();
        std::fs::write(&porting, "# x\n\n## Lệch có chủ ý\n\nChưa có mục nào.\n").unwrap();
        assert!(load(&exc, &porting).is_err());
        std::fs::write(&porting, "# x\n\n## Lệch có chủ ý\n\n### lech-a\n").unwrap();
        assert_eq!(load(&exc, &porting).unwrap().len(), 1);
    }
}
