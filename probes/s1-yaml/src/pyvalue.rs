//! Python object model for what `yaml.load(..., Loader=CSafeLoader)` returns, plus the parts of
//! Python's `str()` / `repr()` that the spec's `vault.flat()` relies on to turn values into props.

use serde_json::{Value, json};
use std::sync::LazyLock;

/// Offset of a `datetime.timezone`, in seconds east of UTC. `Utc` is the singleton
/// `datetime.timezone.utc` (from `Z`, or any zero offset).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tz {
    Utc,
    Offset(i64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DateTime {
    pub year: i64,
    pub month: i64,
    pub day: i64,
    pub hour: i64,
    pub minute: i64,
    pub second: i64,
    pub micro: i64,
    pub tz: Option<Tz>,
}

#[derive(Clone, Debug)]
pub enum Py {
    None,
    Bool(bool),
    /// Canonical decimal text (Python ints are unbounded; we never need arithmetic on them).
    Int(String),
    Float(f64),
    Str(String),
    Bytes(Vec<u8>),
    Date(i64, i64, i64),
    DateTime(DateTime),
    List(Vec<Py>),
    Tuple(Vec<Py>),
    Set(Vec<Py>),
    /// Insertion-ordered dict.
    Dict(Vec<(Py, Py)>),
}

impl Py {
    /// Python truthiness (`yaml.load(...) or {}`).
    pub fn truthy(&self) -> bool {
        match self {
            Py::None => false,
            Py::Bool(b) => *b,
            Py::Int(s) => s != "0",
            Py::Float(f) => *f != 0.0,
            Py::Str(s) => !s.is_empty(),
            Py::Bytes(b) => !b.is_empty(),
            Py::Date(..) | Py::DateTime(_) => true,
            Py::List(v) | Py::Tuple(v) | Py::Set(v) => !v.is_empty(),
            Py::Dict(v) => !v.is_empty(),
        }
    }

    /// `isinstance(x, collections.abc.Hashable)` as SafeConstructor checks it for mapping keys.
    pub fn hashable(&self) -> bool {
        !matches!(self, Py::List(_) | Py::Set(_) | Py::Dict(_))
    }

    /// Numeric value for cross-type key equality (`1 == 1.0 == True`).
    fn number(&self) -> Option<f64> {
        match self {
            Py::Bool(b) => Some(f64::from(u8::from(*b))),
            Py::Int(s) => s.parse::<f64>().ok(),
            Py::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Python `==` as used by dict keys.
    pub fn key_eq(&self, other: &Py) -> bool {
        match (self, other) {
            (Py::Int(a), Py::Int(b)) => a == b,
            (Py::Float(a), Py::Float(b)) if a.is_nan() && b.is_nan() => true, // one shared nan object
            (Py::Bool(_) | Py::Int(_) | Py::Float(_), Py::Bool(_) | Py::Int(_) | Py::Float(_)) => {
                self.number() == other.number()
            }
            (Py::None, Py::None) => true,
            (Py::Str(a), Py::Str(b)) => a == b,
            (Py::Bytes(a), Py::Bytes(b)) => a == b,
            (Py::Date(a, b, c), Py::Date(x, y, z)) => (a, b, c) == (x, y, z),
            (Py::DateTime(a), Py::DateTime(b)) => a == b,
            (Py::Tuple(a), Py::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.key_eq(y))
            }
            _ => false,
        }
    }

    /// Typed JSON, same shape as `pyyaml-dump.py`'s `enc()`.
    pub fn typed(&self) -> Value {
        match self {
            Py::None => json!(["null"]),
            Py::Bool(b) => json!(["bool", b]),
            Py::Int(s) => json!(["int", s]),
            Py::Float(f) => json!(["float", float_repr(*f)]),
            Py::Str(s) => json!(["str", s]),
            Py::Bytes(b) => json!(["bytes", bytes_repr(b)]),
            Py::Date(y, m, d) => json!(["date", date_iso(*y, *m, *d)]),
            Py::DateTime(dt) => json!(["datetime", datetime_iso(dt, 'T')]),
            Py::List(v) => json!(["list", v.iter().map(Py::typed).collect::<Vec<_>>()]),
            Py::Tuple(v) => json!(["tuple", v.iter().map(Py::typed).collect::<Vec<_>>()]),
            Py::Set(v) => {
                let mut items: Vec<Value> = v.iter().map(Py::typed).collect();
                items.sort_by_cached_key(|e| e.to_string());
                json!(["set", items])
            }
            Py::Dict(v) => json!([
                "map",
                v.iter()
                    .map(|(k, x)| json!([k.typed(), x.typed()]))
                    .collect::<Vec<_>>()
            ]),
        }
    }

    /// Python `str(x)`.
    pub fn py_str(&self) -> String {
        match self {
            Py::Str(s) => s.clone(),
            Py::Date(y, m, d) => date_iso(*y, *m, *d),
            Py::DateTime(dt) => datetime_iso(dt, ' '),
            _ => self.py_repr(),
        }
    }

    /// Python `repr(x)`.
    pub fn py_repr(&self) -> String {
        let join = |v: &[Py]| v.iter().map(Py::py_repr).collect::<Vec<_>>().join(", ");
        match self {
            Py::None => "None".into(),
            Py::Bool(true) => "True".into(),
            Py::Bool(false) => "False".into(),
            Py::Int(s) => s.clone(),
            Py::Float(f) => float_repr(*f),
            Py::Str(s) => str_repr(s),
            Py::Bytes(b) => bytes_repr(b),
            Py::Date(y, m, d) => format!("datetime.date({y}, {m}, {d})"),
            Py::DateTime(dt) => datetime_repr(dt),
            Py::List(v) => format!("[{}]", join(v)),
            Py::Tuple(v) if v.len() == 1 => format!("({},)", v[0].py_repr()),
            Py::Tuple(v) => format!("({})", join(v)),
            // Python iterates a set in hash order, which for str changes per process; any order is "right".
            Py::Set(v) if v.is_empty() => "set()".into(),
            Py::Set(v) => format!("{{{}}}", join(v)),
            Py::Dict(v) => format!(
                "{{{}}}",
                v.iter()
                    .map(|(k, x)| format!("{}: {}", k.py_repr(), x.py_repr()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

/// `vault.flat(v)`: one prop value per list leaf; `None` stays `None`.
pub fn flat(v: &Py, out: &mut Vec<Option<String>>) {
    match v {
        Py::None => out.push(None),
        Py::Bool(b) => out.push(Some(if *b { "true" } else { "false" }.into())),
        Py::Date(y, m, d) => out.push(Some(date_iso(*y, *m, *d))),
        Py::DateTime(dt) => out.push(Some(datetime_iso(dt, 'T'))),
        Py::List(items) => items.iter().for_each(|x| flat(x, out)),
        _ => out.push(Some(v.py_str())),
    }
}

/// `vault.parse()`'s props loop: top-level keys, nested dicts one level deep as `parent.child`.
pub fn props(fm: &[(Py, Py)]) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    for (k, v) in fm {
        let k = k.py_str();
        if let Py::Dict(inner) = v {
            for (k2, v2) in inner {
                let key = format!("{k}.{}", k2.py_str());
                let mut vals = Vec::new();
                flat(v2, &mut vals);
                out.extend(vals.into_iter().map(|x| (key.clone(), x)));
            }
            continue;
        }
        let mut vals = Vec::new();
        flat(v, &mut vals);
        out.extend(vals.into_iter().map(|x| (k.clone(), x)));
    }
    out
}

/// Python `repr(float)`: shortest round-trip digits, scientific when the exponent is < -4 or >= 16.
pub fn float_repr(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf" } else { "-inf" }.into();
    }
    let sign = if x.is_sign_negative() { "-" } else { "" };
    if x == 0.0 {
        return format!("{sign}0.0");
    }
    let sci = format!("{:e}", x.abs()); // e.g. "1.2345e-5": Rust prints the shortest round-trip digits
    let (mant, exp) = sci
        .split_once('e')
        .expect("LowerExp always has an exponent");
    let exp: i32 = exp.parse().expect("LowerExp exponent is an integer");
    let digits: String = mant.chars().filter(char::is_ascii_digit).collect();
    let n = i32::try_from(digits.len()).expect("f64 has at most 17 significant digits");
    let decpt = exp + 1;
    let body = if !(-4 < decpt && decpt <= 16) {
        let rest = if digits.len() > 1 {
            format!(".{}", &digits[1..])
        } else {
            String::new()
        };
        let esign = if exp < 0 { '-' } else { '+' };
        format!("{}{rest}e{esign}{:02}", &digits[..1], exp.abs())
    } else if decpt <= 0 {
        format!("0.{}{digits}", "0".repeat((-decpt) as usize))
    } else if decpt >= n {
        format!("{digits}{}.0", "0".repeat((decpt - n) as usize))
    } else {
        let at = decpt as usize;
        format!("{}.{}", &digits[..at], &digits[at..])
    };
    format!("{sign}{body}")
}

static NOT_PRINTABLE: LazyLock<regex::Regex> = LazyLock::new(|| {
    // Python's str.isprintable(): false for Other (Cc Cf Cs Co Cn) and Separator (Zl Zp Zs) except ' '.
    regex::Regex::new(r"^[\p{Cc}\p{Cf}\p{Co}\p{Cn}\p{Zl}\p{Zp}\p{Zs}]$")
        .expect("static regex compiles")
});

fn printable(c: char) -> bool {
    c == ' ' || !NOT_PRINTABLE.is_match(c.encode_utf8(&mut [0; 4]))
}

fn quote_for(has_single: bool, has_double: bool) -> char {
    if has_single && !has_double { '"' } else { '\'' }
}

/// Python `repr(str)`.
pub fn str_repr(s: &str) -> String {
    let q = quote_for(s.contains('\''), s.contains('"'));
    let mut out = String::with_capacity(s.len() + 2);
    out.push(q);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if c == q => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\x{:02x}", c as u32))
            }
            c if (c as u32) < 0x7f || printable(c) => out.push(c),
            c if (c as u32) <= 0xff => out.push_str(&format!("\\x{:02x}", c as u32)),
            c if (c as u32) <= 0xffff => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push_str(&format!("\\U{:08x}", c as u32)),
        }
    }
    out.push(q);
    out
}

/// Python `repr(bytes)`.
pub fn bytes_repr(b: &[u8]) -> String {
    let q = quote_for(b.contains(&b'\''), b.contains(&b'"'));
    let mut out = format!("b{q}");
    for &c in b {
        match c {
            b'\\' => out.push_str("\\\\"),
            b'\t' => out.push_str("\\t"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            c if c as char == q => {
                out.push('\\');
                out.push(q);
            }
            0x20..=0x7e => out.push(c as char),
            c => out.push_str(&format!("\\x{c:02x}")),
        }
    }
    out.push(q);
    out
}

pub fn date_iso(y: i64, m: i64, d: i64) -> String {
    format!("{y:04}-{m:02}-{d:02}")
}

fn offset_iso(secs: i64) -> String {
    let sign = if secs < 0 { '-' } else { '+' };
    let a = secs.abs();
    format!("{sign}{:02}:{:02}", a / 3600, a % 3600 / 60)
}

/// `datetime.isoformat(sep)`; `str(datetime)` uses `sep=' '`.
pub fn datetime_iso(dt: &DateTime, sep: char) -> String {
    let mut s = format!(
        "{}{sep}{:02}:{:02}:{:02}",
        date_iso(dt.year, dt.month, dt.day),
        dt.hour,
        dt.minute,
        dt.second
    );
    if dt.micro != 0 {
        s.push_str(&format!(".{:06}", dt.micro));
    }
    match dt.tz {
        Some(Tz::Utc) => s.push_str("+00:00"),
        Some(Tz::Offset(o)) => s.push_str(&offset_iso(o)),
        None => {}
    }
    s
}

/// `repr(datetime.timedelta(seconds=secs))`.
pub fn timedelta_repr(secs: i64) -> String {
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let mut parts = Vec::new();
    if days != 0 {
        parts.push(format!("days={days}"));
    }
    if rem != 0 {
        parts.push(format!("seconds={rem}"));
    }
    if parts.is_empty() {
        "datetime.timedelta(0)".into()
    } else {
        format!("datetime.timedelta({})", parts.join(", "))
    }
}

fn datetime_repr(dt: &DateTime) -> String {
    let mut s = format!(
        "datetime.datetime({}, {}, {}, {}, {}",
        dt.year, dt.month, dt.day, dt.hour, dt.minute
    );
    if dt.micro != 0 {
        s.push_str(&format!(", {}, {}", dt.second, dt.micro));
    } else if dt.second != 0 {
        s.push_str(&format!(", {}", dt.second));
    }
    match dt.tz {
        Some(Tz::Utc) => s.push_str(", tzinfo=datetime.timezone.utc"),
        Some(Tz::Offset(o)) => s.push_str(&format!(
            ", tzinfo=datetime.timezone({})",
            timedelta_repr(o)
        )),
        None => {}
    }
    s.push(')');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_repr_matches_python() {
        for (x, want) in [
            (1e16, "1e+16"),
            (1e15, "1000000000000000.0"),
            (123456789012345678.0, "1.2345678901234568e+17"),
            (0.0001, "0.0001"),
            (0.00001, "1e-05"),
            (1e22, "1e+22"),
            (5e-324, "5e-324"),
            (1.5e300, "1.5e+300"),
            (1000.0, "1000.0"),
            (1.1, "1.1"),
            (-0.0, "-0.0"),
            (685230.15, "685230.15"),
        ] {
            assert_eq!(float_repr(x), want, "{x}");
        }
    }

    #[test]
    fn str_repr_matches_python() {
        let s = "a\u{200b}b\x01\u{a0}\u{ad}\u{e0001}\u{e000}\u{378}\x7f\u{85}";
        assert_eq!(
            str_repr(s),
            "'a\\u200bb\\x01\\xa0\\xad\\U000e0001\\ue000\\u0378\\x7f\\x85'"
        );
        assert_eq!(str_repr("it's"), "\"it's\"");
        assert_eq!(str_repr("Tiếng Việt"), "'Tiếng Việt'");
        assert_eq!(bytes_repr(b"\x00ab\xff'\""), r#"b'\x00ab\xff\'"'"#);
        assert_eq!(bytes_repr(b"'"), "b\"'\"");
    }

    #[test]
    fn datetime_repr_matches_python() {
        let dt = DateTime {
            year: 2026,
            month: 1,
            day: 5,
            hour: 10,
            minute: 30,
            second: 0,
            micro: 500_000,
            tz: Some(Tz::Offset(25_200)),
        };
        assert_eq!(
            Py::DateTime(dt).py_repr(),
            "datetime.datetime(2026, 1, 5, 10, 30, 0, 500000, tzinfo=datetime.timezone(datetime.timedelta(seconds=25200)))"
        );
        assert_eq!(
            timedelta_repr(-5 * 3600),
            "datetime.timedelta(days=-1, seconds=68400)"
        );
    }
}
