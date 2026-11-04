//! Ghi frontmatter như `yaml.safe_dump(x, sort_keys=False, allow_unicode=True, width=10_000)` của PyYAML 6, đủ cho
//! note đề xuất: dict giữ thứ tự, list, chuỗi, số, `null`, `true`/`false`; dạng khối, list trong dict không thụt
//! (`notes:\n- a`), dict hay list rỗng là `{}`/`[]`.
//!
//! Kiểu chuỗi chọn như `Emitter.choose_scalar_style`: trơn khi `analyze_scalar` cho phép và đọc lại vẫn là chuỗi
//! (resolver YAML 1.1: `'2026-11-04'`, `'yes'`, `'1.5'` phải đặt trong nháy), rồi nháy đơn, rồi nháy kép. Không gập
//! dòng theo `width`: với `width=10_000` PyYAML chỉ gập chuỗi dài hơn 10.000 ký tự, đề xuất không có chuỗi như vậy.

use crate::vault::yaml::{self, Loader, Obj};

/// Một giá trị YAML để ghi.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Node {
    Null,
    Bool(bool),
    /// Số nguyên đã ở dạng thập phân.
    Int(String),
    Float(f64),
    Str(String),
    Seq(Vec<Node>),
    Map(Vec<(String, Node)>),
}

impl Node {
    /// Giá trị JSON (điểm của cặp gần nghĩa) thành YAML như PyYAML ghi object Python mà `json.load` trả.
    pub(crate) fn from_json(v: &serde_json::Value) -> Node {
        use serde_json::Value;
        match v {
            Value::Null => Node::Null,
            Value::Bool(b) => Node::Bool(*b),
            Value::Number(n) if n.is_f64() => Node::Float(n.as_f64().unwrap_or(f64::NAN)),
            Value::Number(n) => Node::Int(n.to_string()),
            Value::String(s) => Node::Str(s.clone()),
            Value::Array(a) => Node::Seq(a.iter().map(Node::from_json).collect()),
            Value::Object(m) => Node::Map(
                m.iter()
                    .map(|(k, v)| (k.clone(), Node::from_json(v)))
                    .collect(),
            ),
        }
    }
}

/// Văn bản YAML của một dict ở gốc, kết thúc bằng `\n` (rỗng: `{}\n`).
pub(crate) fn dump(entries: &[(String, Node)]) -> String {
    let mut out = String::new();
    if entries.is_empty() {
        out.push_str("{}\n");
    } else {
        map(&mut out, entries, 0, false);
    }
    out
}

fn pad(out: &mut String, n: usize) {
    out.extend(std::iter::repeat_n(' ', n));
}

/// Dict khối có khoá ở cột `indent`; `inline_first`: khoá đầu viết ngay sau `- ` của mục list.
fn map(out: &mut String, entries: &[(String, Node)], indent: usize, inline_first: bool) {
    for (i, (k, v)) in entries.iter().enumerate() {
        if i > 0 || !inline_first {
            pad(out, indent);
        }
        out.push_str(&scalar_str(k, indent + 2, true));
        out.push(':');
        match v {
            Node::Map(m) if !m.is_empty() => {
                out.push('\n');
                map(out, m, indent + 2, false);
            }
            // List trong dict không thụt thêm (`indentless`).
            Node::Seq(s) if !s.is_empty() => {
                out.push('\n');
                seq(out, s, indent, false);
            }
            _ => {
                out.push(' ');
                out.push_str(&leaf(v, indent + 2));
                out.push('\n');
            }
        }
    }
}

/// List khối có `- ` ở cột `indent`.
fn seq(out: &mut String, items: &[Node], indent: usize, inline_first: bool) {
    for (i, item) in items.iter().enumerate() {
        if i > 0 || !inline_first {
            pad(out, indent);
        }
        out.push_str("- ");
        match item {
            Node::Map(m) if !m.is_empty() => map(out, m, indent + 2, true),
            Node::Seq(s) if !s.is_empty() => seq(out, s, indent + 2, true),
            _ => {
                out.push_str(&leaf(item, indent + 2));
                out.push('\n');
            }
        }
    }
}

/// Giá trị không phải container khối (cả `{}`, `[]`); `indent`: cột của dòng nối trong chuỗi nháy đơn nhiều dòng.
fn leaf(v: &Node, indent: usize) -> String {
    match v {
        Node::Null => "null".into(),
        Node::Bool(b) => if *b { "true" } else { "false" }.into(),
        Node::Int(s) => s.clone(),
        Node::Float(f) => float(*f),
        Node::Str(s) => scalar_str(s, indent, false),
        Node::Seq(_) => "[]".into(),
        Node::Map(_) => "{}".into(),
    }
}

/// `SafeRepresenter.represent_float`.
fn float(x: f64) -> String {
    if x.is_nan() {
        return ".nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { ".inf" } else { "-.inf" }.into();
    }
    let v = yaml::repr::float_repr(x).to_lowercase();
    if !v.contains('.') && v.contains('e') {
        v.replacen('e', ".0e", 1)
    } else {
        v
    }
}

fn is_break(c: char) -> bool {
    matches!(c, '\n' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

fn is_blank_or_end(c: Option<char>) -> bool {
    matches!(
        c,
        None | Some('\0' | ' ' | '\t' | '\r' | '\n' | '\u{85}' | '\u{2028}' | '\u{2029}')
    )
}

/// Kết quả của `Emitter.analyze_scalar` với `allow_unicode=True`.
struct Analysis {
    empty: bool,
    multiline: bool,
    allow_block_plain: bool,
    allow_single_quoted: bool,
}

fn analyze(s: &str) -> Analysis {
    if s.is_empty() {
        return Analysis {
            empty: true,
            multiline: false,
            allow_block_plain: true,
            allow_single_quoted: true,
        };
    }
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut block_indicators = s.starts_with("---") || s.starts_with("...");
    let (mut line_breaks, mut special) = (false, false);
    let (mut leading_space, mut leading_break) = (false, false);
    let (mut trailing_space, mut trailing_break) = (false, false);
    let (mut break_space, mut space_break) = (false, false);
    let (mut previous_space, mut previous_break) = (false, false);
    let mut preceded_by_whitespace = true;
    let mut followed_by_whitespace = n == 1 || is_blank_or_end(chars.get(1).copied());
    for (i, &ch) in chars.iter().enumerate() {
        if i == 0 {
            if "#,[]{}&*!|>'\"%@`".contains(ch) || ("?:-".contains(ch) && followed_by_whitespace) {
                block_indicators = true;
            }
        } else if (ch == ':' && followed_by_whitespace) || (ch == '#' && preceded_by_whitespace) {
            block_indicators = true;
        }
        if is_break(ch) {
            line_breaks = true;
        }
        let printable_ascii = ch == '\n' || (' '..='~').contains(&ch);
        let unicode = (ch == '\u{85}'
            || ('\u{a0}'..='\u{d7ff}').contains(&ch)
            || ('\u{e000}'..='\u{fffd}').contains(&ch)
            || ('\u{10000}'..'\u{10ffff}').contains(&ch))
            && ch != '\u{feff}';
        if !printable_ascii && !unicode {
            special = true;
        }
        if ch == ' ' {
            leading_space |= i == 0;
            trailing_space |= i == n - 1;
            break_space |= previous_break;
            (previous_space, previous_break) = (true, false);
        } else if is_break(ch) {
            leading_break |= i == 0;
            trailing_break |= i == n - 1;
            space_break |= previous_space;
            (previous_space, previous_break) = (false, true);
        } else {
            (previous_space, previous_break) = (false, false);
        }
        preceded_by_whitespace = is_blank_or_end(Some(ch));
        followed_by_whitespace = i + 2 >= n || is_blank_or_end(chars.get(i + 2).copied());
    }
    let mut allow_block_plain = true;
    let mut allow_single_quoted = true;
    if leading_space || leading_break || trailing_space || trailing_break {
        allow_block_plain = false;
    }
    if break_space {
        allow_block_plain = false;
        allow_single_quoted = false;
    }
    if space_break || special || line_breaks || block_indicators {
        allow_block_plain = false;
    }
    if space_break || special {
        allow_single_quoted = false;
    }
    Analysis {
        empty: false,
        multiline: line_breaks,
        allow_block_plain,
        allow_single_quoted,
    }
}

/// Chuỗi viết trơn có được đọc lại thành chính nó không (`implicit[0]` của `Serializer`): resolver YAML 1.1 không
/// đổi nó thành số, ngày, `null`, `bool`, `<<` hay `=`.
fn resolves_to_itself(s: &str) -> bool {
    if s == "<<" || s == "=" {
        return false;
    }
    match yaml::load(s, Loader::Safe) {
        Ok(Some(doc)) => matches!(doc.root().obj(), Obj::Str(v) if v == s),
        _ => false,
    }
}

/// Chuỗi ở chỗ một giá trị (hoặc khoá đơn giản nếu `key`), kiểu do PyYAML chọn.
fn scalar_str(s: &str, indent: usize, key: bool) -> String {
    let a = analyze(s);
    if !(key && (a.empty || a.multiline)) && a.allow_block_plain && resolves_to_itself(s) {
        return s.to_owned();
    }
    if a.allow_single_quoted && !(key && a.multiline) {
        return single_quoted(s, indent);
    }
    double_quoted(s)
}

/// `Emitter.write_single_quoted`: `'` gấp đôi; mỗi dãy xuống dòng ghi lại (dãy bắt đầu bằng `\n` thêm một dòng
/// trống), rồi thụt tới `indent`.
fn single_quoted(s: &str, indent: usize) -> String {
    let mut out = String::from("'");
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if is_break(c) {
            let start = i;
            while i < chars.len() && is_break(chars[i]) {
                i += 1;
            }
            if chars[start] == '\n' {
                out.push('\n');
            }
            out.extend(&chars[start..i]);
            pad(&mut out, indent);
            continue;
        }
        if c == '\'' {
            out.push_str("''");
        } else {
            out.push(c);
        }
        i += 1;
    }
    out.push('\'');
    out
}

/// `Emitter.write_double_quoted` với `allow_unicode=True`.
fn double_quoted(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        let named = match c {
            '\0' => Some('0'),
            '\u{7}' => Some('a'),
            '\u{8}' => Some('b'),
            '\t' => Some('t'),
            '\n' => Some('n'),
            '\u{b}' => Some('v'),
            '\u{c}' => Some('f'),
            '\r' => Some('r'),
            '\u{1b}' => Some('e'),
            '"' => Some('"'),
            '\\' => Some('\\'),
            '\u{85}' => Some('N'),
            '\u{a0}' => Some('_'),
            '\u{2028}' => Some('L'),
            '\u{2029}' => Some('P'),
            _ => None,
        };
        let plain = (' '..='~').contains(&c)
            || ('\u{a0}'..='\u{d7ff}').contains(&c)
            || ('\u{e000}'..='\u{fffd}').contains(&c);
        match named {
            Some(e) => {
                out.push('\\');
                out.push(e);
            }
            None if plain && c != '\u{feff}' => out.push(c),
            None => {
                let n = u32::from(c);
                if n <= 0xff {
                    out.push_str(&format!("\\x{n:02X}"));
                } else if n <= 0xffff {
                    out.push_str(&format!("\\u{n:04X}"));
                } else {
                    out.push_str(&format!("\\U{n:08X}"));
                }
            }
        }
    }
    out.push('"');
    out
}
