//! Hàm chuỗi và đường dẫn của Python mà `vault.py` dùng, viết lại cho đúng từng ký tự: `str.isspace`, `strip`,
//! `split()`, `urllib.parse.unquote`, và `posixpath` (`dirname`, `basename`, `join`, `normpath`).
//!
//! Đường dẫn note trong index luôn dùng `/`, nên link được giải theo `posixpath` trên mọi nền (bản đặc tả chạy
//! trên Linux/WSL; golden dựng trên Linux).

/// `str.isspace()` của Python cho một ký tự: bidi WS, B, S hoặc nhóm Zs. Khác `char::is_whitespace` ở U+001C..U+001F.
pub(crate) fn is_space(c: char) -> bool {
    matches!(
        c,
        '\u{9}'..='\u{d}'
            | '\u{1c}'..='\u{20}'
            | '\u{85}'
            | '\u{a0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200a}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
    )
}

/// `str.strip()`.
pub(crate) fn strip(s: &str) -> &str {
    s.trim_matches(is_space)
}

/// `str.split()` không tham số.
pub(crate) fn split_whitespace(s: &str) -> impl Iterator<Item = &str> {
    s.split(is_space).filter(|p| !p.is_empty())
}

/// `urllib.parse.unquote(s)` (UTF-8, `errors='replace'`): giải `%XX` trong từng đoạn ASCII liền nhau; ký tự
/// ngoài ASCII giữ nguyên; byte giải ra không thành UTF-8 hợp lệ được thay bằng U+FFFD.
pub(crate) fn unquote(s: &str) -> String {
    if !s.contains('%') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while !rest.is_empty() {
        let ascii_end = rest.find(|c: char| !c.is_ascii()).unwrap_or(rest.len());
        if ascii_end == 0 {
            let other_end = rest.find(|c: char| c.is_ascii()).unwrap_or(rest.len());
            out.push_str(&rest[..other_end]);
            rest = &rest[other_end..];
            continue;
        }
        out.push_str(&String::from_utf8_lossy(&unquote_to_bytes(
            &rest.as_bytes()[..ascii_end],
        )));
        rest = &rest[ascii_end..];
    }
    out
}

fn unquote_to_bytes(b: &[u8]) -> Vec<u8> {
    let hex = |c: u8| char::from(c).to_digit(16);
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2]))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// `posixpath.basename`.
pub(crate) fn basename(p: &str) -> &str {
    p.rsplit_once('/').map_or(p, |(_, b)| b)
}

/// `posixpath.dirname`.
pub(crate) fn dirname(p: &str) -> &str {
    let head = p.rfind('/').map_or("", |i| &p[..=i]);
    if !head.is_empty() && head.bytes().any(|c| c != b'/') {
        head.trim_end_matches('/')
    } else {
        head
    }
}

/// `posixpath.join(a, b)`.
pub(crate) fn join(a: &str, b: &str) -> String {
    if b.starts_with('/') {
        b.to_owned()
    } else if a.is_empty() || a.ends_with('/') {
        format!("{a}{b}")
    } else {
        format!("{a}/{b}")
    }
}

/// `posixpath.normpath`.
pub(crate) fn normpath(p: &str) -> String {
    if p.is_empty() {
        return ".".into();
    }
    let lead = if p.starts_with("//") && !p.starts_with("///") {
        "//"
    } else if p.starts_with('/') {
        "/"
    } else {
        ""
    };
    let mut parts: Vec<&str> = Vec::new();
    for c in p.split('/') {
        if c.is_empty() || c == "." {
            continue;
        }
        if c != ".." || (lead.is_empty() && parts.last().is_none_or(|&l| l == "..")) {
            parts.push(c);
        } else if !parts.is_empty() {
            parts.pop();
        }
    }
    let out = format!("{lead}{}", parts.join("/"));
    if out.is_empty() { ".".into() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_follows_python() {
        assert!(is_space('\u{1c}') && is_space('\u{a0}') && is_space('\u{3000}'));
        assert!(!is_space('\u{200b}') && !is_space('\u{180e}') && !is_space('x'));
        assert_eq!(strip("\u{a0} a b\u{2003}\n"), "a b");
        assert_eq!(
            split_whitespace(" a \u{1f}b\tc  ").collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
    }

    #[test]
    fn unquote_follows_urllib() {
        assert_eq!(unquote("Ghi%20ch%C3%BA.md"), "Ghi chú.md");
        assert_eq!(unquote("a%2"), "a%2");
        assert_eq!(unquote("a%zz%41"), "a%zzA");
        assert_eq!(unquote("%C3"), "\u{fffd}");
        assert_eq!(unquote("é%41é"), "éAé");
        assert_eq!(unquote("no-percent"), "no-percent");
    }

    #[test]
    fn posixpath_follows_python() {
        assert_eq!(dirname("a/b/c.md"), "a/b");
        assert_eq!(dirname("c.md"), "");
        assert_eq!(dirname("/c.md"), "/");
        assert_eq!(dirname("a//c.md"), "a");
        assert_eq!(basename("a/b/c.md"), "c.md");
        assert_eq!(basename("a/"), "");
        assert_eq!(join("", "x"), "x");
        assert_eq!(join("a", "/x"), "/x");
        assert_eq!(join("a", "x"), "a/x");
        for (p, want) in [
            ("", "."),
            ("a/../..", ".."),
            ("a/./b/../c", "a/c"),
            ("/../a", "/a"),
            ("//a//b/", "//a/b"),
            ("///a", "/a"),
            ("../x/../../y", "../../y"),
            ("a/..", "."),
        ] {
            assert_eq!(normpath(p), want, "{p}");
        }
    }
}
