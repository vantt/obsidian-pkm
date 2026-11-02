//! Phần `str()` / `repr()` của Python mà `vault.flat` cần để đổi giá trị YAML thành `props`: số thực ngắn nhất,
//! chuỗi có thoát ký tự không in được, bytes, ngày giờ và múi giờ.

use std::sync::LazyLock;

use regex::Regex;

use super::{Date, DateTime, Tz};

/// `repr(float)`: chữ số ngắn nhất mà đọc lại ra đúng số, dạng mũ khi số mũ < -4 hoặc ≥ 16.
pub(crate) fn float_repr(x: f64) -> String {
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
    // `{:e}` của Rust in chữ số ngắn nhất đọc lại được, như `repr` của Python.
    let sci = format!("{:e}", x.abs());
    let (mant, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let digits: String = mant.chars().filter(char::is_ascii_digit).collect();
    let n = i32::try_from(digits.len()).unwrap_or(i32::MAX);
    let decpt = exp + 1;
    let body = if !(-4 < decpt && decpt <= 16) {
        let rest = if digits.len() > 1 {
            format!(".{}", &digits[1..])
        } else {
            String::new()
        };
        let esign = if exp < 0 { '-' } else { '+' };
        format!("{}{rest}e{esign}{:02}", &digits[..1], exp.unsigned_abs())
    } else if decpt <= 0 {
        format!("0.{}{digits}", "0".repeat(decpt.unsigned_abs() as usize))
    } else if decpt >= n {
        format!("{digits}{}.0", "0".repeat((decpt - n) as usize))
    } else {
        let at = decpt as usize;
        format!("{}.{}", &digits[..at], &digits[at..])
    };
    format!("{sign}{body}")
}

/// Ký tự `str.isprintable()` coi là không in được: nhóm Other (Cc, Cf, Cs, Co, Cn) và Separator (Zl, Zp, Zs),
/// trừ dấu cách.
static NOT_PRINTABLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[\p{Cc}\p{Cf}\p{Co}\p{Cn}\p{Zl}\p{Zp}\p{Zs}]$").expect("regex tĩnh hợp lệ")
});

fn printable(c: char) -> bool {
    c == ' ' || !NOT_PRINTABLE.is_match(c.encode_utf8(&mut [0; 4]))
}

fn quote_for(has_single: bool, has_double: bool) -> char {
    if has_single && !has_double { '"' } else { '\'' }
}

/// `repr(str)`.
pub(crate) fn str_repr(s: &str) -> String {
    let q = quote_for(s.contains('\''), s.contains('"'));
    let mut out = String::with_capacity(s.len() + 2);
    out.push(q);
    for c in s.chars() {
        let u = c as u32;
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if c == q => {
                out.push('\\');
                out.push(c);
            }
            _ if u < 0x20 || u == 0x7f => out.push_str(&format!("\\x{u:02x}")),
            _ if u < 0x7f || printable(c) => out.push(c),
            _ if u <= 0xff => out.push_str(&format!("\\x{u:02x}")),
            _ if u <= 0xffff => out.push_str(&format!("\\u{u:04x}")),
            _ => out.push_str(&format!("\\U{u:08x}")),
        }
    }
    out.push(q);
    out
}

/// `repr(s)[:n]`, như `%.200R` trong thông báo lỗi của `int()`.
pub(crate) fn str_repr_cut(s: &str, n: usize) -> String {
    str_repr(s).chars().take(n).collect()
}

/// `repr(bytes)`.
pub(crate) fn bytes_repr(b: &[u8]) -> String {
    let q = quote_for(b.contains(&b'\''), b.contains(&b'"'));
    let mut out = format!("b{q}");
    for &c in b {
        match c {
            b'\\' => out.push_str("\\\\"),
            b'\t' => out.push_str("\\t"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            c if char::from(c) == q => {
                out.push('\\');
                out.push(q);
            }
            0x20..=0x7e => out.push(char::from(c)),
            c => out.push_str(&format!("\\x{c:02x}")),
        }
    }
    out.push(q);
    out
}

/// `date.isoformat()`, cũng là `str(date)`.
pub(crate) fn date_iso(d: &Date) -> String {
    format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)
}

fn offset_iso(secs: i32) -> String {
    let sign = if secs < 0 { '-' } else { '+' };
    let a = secs.unsigned_abs();
    format!("{sign}{:02}:{:02}", a / 3600, a % 3600 / 60)
}

/// `datetime.isoformat(sep)`; `str(datetime)` dùng `sep=' '`.
pub(crate) fn datetime_iso(dt: &DateTime, sep: char) -> String {
    let mut s = format!(
        "{}{sep}{:02}:{:02}:{:02}",
        date_iso(&dt.date),
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
pub(crate) fn timedelta_repr(secs: i64) -> String {
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

pub(crate) fn date_repr(d: &Date) -> String {
    format!("datetime.date({}, {}, {})", d.year, d.month, d.day)
}

pub(crate) fn datetime_repr(dt: &DateTime) -> String {
    let d = &dt.date;
    let mut s = format!(
        "datetime.datetime({}, {}, {}, {}, {}",
        d.year, d.month, d.day, dt.hour, dt.minute
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
            timedelta_repr(i64::from(o))
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
            (f64::NAN, "nan"),
            (f64::NEG_INFINITY, "-inf"),
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
        assert_eq!(str_repr("it's \"q\""), "'it\\'s \"q\"'");
        assert_eq!(str_repr("Tiếng Việt"), "'Tiếng Việt'");
        assert_eq!(bytes_repr(b"\x00ab\xff'\""), r#"b'\x00ab\xff\'"'"#);
        assert_eq!(bytes_repr(b"'"), "b\"'\"");
        assert_eq!(str_repr_cut("xxxx", 3), "'xx");
    }

    #[test]
    fn datetime_repr_matches_python() {
        let dt = DateTime {
            date: Date {
                year: 2026,
                month: 1,
                day: 5,
            },
            hour: 10,
            minute: 30,
            second: 0,
            micro: 500_000,
            tz: Some(Tz::Offset(25_200)),
        };
        assert_eq!(
            datetime_repr(&dt),
            "datetime.datetime(2026, 1, 5, 10, 30, 0, 500000, tzinfo=datetime.timezone(datetime.timedelta(seconds=25200)))"
        );
        assert_eq!(datetime_iso(&dt, 'T'), "2026-01-05T10:30:00.500000+07:00");
        assert_eq!(
            timedelta_repr(-5 * 3600),
            "datetime.timedelta(days=-1, seconds=68400)"
        );
        let neg = DateTime {
            tz: Some(Tz::Offset(-(5 * 3600 + 1800))),
            micro: 0,
            ..dt
        };
        assert_eq!(datetime_iso(&neg, ' '), "2026-01-05 10:30:00-05:30");
        assert_eq!(
            datetime_repr(&DateTime {
                second: 7,
                tz: Some(Tz::Utc),
                ..neg
            }),
            "datetime.datetime(2026, 1, 5, 10, 30, 7, tzinfo=datetime.timezone.utc)"
        );
    }
}
