//! `json.dumps` của Python cho mọi chỗ `pkm` in JSON mà bản Python in bằng `json`: cùng dấu phân cách, cùng cách thụt
//! lề, cùng cách thoát ký tự (`ensure_ascii`), số thực in như `repr(float)` (`1e-05`, `1.0`, `1e+16`).
//!
//! `serde_json` khác Python ở ba chỗ này (`1e-5`, không có `, `/`: ` mặc định, không thoát ký tự ngoài ASCII), nên
//! đầu ra của CLI và bản tin MCP đi qua đây.

use serde_json::{Number, Value};

/// Cách in, tương ứng tham số của `json.dumps`.
#[derive(Debug, Clone, Copy)]
pub struct Style {
    /// `indent=N`: mỗi phần tử một dòng; `None` là một dòng.
    pub indent: Option<usize>,
    /// `separators=(',', ':')` (chỉ có nghĩa khi không thụt lề); mặc định Python là `', '` và `': '`.
    pub compact: bool,
    pub ensure_ascii: bool,
}

/// `json.dumps(x)`: một dòng, `', '` và `': '`, thoát ký tự ngoài ASCII.
pub const DEFAULT: Style = Style {
    indent: None,
    compact: false,
    ensure_ascii: true,
};

/// `json.dumps(x, ensure_ascii=False)`.
pub const LINE: Style = Style {
    indent: None,
    compact: false,
    ensure_ascii: false,
};

/// `json.dumps(x, ensure_ascii=False, separators=(',', ':'))`.
pub const COMPACT: Style = Style {
    indent: None,
    compact: true,
    ensure_ascii: false,
};

/// `json.dump(x, ensure_ascii=False, indent=1)` (đầu ra JSON của CLI).
pub const INDENT1: Style = Style {
    indent: Some(1),
    compact: false,
    ensure_ascii: false,
};

pub fn dumps(v: &Value, style: Style) -> String {
    let mut out = String::new();
    write_value(&mut out, v, style, 0);
    out
}

fn write_value(out: &mut String, v: &Value, style: Style, depth: usize) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&number(n)),
        Value::String(s) => write_str(out, s, style.ensure_ascii),
        Value::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, x) in items.iter().enumerate() {
                separator(out, i, style, depth + 1);
                write_value(out, x, style, depth + 1);
            }
            close(out, style, depth);
            out.push(']');
        }
        Value::Object(map) => {
            if map.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (k, x)) in map.iter().enumerate() {
                separator(out, i, style, depth + 1);
                write_str(out, k, style.ensure_ascii);
                out.push_str(if style.compact && style.indent.is_none() {
                    ":"
                } else {
                    ": "
                });
                write_value(out, x, style, depth + 1);
            }
            close(out, style, depth);
            out.push('}');
        }
    }
}

/// Dấu phân cách trước phần tử thứ `i` (và xuống dòng, thụt lề khi có `indent`).
fn separator(out: &mut String, i: usize, style: Style, depth: usize) {
    match style.indent {
        Some(n) => {
            if i > 0 {
                out.push(',');
            }
            out.push('\n');
            out.push_str(&" ".repeat(n * depth));
        }
        None if i > 0 => out.push_str(if style.compact { "," } else { ", " }),
        None => {}
    }
}

fn close(out: &mut String, style: Style, depth: usize) {
    if let Some(n) = style.indent {
        out.push('\n');
        out.push_str(&" ".repeat(n * depth));
    }
}

/// Chuỗi JSON như `json.encoder`: `ensure_ascii` thoát mọi ký tự ngoài ` `..`~` (ký tự ngoài BMP thành cặp
/// surrogate); không `ensure_ascii` thì chỉ thoát `"`, `\` và ký tự điều khiển dưới 0x20.
fn write_str(out: &mut String, s: &str, ensure_ascii: bool) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if ensure_ascii && !(' '..='~').contains(&c) => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn number(n: &Number) -> String {
    if let Some(i) = n.as_i64() {
        return i.to_string();
    }
    if let Some(u) = n.as_u64() {
        return u.to_string();
    }
    float_repr(n.as_f64().unwrap_or(f64::NAN))
}

/// `repr(float)` của Python: số chữ số ngắn nhất đọc lại đúng; dạng thường khi số mũ thập phân trong `[-4, 16)`,
/// không thì dạng mũ có dấu và ít nhất hai chữ số (`1e-05`, `1.5e+16`). Số nguyên thêm `.0`.
pub fn float_repr(x: f64) -> String {
    if x.is_nan() {
        return "NaN".to_owned();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.to_owned();
    }
    // `{:e}` của Rust cho đúng các chữ số ngắn nhất: "1.2345e-5", "1e16", "-0e0".
    let sci = format!("{:e}", x.abs());
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let sign = if x.is_sign_negative() { "-" } else { "" };
    if !(-4..16).contains(&exp) {
        let (first, rest) = digits.split_at(1);
        let frac = if rest.is_empty() {
            String::new()
        } else {
            format!(".{rest}")
        };
        let esign = if exp < 0 { '-' } else { '+' };
        return format!("{sign}{first}{frac}e{esign}{:02}", exp.abs());
    }
    if exp < 0 {
        let zeros = "0".repeat(usize::try_from(-exp - 1).unwrap_or(0));
        return format!("{sign}0.{zeros}{digits}");
    }
    let int_len = usize::try_from(exp + 1).unwrap_or(1);
    if digits.len() <= int_len {
        let pad = "0".repeat(int_len - digits.len());
        format!("{sign}{digits}{pad}.0")
    } else {
        let (int, frac) = digits.split_at(int_len);
        format!("{sign}{int}.{frac}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn float_repr_matches_python() {
        let cases = [
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (1.0, "1.0"),
            (12.5, "12.5"),
            (0.1, "0.1"),
            (1e-4, "0.0001"),
            (1e-5, "1e-05"),
            (1.5e-7, "1.5e-07"),
            (123_456_789_012_345.6, "123456789012345.6"),
            (1e15, "1000000000000000.0"),
            (1e16, "1e+16"),
            (1.2345e100, "1.2345e+100"),
            (-2.5e-10, "-2.5e-10"),
            (0.30000000000000004, "0.30000000000000004"),
        ];
        for (x, want) in cases {
            assert_eq!(float_repr(x), want, "{x}");
        }
    }

    #[test]
    fn separators_and_indent_like_json_dumps() {
        let v = json!({"a": [1, 2.0, "x"], "b": {}, "c": [], "d": null, "e": true});
        assert_eq!(
            dumps(&v, LINE),
            r#"{"a": [1, 2.0, "x"], "b": {}, "c": [], "d": null, "e": true}"#
        );
        assert_eq!(
            dumps(&v, COMPACT),
            r#"{"a":[1,2.0,"x"],"b":{},"c":[],"d":null,"e":true}"#
        );
        assert_eq!(
            dumps(&json!({"a": [1, {"b": 2}], "c": []}), INDENT1),
            "{\n \"a\": [\n  1,\n  {\n   \"b\": 2\n  }\n ],\n \"c\": []\n}"
        );
    }

    #[test]
    fn escaping_follows_ensure_ascii() {
        let v = json!("Đức \"x\"\\\n\t\u{1}\u{7f}😀");
        assert_eq!(
            dumps(&v, LINE),
            "\"Đức \\\"x\\\"\\\\\\n\\t\\u0001\u{7f}😀\""
        );
        assert_eq!(
            dumps(&v, DEFAULT),
            "\"\\u0110\\u1ee9c \\\"x\\\"\\\\\\n\\t\\u0001\\u007f\\ud83d\\ude00\""
        );
    }
}
