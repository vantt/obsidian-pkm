//! Vài phép của Python mà `query.py`, `tools.py` và `Server.call` dùng trên giá trị JSON của người gọi: độ "đúng"
//! (`bool(x)`), `str()`, `repr()`, `json.dumps(..., sort_keys=True, ensure_ascii=False)`, `int()`, cắt danh sách
//! kiểu `xs[a:b]`, `str.strip()`.

use rusqlite::types::Value as SqlValue;
use serde_json::{Map, Value};

use crate::vault::yaml::repr::{bytes_repr, float_repr, str_repr};

/// `bool(x)` với giá trị đọc từ JSON.
pub(crate) fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|x| x != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// Tên kiểu Python của giá trị JSON (cho câu lỗi `TypeError`).
pub(crate) fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(n) if n.is_f64() => "float",
        Value::Number(_) => "int",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

fn number_repr(n: &serde_json::Number) -> String {
    match n.as_f64() {
        Some(x) if n.is_f64() => float_repr(x),
        _ => n.to_string(),
    }
}

/// `repr(x)`.
pub(crate) fn repr(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Number(n) => number_repr(n),
        Value::String(s) => str_repr(s),
        Value::Array(a) => format!("[{}]", a.iter().map(repr).collect::<Vec<_>>().join(", ")),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, v)| format!("{}: {}", str_repr(k), repr(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// `str(x)`.
pub(crate) fn str_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        _ => repr(v),
    }
}

/// Chuỗi JSON như `json.dumps(s, ensure_ascii=False)`.
fn json_str(s: &str, out: &mut String) {
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
            c => out.push(c),
        }
    }
    out.push('"');
}

/// `json.dumps(x, sort_keys=True, ensure_ascii=False)` (dấu phân cách mặc định `, ` và `: `).
pub(crate) fn json_dumps_sorted(v: &Value) -> String {
    let mut out = String::new();
    dump(v, &mut out);
    out
}

fn dump(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&number_repr(n)),
        Value::String(s) => json_str(s, out),
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                dump(x, out);
            }
            out.push(']');
        }
        Value::Object(o) => {
            // Khoá Python sắp theo điểm mã, trùng với thứ tự byte của UTF-8.
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                json_str(k, out);
                out.push_str(": ");
                dump(&o[k], out);
            }
            out.push('}');
        }
    }
}

/// `str.strip()`.
pub(crate) fn strip(s: &str) -> &str {
    s.trim_matches(crate::index::py_is_space)
}

/// Chỉ số của `xs[start:stop]` trên danh sách dài `len` (số âm tính từ cuối, như Python).
pub(crate) fn slice(len: usize, start: Option<i64>, stop: Option<i64>) -> std::ops::Range<usize> {
    let n = i64::try_from(len).unwrap_or(i64::MAX);
    let fix = |i: i64| -> usize {
        let i = if i < 0 { (i + n).max(0) } else { i.min(n) };
        usize::try_from(i).unwrap_or(0)
    };
    let a = start.map_or(0, fix);
    let b = stop.map_or(len, fix);
    a..b.max(a)
}

/// `xs[:stop]`.
pub(crate) fn head<T: Clone>(xs: &[T], stop: i64) -> Vec<T> {
    xs[slice(xs.len(), None, Some(stop))].to_vec()
}

/// Giá trị SQLite đọc ra thành JSON như `json.dumps(..., default=str)` của MCP server ghi: `bytes` thành `repr`.
pub(crate) fn sql_to_json(v: SqlValue) -> Value {
    match v {
        SqlValue::Null => Value::Null,
        SqlValue::Integer(i) => Value::from(i),
        SqlValue::Real(x) => serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number),
        SqlValue::Text(s) => Value::String(s),
        SqlValue::Blob(b) => Value::String(bytes_repr(&b)),
    }
}

/// Khoá của `dict` Python thành khoá JSON như `json.dumps` đổi (`None` thành `null`, số thành chữ).
pub(crate) fn sql_key(v: &SqlValue) -> String {
    match v {
        SqlValue::Null => "null".into(),
        SqlValue::Integer(i) => i.to_string(),
        SqlValue::Real(x) => float_repr(*x),
        SqlValue::Text(s) => s.clone(),
        SqlValue::Blob(b) => bytes_repr(b),
    }
}

/// Một object JSON từ các cặp (khoá, giá trị) theo đúng thứ tự (như `dict(zip(...))`).
pub(crate) fn object<I, K>(pairs: I) -> Map<String, Value>
where
    I: IntoIterator<Item = (K, Value)>,
    K: Into<String>,
{
    pairs.into_iter().map(|(k, v)| (k.into(), v)).collect()
}

/// `int(x)`: số nguyên, `bool`, số thực (cắt về 0) hoặc chuỗi số nguyên thập phân (bỏ khoảng trắng hai đầu, `_`
/// giữa hai chữ số). `Err((kiểu ngoại lệ, câu))` khi Python ném lỗi.
pub(crate) fn int(v: &Value) -> Result<i64, (&'static str, String)> {
    match v {
        Value::Bool(b) => Ok(i64::from(*b)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i)
            } else {
                let x = n.as_f64().unwrap_or(f64::NAN);
                if x.is_finite() {
                    // `int(float)` cắt phần lẻ; số ngoài khoảng i64 bị chặn (Python giữ số lớn).
                    Ok(x.trunc() as i64)
                } else {
                    Err((
                        "OverflowError",
                        "cannot convert float infinity to integer".into(),
                    ))
                }
            }
        }
        Value::String(s) => {
            let bad = || {
                (
                    "ValueError",
                    format!("invalid literal for int() with base 10: {}", str_repr(s)),
                )
            };
            let t = strip(s);
            let (neg, digits) = match t.as_bytes().first() {
                Some(b'-') => (true, &t[1..]),
                Some(b'+') => (false, &t[1..]),
                _ => (false, t),
            };
            let b = digits.as_bytes();
            if b.is_empty() || b[0] == b'_' || b[b.len() - 1] == b'_' || digits.contains("__") {
                return Err(bad());
            }
            let plain: String = digits.chars().filter(|c| *c != '_').collect();
            if !plain.bytes().all(|c| c.is_ascii_digit()) {
                return Err(bad());
            }
            let n: i64 = plain.parse().map_err(|_| bad())?;
            Ok(if neg { -n } else { n })
        }
        _ => Err((
            "TypeError",
            format!(
                "int() argument must be a string, a bytes-like object or a real number, not '{}'",
                type_name(v)
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dumps_like_python() {
        let v = json!(["find", ["Công ty", null, true, 1.0, 2, {"b": 1, "a": "x\n\"y\""}]]);
        assert_eq!(
            json_dumps_sorted(&v),
            r#"["find", ["Công ty", null, true, 1.0, 2, {"a": "x\n\"y\"", "b": 1}]]"#
        );
        assert_eq!(json_dumps_sorted(&json!(1e-7)), "1e-07");
    }

    #[test]
    fn str_and_repr_like_python() {
        assert_eq!(str_of(&json!(null)), "None");
        assert_eq!(str_of(&json!(true)), "True");
        assert_eq!(str_of(&json!(2.0)), "2.0");
        assert_eq!(str_of(&json!("a'b")), "a'b");
        assert_eq!(repr(&json!("a'b")), "\"a'b\"");
        assert_eq!(
            repr(&json!(["x", 1, {"k": false}])),
            "['x', 1, {'k': False}]"
        );
    }

    #[test]
    fn slices_like_python() {
        let xs = [1, 2, 3, 4, 5];
        assert_eq!(head(&xs, 2), [1, 2]);
        assert_eq!(head(&xs, -2), [1, 2, 3]);
        assert_eq!(head(&xs, -9), Vec::<i32>::new());
        assert_eq!(head(&xs, 9), xs);
        assert_eq!(&xs[slice(5, Some(3), Some(8))], &[4, 5]);
        assert_eq!(&xs[slice(5, Some(-2), Some(1))], &[] as &[i32]);
        assert_eq!(&xs[slice(5, Some(-2), Some(5))], &[4, 5]);
    }

    #[test]
    fn int_like_python() {
        assert_eq!(int(&json!(" 1_0\n")), Ok(10));
        assert_eq!(int(&json!("-3")), Ok(-3));
        assert_eq!(int(&json!(4.9)), Ok(4));
        assert_eq!(int(&json!(true)), Ok(1));
        assert_eq!(int(&json!("4.5")).unwrap_err().0, "ValueError");
        assert_eq!(int(&json!("1__0")).unwrap_err().0, "ValueError");
        assert_eq!(int(&json!([1])).unwrap_err().0, "TypeError");
    }

    #[test]
    fn truthiness_like_python() {
        for v in [
            json!(null),
            json!(false),
            json!(0),
            json!(0.0),
            json!(""),
            json!([]),
            json!({}),
        ] {
            assert!(!truthy(&v), "{v}");
        }
        for v in [
            json!(true),
            json!(-1),
            json!(0.5),
            json!(" "),
            json!([0]),
            json!({"a": 0}),
        ] {
            assert!(truthy(&v), "{v}");
        }
    }
}
