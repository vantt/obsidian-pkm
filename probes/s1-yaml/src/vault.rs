//! Note listing and frontmatter split, mirroring the spec's `vault.list_notes`, `vault.read` and `FM_RE`.

use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

/// `^---[ \t]*\r?\n(.*?)\r?\n---[ \t]*(?:\r?\n|$)` with `re.S`. Python's `$` also matches before a final
/// `\n`, but the `\r?\n` alternative already covers that case, so plain `$` gives the same group.
static FM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)\A---[ \t]*\r?\n(.*?)\r?\n---[ \t]*(?:\r?\n|\z)")
        .expect("static regex compiles")
});

pub enum Front<'a> {
    /// No frontmatter block: `fm = {}`, no error.
    Absent,
    /// Starts with `---` but never closes.
    Unclosed,
    Yaml(&'a str),
}

pub fn split(text: &str) -> Front<'_> {
    if let Some(c) = FM_RE.captures(text) {
        return Front::Yaml(c.get(1).map_or("", |m| m.as_str()));
    }
    if text.starts_with("---\n") || text.starts_with("---\r\n") {
        Front::Unclosed
    } else {
        Front::Absent
    }
}

/// `open(p, encoding='utf-8', errors='replace', newline='')`: keep CRLF and a BOM, replace bad bytes.
pub fn read(path: &Path) -> std::io::Result<String> {
    Ok(String::from_utf8_lossy(&std::fs::read(path)?).into_owned())
}

/// `{relative NFC path: real path}` sorted by key. Skips dot directories, symlinked directories and
/// `.excalidraw.md`. A single `.md` file is accepted too (keyed by its file name).
pub fn list_notes(root: &Path) -> std::io::Result<Vec<(String, PathBuf)>> {
    if root.is_file() {
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        return Ok(vec![(name, root.to_path_buf())]);
    }
    let mut out = Vec::new();
    walk(root, root, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) -> std::io::Result<()> {
    let mut subs = Vec::new();
    for e in std::fs::read_dir(dir)? {
        let e = e?;
        let name = e.file_name().to_string_lossy().into_owned();
        let path = e.path();
        if path.is_dir() {
            if !name.starts_with('.') && !e.file_type()?.is_symlink() {
                subs.push(path);
            }
        } else if name.ends_with(".md") && !name.ends_with(".excalidraw.md") {
            let rel = path
                .strip_prefix(root)
                .expect("walked path is under root")
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");
            out.push((rel.nfc().collect(), path));
        }
    }
    for s in subs {
        walk(root, &s, out)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_follows_fm_re() {
        assert!(matches!(split("---\na: 1\n---\nbody"), Front::Yaml("a: 1")));
        assert!(matches!(
            split("---\r\na: 1\r\n---\r\n"),
            Front::Yaml("a: 1")
        ));
        assert!(matches!(split("---  \na: 1\n--- "), Front::Yaml("a: 1")));
        assert!(matches!(split("---\na: 1\n"), Front::Unclosed));
        assert!(matches!(split("\u{feff}---\na: 1\n---\n"), Front::Absent));
        assert!(matches!(split("---\n---\n"), Front::Unclosed));
    }
}
