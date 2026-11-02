//! Dựng scalar như `SafeConstructor` của PyYAML 6.0.3: `int`, `float`, `binary`, `timestamp`, kèm đúng câu lỗi
//! của Python 3.14 (`int()`, `float()`, `binascii`, `datetime`).

use std::sync::LazyLock;

use regex::Regex;

use super::repr::{str_repr, str_repr_cut, timedelta_repr};
use super::{Date, DateTime, Fail, Tz};
use crate::vault::pystr::strip;

/// Giới hạn số chữ số khi đổi giữa số nguyên và chuỗi ở cơ số không phải luỹ thừa của 2 (`sys.int_info`).
const MAX_STR_DIGITS: usize = 4300;

fn err<T>(msg: impl Into<String>) -> Result<T, Fail> {
    Err(Fail::Error(msg.into()))
}

/// Số tự nhiên không giới hạn độ lớn (Python `int`), lưu theo khối 10^9, khối thấp trước.
#[derive(Default)]
struct Big(Vec<u32>);

const LIMB: u64 = 1_000_000_000;

impl Big {
    fn mul_add(&mut self, mul: u32, add: u32) {
        let mut carry = u64::from(add);
        for l in &mut self.0 {
            let v = u64::from(*l) * u64::from(mul) + carry;
            *l = (v % LIMB) as u32;
            carry = v / LIMB;
        }
        while carry > 0 {
            self.0.push((carry % LIMB) as u32);
            carry /= LIMB;
        }
    }

    fn add(&mut self, other: &Big) {
        let mut carry = 0u64;
        for i in 0..self.0.len().max(other.0.len()) {
            let v = u64::from(self.0.get(i).copied().unwrap_or(0))
                + u64::from(other.0.get(i).copied().unwrap_or(0))
                + carry;
            if i < self.0.len() {
                self.0[i] = (v % LIMB) as u32;
            } else {
                self.0.push((v % LIMB) as u32);
            }
            carry = v / LIMB;
        }
        if carry > 0 {
            self.0.push(carry as u32);
        }
    }

    fn from_digits(digits: &str, base: u32) -> Big {
        let mut b = Big::default();
        for c in digits.chars() {
            b.mul_add(base, c.to_digit(base).unwrap_or(0));
        }
        b
    }

    fn decimal(&self) -> String {
        let mut it = self.0.iter().rev().skip_while(|&&l| l == 0);
        let Some(first) = it.next() else {
            return "0".into();
        };
        let mut s = first.to_string();
        for l in it {
            s.push_str(&format!("{l:09}"));
        }
        s
    }
}

/// Python `int(text, base)` với những chuỗi `SafeConstructor` đưa vào (dấu `_` đã bỏ). Trả chuỗi thập phân.
fn py_int(text: &str, base: u32) -> Result<String, Fail> {
    let bad = || {
        err(format!(
            "invalid literal for int() with base {base}: {}",
            str_repr_cut(text, 200)
        ))
    };
    let t = strip(text);
    let (neg, t) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let prefix = match base {
        2 => Some("0b"),
        8 => Some("0o"),
        16 => Some("0x"),
        _ => None,
    };
    let t = match prefix {
        Some(p) if t.len() >= 2 && t[..2].eq_ignore_ascii_case(p) => &t[2..],
        _ => t,
    };
    let leading = t.chars().take_while(|c| c.is_digit(base)).count();
    if !base.is_power_of_two() && leading > MAX_STR_DIGITS {
        return err(format!(
            "Exceeds the limit ({MAX_STR_DIGITS} digits) for integer string conversion: value has {leading} digits; \
             use sys.set_int_max_str_digits() to increase the limit"
        ));
    }
    if t.is_empty() || leading != t.chars().count() {
        return bad();
    }
    let mag = Big::from_digits(t, base).decimal();
    Ok(if neg && mag != "0" {
        format!("-{mag}")
    } else {
        mag
    })
}

/// `construct_yaml_int`. Số nguyên giữ dạng chuỗi thập phân.
pub(super) fn construct_int(raw: &str) -> Result<String, Fail> {
    let value = raw.replace('_', "");
    let Some(&first) = value.as_bytes().first() else {
        return Err(Fail::Crash("IndexError"));
    };
    let neg = first == b'-';
    let v = if first == b'-' || first == b'+' {
        &value[1..]
    } else {
        &value[..]
    };
    let mag = if v.is_empty() {
        return Err(Fail::Crash("IndexError"));
    } else if v == "0" {
        return Ok("0".into());
    } else if let Some(b) = v.strip_prefix("0b") {
        py_int(b, 2)?
    } else if let Some(h) = v.strip_prefix("0x") {
        py_int(h, 16)?
    } else if v.starts_with('0') {
        py_int(v, 8)?
    } else if v.contains(':') {
        sexagesimal(v)?
    } else {
        py_int(v, 10)?
    };
    Ok(match (neg, mag.strip_prefix('-')) {
        (false, _) => mag,
        (true, Some(pos)) => pos.to_string(),
        (true, None) if mag == "0" => mag,
        (true, None) => format!("-{mag}"),
    })
}

/// Số cơ số 60 (`1:30` = 90): mỗi phần qua `int(part)`, cộng từ phần lớn nhất. Phần âm (`!!int 1:-5`) thì
/// tính bằng `i128`; vượt `i128` thì báo lỗi thay vì tính sai.
fn sexagesimal(v: &str) -> Result<String, Fail> {
    let parts = v
        .split(':')
        .map(|p| py_int(p, 10))
        .collect::<Result<Vec<_>, _>>()?;
    if parts.iter().all(|p| !p.starts_with('-')) {
        let mut total = Big::default();
        for p in &parts {
            total.mul_add(60, 0);
            total.add(&Big::from_digits(p, 10));
        }
        return Ok(total.decimal());
    }
    let mut total: i128 = 0;
    for p in &parts {
        total = p
            .parse::<i128>()
            .ok()
            .and_then(|d| total.checked_mul(60)?.checked_add(d))
            .ok_or_else(|| Fail::Error(format!("số nguyên cơ số 60 quá lớn: {}", str_repr(v))))?;
    }
    Ok(total.to_string())
}

/// Python `float(text)`.
fn py_float(text: &str) -> Result<f64, Fail> {
    let t = strip(text);
    let lower = t.to_ascii_lowercase();
    let body = lower.trim_start_matches(['+', '-']);
    let word = matches!(body, "inf" | "infinity" | "nan");
    let number = body.len() + 1 >= lower.len()
        && body.bytes().any(|c| c.is_ascii_digit())
        && body
            .bytes()
            .all(|c| c.is_ascii_digit() || matches!(c, b'.' | b'e' | b'+' | b'-'));
    match (word || number).then(|| t.parse::<f64>()) {
        Some(Ok(f)) => Ok(f),
        _ => err(format!(
            "could not convert string to float: {}",
            str_repr(text)
        )),
    }
}

/// `construct_yaml_float`. `Ok(None)` là `.nan`: hằng NaN dùng chung của `SafeConstructor` (cùng một object).
pub(super) fn construct_float(raw: &str) -> Result<Option<f64>, Fail> {
    let value = raw.replace('_', "").to_lowercase();
    let Some(&first) = value.as_bytes().first() else {
        return Err(Fail::Crash("IndexError"));
    };
    let sign = if first == b'-' { -1.0 } else { 1.0 };
    let v = if first == b'-' || first == b'+' {
        &value[1..]
    } else {
        &value[..]
    };
    Ok(Some(if v == ".inf" {
        sign * f64::INFINITY
    } else if v == ".nan" {
        return Ok(None);
    } else if v.contains(':') {
        // Cộng từ phần nhỏ nhất như PyYAML để làm tròn giống.
        let parts = v.split(':').map(py_float).collect::<Result<Vec<_>, _>>()?;
        let (mut total, mut base) = (0.0, 1.0);
        for d in parts.iter().rev() {
            total += d * base;
            base *= 60.0;
        }
        sign * total
    } else {
        sign * py_float(v)?
    }))
}

/// `construct_yaml_binary`: `str.encode('ascii')` rồi `base64.decodebytes` (`binascii.a2b_base64`, không chặt).
pub(super) fn construct_binary(raw: &str) -> Result<Vec<u8>, Fail> {
    if let Some(start) = raw.chars().position(|c| !c.is_ascii()) {
        let run = raw
            .chars()
            .skip(start)
            .take_while(|c| !c.is_ascii())
            .count();
        let what = if run == 1 {
            let c = raw.chars().nth(start).map_or(0, |c| c as u32);
            let shown = if c <= 0xff {
                format!("\\x{c:02x}")
            } else if c <= 0xffff {
                format!("\\u{c:04x}")
            } else {
                format!("\\U{c:08x}")
            };
            format!("character '{shown}' in position {start}")
        } else {
            format!("characters in position {start}-{}", start + run - 1)
        };
        return err(format!(
            "failed to convert base64 data into ascii: 'ascii' codec can't encode {what}: ordinal not in range(128)"
        ));
    }
    let mut out = Vec::new();
    let (mut quad, mut left, mut pads) = (0u8, 0u8, 0u8);
    for c in raw.bytes() {
        if c == b'=' {
            if quad >= 2 {
                pads += 1;
                if quad + pads >= 4 {
                    return Ok(out);
                }
            }
            continue;
        }
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        };
        pads = 0;
        match quad {
            0 => {
                quad = 1;
                left = v;
            }
            1 => {
                quad = 2;
                out.push((left << 2) | (v >> 4));
                left = v & 0x0f;
            }
            2 => {
                quad = 3;
                out.push((left << 4) | (v >> 2));
                left = v & 0x03;
            }
            _ => {
                quad = 0;
                out.push((left << 6) | v);
                left = 0;
            }
        }
    }
    match quad {
        0 => Ok(out),
        1 => err(format!(
            "failed to decode base64 data: Invalid base64-encoded string: number of data characters ({}) \
             cannot be 1 more than a multiple of 4",
            out.len() / 3 * 4 + 1
        )),
        _ => err("failed to decode base64 data: Incorrect padding"),
    }
}

/// Regex của constructor `timestamp` (không phải của resolver). `$` của Python khớp cả trước `\n` cuối.
static TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\A(?P<year>[0-9][0-9][0-9][0-9])-(?P<month>[0-9][0-9]?)-(?P<day>[0-9][0-9]?)(?:(?:[Tt]|[ \t]+)(?P<hour>[0-9][0-9]?):(?P<minute>[0-9][0-9]):(?P<second>[0-9][0-9])(?:\.(?P<fraction>[0-9]*))?(?:[ \t]*(?P<tz>Z|(?P<tz_sign>[-+])(?P<tz_hour>[0-9][0-9]?)(?::(?P<tz_minute>[0-9][0-9]))?))?)?\n?\z",
    )
    .expect("regex tĩnh hợp lệ")
});

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Kiểm như `datetime.date(y, m, d)`, cùng câu lỗi.
fn check_date(y: i32, m: u32, d: u32) -> Result<Date, Fail> {
    if !(1..=9999).contains(&y) {
        return err(format!("year must be in 1..9999, not {y}"));
    }
    if !(1..=12).contains(&m) {
        return err(format!("month must be in 1..12, not {m}"));
    }
    let dim = days_in_month(y, m);
    if !(1..=dim).contains(&d) {
        return err(format!(
            "day {d} must be in range 1..{dim} for month {m} in year {y}"
        ));
    }
    Ok(Date {
        year: y,
        month: m,
        day: d,
    })
}

pub(super) enum Stamp {
    Date(Date),
    DateTime(DateTime),
}

/// `construct_yaml_timestamp`. Chuỗi không đúng dạng: Python gọi `.groupdict()` trên `None` (`AttributeError`).
pub(super) fn construct_timestamp(raw: &str) -> Result<Stamp, Fail> {
    let Some(c) = TIMESTAMP.captures(raw) else {
        return Err(Fail::Crash("AttributeError"));
    };
    // Nhóm chỉ có chữ số ASCII, tối đa bốn chữ số: luôn đọc được.
    let num = |n: &str| -> u32 { c.name(n).map_or(0, |m| m.as_str().parse().unwrap_or(0)) };
    let year = i32::try_from(num("year")).unwrap_or(0);
    let (month, day) = (num("month"), num("day"));
    if c.name("hour").is_none() {
        return check_date(year, month, day).map(Stamp::Date);
    }
    let micro = c
        .name("fraction")
        .map(|f| f.as_str())
        .filter(|f| !f.is_empty())
        .map_or(0, |f| {
            format!("{:0<6}", &f[..f.len().min(6)]).parse().unwrap_or(0)
        });
    let tz = if let Some(sign) = c.name("tz_sign") {
        let secs = i64::from(num("tz_hour")) * 3600 + i64::from(num("tz_minute")) * 60;
        let secs = if sign.as_str() == "-" { -secs } else { secs };
        if secs.abs() >= 86_400 {
            return err(format!(
                "offset must be a timedelta strictly between -timedelta(hours=24) and timedelta(hours=24), not {}",
                timedelta_repr(secs)
            ));
        }
        // `timezone(timedelta(0))` là chính `timezone.utc`.
        Some(match i32::try_from(secs) {
            Ok(0) => Tz::Utc,
            Ok(s) => Tz::Offset(s),
            Err(_) => Tz::Utc,
        })
    } else {
        c.name("tz").map(|_| Tz::Utc)
    };
    let date = check_date(year, month, day)?;
    let (hour, minute, second) = (num("hour"), num("minute"), num("second"));
    for (name, v, hi) in [
        ("hour", hour, 23),
        ("minute", minute, 59),
        ("second", second, 59),
    ] {
        if v > hi {
            return err(format!("{name} must be in 0..{hi}, not {v}"));
        }
    }
    Ok(Stamp::DateTime(DateTime {
        date,
        hour,
        minute,
        second,
        micro,
        tz,
    }))
}

/// `str(int)` của Python: số quá 4300 chữ số thì `ValueError` (ngoài khối `try` của `vault.parse`).
pub(super) fn int_str_ok(decimal: &str) -> bool {
    decimal.trim_start_matches('-').len() <= MAX_STR_DIGITS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_matches_python() {
        let int = |s: &str| construct_int(s).expect(s);
        assert_eq!(int("010"), "8");
        assert_eq!(int("-0"), "0");
        assert_eq!(int("-0b1_0"), "-2");
        assert_eq!(int("190:20:30"), "685230");
        assert_eq!(int("0x1F"), "31");
        assert_eq!(int("0o17"), "15");
        assert_eq!(int("1:-5"), "55");
        assert_eq!(
            int("123456789012345678901234567890"),
            "123456789012345678901234567890"
        );
        assert_eq!(
            int(&format!("0x{}", "f".repeat(40))),
            "1461501637330902918203684832716283019655932542975"
        );
        assert_eq!(
            construct_int("0x_").err(),
            Some(Fail::Error(
                "invalid literal for int() with base 16: ''".into()
            ))
        );
        assert_eq!(construct_int("").err(), Some(Fail::Crash("IndexError")));
        assert_eq!(construct_int("-").err(), Some(Fail::Crash("IndexError")));
        let long = construct_int(&"1".repeat(5000)).unwrap_err();
        assert_eq!(
            long,
            Fail::Error("Exceeds the limit (4300 digits) for integer string conversion: value has 5000 digits; use sys.set_int_max_str_digits() to increase the limit".into())
        );
        let Fail::Error(m) = construct_int(&format!("{}x", "9".repeat(300))).unwrap_err() else {
            panic!("int() lỗi là ValueError");
        };
        assert_eq!(m.chars().count(), 240);
    }

    #[test]
    fn float_matches_python() {
        assert_eq!(construct_float("1_000.5").unwrap(), Some(1000.5));
        assert_eq!(construct_float("-.inf").unwrap(), Some(f64::NEG_INFINITY));
        assert_eq!(construct_float(".NaN").unwrap(), None);
        assert_eq!(construct_float("1:30.5").unwrap(), Some(90.5));
        assert_eq!(
            construct_float("1E5E").unwrap_err(),
            Fail::Error("could not convert string to float: '1e5e'".into())
        );
        assert_eq!(construct_float("").unwrap_err(), Fail::Crash("IndexError"));
        assert_eq!(
            construct_float("-").unwrap_err(),
            Fail::Error("could not convert string to float: ''".into())
        );
    }

    #[test]
    fn binary_matches_binascii() {
        assert_eq!(construct_binary("aGVsbG8=\n").unwrap(), b"hello");
        let msg = |s: &str| match construct_binary(s).unwrap_err() {
            Fail::Error(m) => m,
            Fail::Crash(c) => c.into(),
        };
        assert_eq!(
            msg("abcde"),
            "failed to decode base64 data: Invalid base64-encoded string: number of data characters (5) cannot be 1 more than a multiple of 4"
        );
        for s in ["ab", "ab=", "a=bc"] {
            assert_eq!(msg(s), "failed to decode base64 data: Incorrect padding");
        }
        assert_eq!(
            msg("é"),
            "failed to convert base64 data into ascii: 'ascii' codec can't encode character '\\xe9' in position 0: ordinal not in range(128)"
        );
        assert_eq!(
            msg("aéé"),
            "failed to convert base64 data into ascii: 'ascii' codec can't encode characters in position 1-2: ordinal not in range(128)"
        );
    }

    #[test]
    fn timestamp_matches_python() {
        assert!(matches!(
            construct_timestamp("2026-01-05\n"),
            Ok(Stamp::Date(Date {
                year: 2026,
                month: 1,
                day: 5
            }))
        ));
        assert!(matches!(
            construct_timestamp("soon"),
            Err(Fail::Crash("AttributeError"))
        ));
        let e = |s: &str| match construct_timestamp(s) {
            Err(Fail::Error(m)) => m,
            _ => panic!("{s}"),
        };
        assert_eq!(
            e("2026-02-30"),
            "day 30 must be in range 1..28 for month 2 in year 2026"
        );
        assert_eq!(e("0000-01-01"), "year must be in 1..9999, not 0");
        assert_eq!(e("2026-01-01 24:00:00"), "hour must be in 0..23, not 24");
        assert_eq!(
            e("2026-01-01 10:00:00 +25:00"),
            "offset must be a timedelta strictly between -timedelta(hours=24) and timedelta(hours=24), not datetime.timedelta(days=1, seconds=3600)"
        );
        let Ok(Stamp::DateTime(dt)) = construct_timestamp("2026-01-05 10:30:00.5 -00:00") else {
            panic!("datetime");
        };
        assert_eq!((dt.micro, dt.tz), (500_000, Some(Tz::Utc)));
    }
}
