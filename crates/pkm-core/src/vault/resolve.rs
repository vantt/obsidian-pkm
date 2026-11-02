//! Giải link như Obsidian (`vault.Resolver`): đường dẫn đủ, đường dẫn tương đối với note nguồn, rồi tên file không
//! phân biệt hoa thường (nhiều note trùng tên: đúng hoa thường trước, cùng thư mục, rồi đường ngắn nhất). Bật alias
//! thì tên không khớp note nào được giải theo alias (Obsidian không làm việc này; `lib/kg.mjs` của fixture có làm).

use std::collections::HashMap;

use super::pystr::{basename, dirname, join, normpath, strip};
use super::{casefold, nfc, strip_md};

/// Cách một link được giải.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Path,
    Title,
    Alias,
    /// Đích là file đính kèm (`.png`, `.pdf`, ...), không phải note.
    Attachment,
    /// `[[#heading]]`: cùng note.
    SelfLink,
}

impl Via {
    pub fn as_str(self) -> &'static str {
        match self {
            Via::Path => "path",
            Via::Title => "title",
            Via::Alias => "alias",
            Via::Attachment => "attachment",
            Via::SelfLink => "self",
        }
    }
}

/// Bộ giải link của một vault.
#[derive(Debug, Clone, Default)]
pub struct Resolver {
    by_path: HashMap<String, String>,
    by_name: HashMap<String, Vec<String>>,
    by_alias: HashMap<String, String>,
}

/// `NOT_NOTE = \.(?!md$)[A-Za-z0-9]{2,5}$`: đuôi 2–5 chữ hoặc số ASCII, khác `md`.
fn is_attachment(base: &str) -> bool {
    base.rsplit_once('.').is_some_and(|(_, ext)| {
        (2..=5).contains(&ext.len())
            && ext.bytes().all(|c| c.is_ascii_alphanumeric())
            && ext != "md"
    })
}

impl Resolver {
    /// `paths`: đường dẫn note theo thứ tự của bảng `nodes` (đã sắp). `aliases`: `(note, alias)` theo thứ tự chèn
    /// của bảng `names`, hoặc `None` khi tắt alias (`--no-aliases`).
    pub fn new<'a>(
        paths: impl IntoIterator<Item = &'a str>,
        aliases: Option<&[(String, String)]>,
    ) -> Self {
        let mut r = Resolver::default();
        for p in paths {
            r.by_path.insert(casefold(strip_md(p)), p.to_owned());
            r.by_name
                .entry(casefold(strip_md(basename(p))))
                .or_default()
                .push(p.to_owned());
        }
        if let Some(aliases) = aliases {
            let mut sorted: Vec<&(String, String)> = aliases.iter().collect();
            // `sorted` của Python ổn định: cùng khoá thì giữ thứ tự cũ.
            sorted.sort_by(|a, b| {
                (a.0.matches('/').count(), &a.0).cmp(&(b.0.matches('/').count(), &b.0))
            });
            for (p, a) in sorted {
                r.by_alias
                    .entry(casefold(&nfc(a)))
                    .or_insert_with(|| p.clone());
            }
        }
        r
    }

    /// Giải `target` (đích của link trong note `src`). `md`: link markdown, luôn thử như đường dẫn.
    /// Trả (đích hoặc `None`, cách giải hoặc `None` khi không giải được).
    pub fn resolve(&self, src: &str, target: &str, md: bool) -> (Option<String>, Option<Via>) {
        let t = nfc(target);
        let head = t.split_once('#').map_or(t.as_str(), |(h, _)| h);
        let mut base = strip(strip(head).trim_end_matches('\\'));
        if let Some(b) = base.strip_suffix(".md") {
            base = b;
        }
        if base.is_empty() {
            return (Some(src.to_owned()), Some(Via::SelfLink));
        }
        if is_attachment(base) {
            return (None, Some(Via::Attachment));
        }
        if md || base.contains('/') {
            let rel = normpath(&join(dirname(src), base));
            for k in [casefold(base.trim_start_matches('/')), casefold(&rel)] {
                if let Some(p) = self.by_path.get(&k) {
                    return (Some(p.clone()), Some(Via::Path));
                }
            }
        }
        let name = basename(base);
        if let Some(cands) = self.by_name.get(&casefold(name)) {
            let exact: Vec<&String> = cands
                .iter()
                .filter(|c| strip_md(basename(c)) == name)
                .collect();
            let cands: Vec<&String> = if exact.is_empty() {
                cands.iter().collect()
            } else {
                exact
            };
            let same = cands.iter().find(|c| dirname(c) == dirname(src));
            let pick = same.copied().or_else(|| {
                cands
                    .iter()
                    .min_by(|a, b| {
                        (a.matches('/').count(), a.chars().count(), a.as_str()).cmp(&(
                            b.matches('/').count(),
                            b.chars().count(),
                            b.as_str(),
                        ))
                    })
                    .copied()
            });
            return (pick.cloned(), Some(Via::Title));
        }
        match self.by_alias.get(&casefold(base)) {
            Some(p) => (Some(p.clone()), Some(Via::Alias)),
            None => (None, None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(aliases: Option<&[(String, String)]>) -> Resolver {
        Resolver::new(
            [
                "A/Ghi chú.md",
                "B/Ghi chú.md",
                "B/ghi CHÚ.md",
                "Sâu/x/Ghi chú.md",
                "Khác.md",
                "Thư mục/Con.md",
            ],
            aliases,
        )
    }

    #[test]
    fn resolves_like_obsidian() {
        let r = r(None);
        let t = |src: &str, target: &str, md: bool| {
            let (d, v) = r.resolve(src, target, md);
            (d, v.map(Via::as_str))
        };
        assert_eq!(
            t("Khác.md", "", false),
            (Some("Khác.md".into()), Some("self"))
        );
        assert_eq!(
            t("Khác.md", "#^q1", false),
            (Some("Khác.md".into()), Some("self"))
        );
        assert_eq!(t("Khác.md", "ảnh.png", false), (None, Some("attachment")));
        assert_eq!(t("Khác.md", "x.md.md", false).1, None);
        assert_eq!(
            t("A/Ghi chú.md", "ghi chú", false),
            (Some("A/Ghi chú.md".into()), Some("title"))
        );
        assert_eq!(
            t("C/x.md", "Ghi chú", false),
            (Some("A/Ghi chú.md".into()), Some("title"))
        );
        assert_eq!(
            t("B/x.md", "ghi CHÚ", false),
            (Some("B/ghi CHÚ.md".into()), Some("title"))
        );
        assert_eq!(
            t("C/x.md", "Thư mục/Con", false),
            (Some("Thư mục/Con.md".into()), Some("path"))
        );
        assert_eq!(
            t("Thư mục/y.md", "Con.md", true),
            (Some("Thư mục/Con.md".into()), Some("path"))
        );
        assert_eq!(
            t("Thư mục/y.md", "../Khác.md", true),
            (Some("Khác.md".into()), Some("path"))
        );
        assert_eq!(t("C/x.md", "Không có", false), (None, None));
        assert_eq!(
            t("C/x.md", " Khác \\", false),
            (Some("Khác.md".into()), Some("title"))
        );
    }

    #[test]
    fn aliases_only_when_enabled_and_shallowest_note_wins() {
        let aliases = vec![
            ("Thư mục/Con.md".to_owned(), "Biệt danh".to_owned()),
            ("Khác.md".to_owned(), "biệt DANH".to_owned()),
        ];
        let on = r(Some(&aliases));
        assert_eq!(
            on.resolve("x.md", "biệt danh", false),
            (Some("Khác.md".into()), Some(Via::Alias))
        );
        assert_eq!(r(None).resolve("x.md", "biệt danh", false), (None, None));
    }

    #[test]
    fn attachment_extension_rule() {
        assert!(is_attachment("a.png") && is_attachment("a.b.jpeg") && is_attachment("x.MD"));
        assert!(
            !is_attachment("a.md")
                && !is_attachment("a.x")
                && !is_attachment("a.toolong")
                && !is_attachment("v1.0")
        );
    }
}
