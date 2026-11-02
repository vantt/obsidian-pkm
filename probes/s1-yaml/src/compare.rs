//! Compare candidate records with the PyYAML reference and count differences by kind.
//!
//! Category names never contain note content (paths, keys, values), so the counts can be printed for a
//! private vault. Only `--show` prints details.

use crate::py11::{INT, STR, resolve_plain};
use regex::Regex;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

static DATE_LIKE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[0-9]{4}-[0-9]{1,2}-[0-9]{1,2}(?:[Tt ]|$)").expect("static regex compiles")
});
/// YAML 1.2 core-schema int / float shapes.
static CORE_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:[-+]?[0-9]+|0o[0-7]+|0x[0-9a-fA-F]+|[-+]?(?:\.[0-9]+|[0-9]+(?:\.[0-9]*)?)(?:[eE][-+]?[0-9]+)?|[-+]?\.(?:inf|Inf|INF)|\.(?:nan|NaN|NAN))$")
        .expect("static regex compiles")
});

/// Fixed phrases an error message is reduced to. Messages can quote note content (keys, values, tags),
/// so only a phrase from this list is ever printed; anything else becomes "other message".
const ERROR_PHRASES: &[&str] = &[
    // vault.parse
    "frontmatter không phải bảng khoá",
    "frontmatter không đóng",
    // libyaml 0.2.5 contexts and problems (scanner, parser, reader)
    "block sequence entries are not allowed in this context",
    "control characters are not allowed",
    "could not find expected ':'",
    "did not find expected <document start>",
    "did not find expected alphabetic or numeric character",
    "did not find expected comment or line break",
    "did not find expected hexdecimal number",
    "did not find expected '-' indicator",
    "did not find expected key",
    "did not find expected node content",
    "did not find expected ',' or ']'",
    "did not find expected whitespace or line break",
    "did not find expected whitespace",
    "found a tab character that violates indentation",
    "found a tab character where an indentation space is expected",
    "found character that cannot start any token",
    "found invalid Unicode character escape code",
    "found undefined tag handle",
    "found unexpected ':'",
    "found unexpected document indicator",
    "found unexpected end of stream",
    "found unknown escape character",
    "mapping keys are not allowed in this context",
    "mapping values are not allowed in this context",
    "while parsing a block collection",
    "while parsing a block mapping",
    "while parsing a block node",
    "while parsing a flow mapping",
    "while parsing a flow node",
    "while parsing a flow sequence",
    "while parsing a node",
    "while parsing a quoted scalar",
    "while parsing a tag",
    "while scanning a block scalar",
    "while scanning a directive",
    "while scanning an alias",
    "while scanning an anchor",
    "while scanning a plain scalar",
    "while scanning a quoted scalar",
    "while scanning a simple key",
    "while scanning a tag",
    "while scanning for the next token",
    // PyYAML composer and SafeConstructor
    "unacceptable character",
    "found duplicate anchor",
    "found undefined alias",
    "expected a single document in the stream",
    "could not determine a constructor for the tag",
    "while constructing a mapping",
    "while constructing an ordered map",
    "while constructing pairs",
    "expected a scalar node",
    "expected a sequence node",
    "expected a mapping node",
    "failed to decode base64 data",
    "failed to convert base64 data into ascii",
    // Python ValueError from int() / float() / datetime
    "invalid literal for int()",
    "could not convert string to float",
    "year must be in",
    "month must be in",
    "must be in range",
    "hour must be in",
    "minute must be in",
    "second must be in",
    "offset must be a timedelta",
    // Rust crates
    "duplicate entry",
    "duplicated key",
    "unknown anchor",
    "tabs disallowed",
    "unexpected character",
    "more than one document",
    "probe:",
];

/// Content-free label for an error message: the earliest known phrase in it.
pub fn error_class(msg: &str) -> &'static str {
    ERROR_PHRASES
        .iter()
        .filter_map(|p| msg.find(p).map(|at| (at, *p)))
        .min_by_key(|(at, p)| (*at, std::cmp::Reverse(p.len())))
        .map_or("other message", |(_, p)| p)
}

fn tv(v: &Value) -> (&str, &Value) {
    (v[0].as_str().unwrap_or("?"), &v[1])
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

fn classify_leaf(p: &Value, r: &Value) -> String {
    let ((pt, pv), (rt, rv)) = (tv(p), tv(r));
    let c = match (pt, rt) {
        ("bool", "str") if matches!(s(rv).to_lowercase().as_str(), "yes" | "no" | "on" | "off") => {
            "bool: yes/no/on/off is bool in 1.1, str in Rust"
        }
        ("str", "date" | "datetime") => "date/time: str in Python, date in Rust",
        ("str", _) | (_, "str") if DATE_LIKE.is_match(s(pv)) || DATE_LIKE.is_match(s(rv)) => {
            "date/time: other"
        }
        ("int", "str") if s(rv).contains(':') => "int: sexagesimal 1:30 is int in 1.1",
        ("int", "str") if s(rv).contains('_') => "int: underscore 1_000 is int in 1.1",
        ("int", "str") if s(rv).trim_start_matches(['-', '+']).starts_with("0b") => {
            "int: 0b binary is int in 1.1"
        }
        ("int", "str") => "int: other 1.1-only int",
        ("int", "int") => "int: leading-zero octal 010 (8 in 1.1, 10 in 1.2)",
        ("str", "int") if s(pv).starts_with("0o") => "int: 0o octal is int only in 1.2",
        ("str", "int") => "int: leading zero with 8/9 (0912...) is str in 1.1, int in 1.2",
        ("float", "str") => "float: 1.1-only float form",
        ("str", "float") => "float: 1e3 without dot is str in 1.1, float in 1.2",
        ("float", "float") => "float: different value",
        ("int", "float") | ("float", "int") => "number: int vs float",
        ("null", _) | (_, "null") => "null: ~/empty/Null forms",
        ("str", "str") => {
            let (a, b) = (s(pv), s(rv));
            if a.nfc().eq(b.nfc()) {
                "unicode: NFC/NFD changed"
            } else if a.contains('\r') || b.contains('\r') {
                "CRLF kept in a string"
            } else if a.contains('\n') || b.contains('\n') {
                "multi-line string"
            } else {
                "string: other"
            }
        }
        ("bytes" | "set" | "tuple" | "date" | "datetime", _) => {
            return format!("explicit tag: {pt} in Python, {rt} in Rust");
        }
        _ => return format!("type: {pt} in Python, {rt} in Rust"),
    };
    c.to_string()
}

fn walk(p: &Value, r: &Value, out: &mut Vec<String>) {
    if p == r {
        return;
    }
    let ((pt, pv), (rt, rv)) = (tv(p), tv(r));
    match (pt, rt) {
        ("map", "map") => {
            let (a, b) = (pv.as_array(), rv.as_array());
            let (Some(a), Some(b)) = (a, b) else { return };
            if a.len() != b.len() {
                out.push("map: different key count (duplicate key, merge <<, key equality)".into());
                return;
            }
            for (x, y) in a.iter().zip(b) {
                walk(&x[0], &y[0], out);
                walk(&x[1], &y[1], out);
            }
        }
        ("list", "list") => {
            let (Some(a), Some(b)) = (pv.as_array(), rv.as_array()) else {
                return;
            };
            if a.len() != b.len() {
                out.push("list: different length".into());
                return;
            }
            for (x, y) in a.iter().zip(b) {
                walk(x, y, out);
            }
        }
        _ => out.push(classify_leaf(p, r)),
    }
}

/// Difference categories for one note (empty when the candidate matches the reference exactly).
pub fn diff(py: &Value, rs: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let (pc, rc) = (py["crash"].as_str(), rs["crash"].as_str());
    if pc.is_some() || rc.is_some() {
        if pc != rc {
            out.push(format!(
                "crash: Python {}, Rust {}",
                pc.unwrap_or("none"),
                rc.unwrap_or("none")
            ));
        }
        return out;
    }
    let (pe, re) = (py["error"].as_str(), rs["error"].as_str());
    match (pe, re) {
        (Some(a), None) => out.push(format!("error only in Python [{}]", error_class(a))),
        (None, Some(b)) => out.push(format!("error only in Rust [{}]", error_class(b))),
        (Some(a), Some(b)) if a != b => out.push(format!(
            "error message differs [{} | {}]",
            error_class(a),
            error_class(b)
        )),
        _ => {}
    }
    if out.is_empty() {
        walk(&py["fm"], &rs["fm"], &mut out);
        if out.is_empty() && py["props"] != rs["props"] {
            out.push("props: same values, different str()/repr() text".into());
        }
    }
    out
}

/// How often each risky YAML shape occurs in a note's frontmatter (read with libyaml events).
pub fn census(yaml: &str, out: &mut BTreeMap<&'static str, usize>) {
    use libyaml_safer::{EventData as D, ScalarStyle};
    let mut bump = |k: &'static str| *out.entry(k).or_default() += 1;
    bump("notes with frontmatter");
    if yaml.contains('\r') {
        bump("frontmatter with CRLF");
    }
    let mut bytes = yaml.as_bytes();
    let mut parser = libyaml_safer::Parser::new();
    parser.set_input_string(&mut bytes);
    // Per open collection: Some(keys seen, expecting key) for mappings, None for sequences.
    let mut stack: Vec<Option<(HashSet<String>, bool)>> = Vec::new();
    loop {
        let ev = match parser.parse() {
            Ok(ev) => ev,
            Err(_) => {
                bump("YAML syntax error (libyaml)");
                return;
            }
        };
        let is_key = matches!(stack.last(), Some(Some((_, true))));
        if let Some(Some((_, expecting))) = stack.last_mut()
            && !matches!(ev.data, D::MappingEnd)
        {
            *expecting = !*expecting;
        }
        let is_map = matches!(ev.data, D::MappingStart { .. });
        match ev.data {
            D::StreamEnd => return,
            D::Alias { .. } => bump("alias *x"),
            D::Scalar {
                anchor,
                tag,
                value,
                style,
                ..
            } => {
                if anchor.is_some() {
                    bump("anchor &x");
                }
                if is_key
                    && let Some(Some((keys, _))) = stack.last_mut()
                    && !keys.insert(value.clone())
                {
                    bump("duplicate key");
                }
                if tag.is_some() {
                    bump("explicit tag");
                }
                if matches!(style, ScalarStyle::Literal | ScalarStyle::Folded) {
                    bump("block scalar | or >");
                }
                if value.contains('\n') {
                    bump("multi-line string");
                }
                if !value.is_ascii() {
                    bump("non-ASCII string");
                    if !value.nfc().eq(value.chars()) {
                        bump("string not in NFC");
                    }
                }
                if style != ScalarStyle::Plain || tag.is_some() {
                    continue;
                }
                let t = resolve_plain(&value);
                let bare = value.trim_start_matches(['-', '+']);
                match t {
                    crate::py11::BOOL
                        if !matches!(value.to_lowercase().as_str(), "true" | "false") =>
                    {
                        bump("plain yes/no/on/off")
                    }
                    crate::py11::NULL if value.is_empty() || value == "~" => {
                        bump("plain ~ or empty null")
                    }
                    crate::py11::MERGE => bump("merge key <<"),
                    crate::py11::VALUE => bump("plain = (value tag)"),
                    INT if value.contains(['_', ':'])
                        || bare.starts_with("0b")
                        || (bare.starts_with('0') && bare.len() > 1 && !bare.starts_with("0x")) =>
                    {
                        bump("plain 1.1-only int (010, 1_000, 0b1, 10:30)")
                    }
                    STR if DATE_LIKE.is_match(&value) => bump("plain date/time (kept as str)"),
                    STR if CORE_NUMBER.is_match(&value) => {
                        bump("plain 1.2-only number (0o7, 0912, 1e3)")
                    }
                    _ => {}
                }
            }
            D::SequenceStart { anchor, tag, .. } | D::MappingStart { anchor, tag, .. } => {
                if anchor.is_some() {
                    bump("anchor &x");
                }
                if tag.is_some() {
                    bump("explicit tag");
                }
                stack.push(is_map.then(|| (HashSet::new(), true)));
            }
            D::SequenceEnd | D::MappingEnd => {
                stack.pop();
            }
            _ => {}
        }
    }
}
