//! Vài hàm chuỗi và số của Python mà `index.py` và adapter dùng: `float(str)`, `json.dumps`, `str.strip`.

/// `str.isspace()` của Python cho một ký tự (bidi WS, B, S hoặc nhóm Zs); khác `char::is_whitespace` ở
/// U+001C..U+001F. Cùng tập với `vault` dùng khi tách frontmatter.
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

/// `float(s)` của Python với chuỗi: bỏ khoảng trắng hai đầu, nhận `_` giữa hai chữ số, `inf`, `infinity`, `nan`
/// (không phân biệt hoa thường, có dấu). `None` khi Python ném `ValueError`. Chỉ nhận chữ số ASCII (Python nhận cả
/// chữ số Unicode khác, như `١٢`).
pub(crate) fn float(s: &str) -> Option<f64> {
    let t = s.trim_matches(is_space);
    if t.is_empty() || !t.is_ascii() {
        return None;
    }
    let b = t.as_bytes();
    let mut plain = String::with_capacity(t.len());
    for (i, &c) in b.iter().enumerate() {
        if c == b'_' {
            let digit = |j: Option<usize>| j.and_then(|j| b.get(j)).is_some_and(u8::is_ascii_digit);
            if !(digit(i.checked_sub(1)) && digit(Some(i + 1))) {
                return None;
            }
        } else {
            plain.push(char::from(c));
        }
    }
    // `str::parse::<f64>` nhận đúng văn phạm của Python cho phần còn lại (số thập phân, `e`, `inf`, `infinity`,
    // `nan`), trừ `_` đã xử lý ở trên.
    plain.parse().ok()
}

/// Chuỗi JSON như `json.dumps(s)` (`ensure_ascii`: ký tự ngoài ASCII thành `\uXXXX`, cặp surrogate với ký tự ngoài
/// BMP; hex viết thường).
pub(crate) fn json_str(s: &str, ensure_ascii: bool) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if ensure_ascii && !c.is_ascii() => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{u:04x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `json.dumps(list_of_str, ensure_ascii=...)`.
pub(crate) fn json_list<'a>(
    items: impl IntoIterator<Item = &'a str>,
    ensure_ascii: bool,
) -> String {
    let parts: Vec<String> = items
        .into_iter()
        .map(|s| json_str(s, ensure_ascii))
        .collect();
    format!("[{}]", parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_follows_python() {
        assert_eq!(float("0.62"), Some(0.62));
        assert_eq!(float("  1_000.5\n"), Some(1000.5));
        assert_eq!(float("1e3"), Some(1000.0));
        assert_eq!(float("-.5"), Some(-0.5));
        assert_eq!(float("5."), Some(5.0));
        assert_eq!(float("+Infinity"), Some(f64::INFINITY));
        assert!(float("nan").is_some_and(f64::is_nan));
        for bad in [
            "", " ", "abc", "1__0", "_1", "1_", "1_.5", "1._5", "0x10", "1e", "None", "1,5",
        ] {
            assert_eq!(float(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn json_str_like_python_dumps() {
        assert_eq!(json_str("accepted", true), "\"accepted\"");
        assert_eq!(json_str("chờ", true), "\"ch\\u1edd\"");
        assert_eq!(json_str("chờ", false), "\"chờ\"");
        assert_eq!(
            json_str("a\"b\\c\n\u{1}", false),
            "\"a\\\"b\\\\c\\n\\u0001\""
        );
        assert_eq!(json_str("😀", true), "\"\\ud83d\\ude00\"");
        assert_eq!(
            json_list(["_E11/", "Trang chủ.md"], false),
            "[\"_E11/\", \"Trang chủ.md\"]"
        );
        assert_eq!(json_list([], false), "[]");
    }
}
