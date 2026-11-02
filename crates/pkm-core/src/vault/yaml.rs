//! Đọc YAML như `yaml.load(..., Loader=CSafeLoader)` của PyYAML 6.0.3 (docs/DECISIONS.md, Q3): parser là libyaml
//! 0.2.5 ([`libyaml`]), trên đó là composer như `CParser`, resolver YAML 1.1 của PyYAML và constructor như
//! `SafeConstructor`. Frontmatter đọc bằng [`Loader::Frontmatter`] (bỏ resolver `timestamp` như `vault.LOADER`),
//! `schema.yaml` bằng [`Loader::Safe`].
//!
//! Kết quả là một [`Doc`]: mọi object Python nằm trong một mảng, container trỏ tới phần tử bằng chỉ số, nên alias
//! dùng chung object và container tự chứa mình (alias vòng) được giữ đúng như Python.
//!
//! Lỗi mang dòng đầu `str(e)` của ngoại lệ Python, phần `vault.parse` lưu lại. Ngoại lệ mà `vault.parse` không bắt
//! (bản Python dừng cả lần index) trả về [`Fail::Crash`] với tên kiểu ngoại lệ.

mod libyaml;
pub(crate) mod repr;
mod scalar;

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use libyaml::{Event, Parser};
use repr::{bytes_repr, date_iso, date_repr, datetime_iso, datetime_repr, float_repr, str_repr};
use scalar::{Stamp, construct_binary, construct_float, construct_int, construct_timestamp};

const NULL: &str = "tag:yaml.org,2002:null";
const BOOL: &str = "tag:yaml.org,2002:bool";
const INT: &str = "tag:yaml.org,2002:int";
const FLOAT: &str = "tag:yaml.org,2002:float";
const STR: &str = "tag:yaml.org,2002:str";
const SEQ: &str = "tag:yaml.org,2002:seq";
const MAP: &str = "tag:yaml.org,2002:map";
const MERGE: &str = "tag:yaml.org,2002:merge";
const VALUE: &str = "tag:yaml.org,2002:value";
const BINARY: &str = "tag:yaml.org,2002:binary";
const TIMESTAMP: &str = "tag:yaml.org,2002:timestamp";
const OMAP: &str = "tag:yaml.org,2002:omap";
const PAIRS: &str = "tag:yaml.org,2002:pairs";
const SET: &str = "tag:yaml.org,2002:set";

/// Độ sâu tối đa (lồng container, chuỗi `<<`, đệ quy khi in). Python dừng bằng `RecursionError` quanh mức
/// `sys.getrecursionlimit()` = 1000 (list lồng trong `vault.flat`); Rust báo cùng lỗi ở đúng mức này thay vì tràn
/// ngăn xếp.
pub(crate) const MAX_DEPTH: usize = 1000;

/// Lỗi khi đọc YAML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fail {
    /// `yaml.YAMLError` hoặc `ValueError`: dòng đầu thông báo, như `vault.parse` lưu.
    Error(String),
    /// Ngoại lệ khác (`KeyError`, `AttributeError`, `IndexError`, `RecursionError`, ...): tên kiểu ngoại lệ. Bản
    /// Python dừng cả lần index ở note này; Rust ghi lỗi cho riêng note (spec/PORTING.md, "Lệch có chủ ý").
    Crash(&'static str),
}

impl std::fmt::Display for Fail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fail::Error(m) => f.write_str(m),
            Fail::Crash(t) => f.write_str(t),
        }
    }
}

impl std::error::Error for Fail {}

fn err<T>(msg: impl Into<String>) -> Result<T, Fail> {
    Err(Fail::Error(msg.into()))
}

const RECURSION: Fail = Fail::Crash("RecursionError");

/// Múi giờ của `datetime.timezone`: `Utc` là `timezone.utc` (từ `Z` hoặc độ lệch 0), `Offset` tính bằng giây.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tz {
    Utc,
    Offset(i32),
}

impl Tz {
    fn seconds(self) -> i64 {
        match self {
            Tz::Utc => 0,
            Tz::Offset(s) => i64::from(s),
        }
    }
}

/// `datetime.date`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

/// `datetime.datetime`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateTime {
    pub date: Date,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub micro: u32,
    pub tz: Option<Tz>,
}

impl DateTime {
    /// (giây kể từ 0001-01-01 theo giờ ghi, micro giây); có múi giờ thì quy về UTC.
    fn instant(&self) -> (i64, u32) {
        let d = &self.date;
        // Số ngày theo lịch Gregory đón trước (thuật toán days_from_civil).
        let (y, m) = if d.month <= 2 {
            (i64::from(d.year) - 1, i64::from(d.month) + 9)
        } else {
            (i64::from(d.year), i64::from(d.month) - 3)
        };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * m + 2) / 5 + i64::from(d.day) - 1;
        let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let secs = days * 86_400
            + i64::from(self.hour) * 3600
            + i64::from(self.minute) * 60
            + i64::from(self.second)
            - self.tz.map_or(0, Tz::seconds);
        (secs, self.micro)
    }
}

/// Một object Python mà `SafeConstructor` có thể trả. Container trỏ tới object khác trong cùng [`Doc`].
#[derive(Debug, Clone)]
pub enum Obj {
    None,
    Bool(bool),
    /// Số nguyên ở dạng thập phân (Python `int` không giới hạn độ lớn; lõi không tính toán trên nó).
    Int(String),
    Float(f64),
    Str(String),
    Bytes(Vec<u8>),
    Date(Date),
    DateTime(DateTime),
    List(Vec<usize>),
    Tuple(Vec<usize>),
    Set(Vec<usize>),
    /// Dict giữ thứ tự chèn.
    Dict(Vec<(usize, usize)>),
    /// Container chưa được điền (chỉ có trong lúc dựng).
    Pending,
}

/// Kết quả của `yaml.load`: các object và object gốc.
#[derive(Debug, Clone)]
pub struct Doc {
    objs: Vec<Obj>,
    root: usize,
}

impl Doc {
    pub fn root(&self) -> Val<'_> {
        Val {
            objs: &self.objs,
            id: self.root,
        }
    }
}

/// Một object trong [`Doc`].
#[derive(Clone, Copy)]
pub struct Val<'a> {
    objs: &'a [Obj],
    id: usize,
}

impl std::fmt::Debug for Val<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.py_repr() {
            Ok(s) => f.write_str(&s),
            Err(e) => write!(f, "<{e}>"),
        }
    }
}

impl<'a> Val<'a> {
    fn at(&self, id: usize) -> Val<'a> {
        Val {
            objs: self.objs,
            id,
        }
    }

    pub fn obj(&self) -> &'a Obj {
        &self.objs[self.id]
    }

    /// Hai giá trị là cùng một object Python (alias, hoặc hằng NaN dùng chung).
    pub fn same(&self, other: &Val<'_>) -> bool {
        std::ptr::eq(self.objs.as_ptr(), other.objs.as_ptr()) && self.id == other.id
    }

    pub fn as_str(&self) -> Option<&'a str> {
        match self.obj() {
            Obj::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn is_none(&self) -> bool {
        matches!(self.obj(), Obj::None)
    }

    pub fn is_dict(&self) -> bool {
        matches!(self.obj(), Obj::Dict(_))
    }

    /// Phần tử của list (cả tuple); `None` nếu không phải list hay tuple.
    pub fn list(&self) -> Option<impl Iterator<Item = Val<'a>> + use<'a>> {
        let objs = self.objs;
        match self.obj() {
            Obj::List(ids) | Obj::Tuple(ids) => Some(ids.iter().map(move |&id| Val { objs, id })),
            _ => None,
        }
    }

    /// Cặp khoá, giá trị của dict theo thứ tự chèn; `None` nếu không phải dict.
    pub fn items(&self) -> Option<impl Iterator<Item = (Val<'a>, Val<'a>)> + use<'a>> {
        let objs = self.objs;
        match self.obj() {
            Obj::Dict(pairs) => Some(
                pairs
                    .iter()
                    .map(move |&(k, v)| (Val { objs, id: k }, Val { objs, id: v })),
            ),
            _ => None,
        }
    }

    /// `d.get(key)` với khoá chuỗi; `None` cả khi không phải dict.
    pub fn get(&self, key: &str) -> Option<Val<'a>> {
        self.items()?
            .find(|(k, _)| k.as_str() == Some(key))
            .map(|(_, v)| v)
    }

    /// Độ "đúng" của Python (`x or {}`, `bool(x)`).
    pub fn truthy(&self) -> bool {
        match self.obj() {
            Obj::None | Obj::Pending => false,
            Obj::Bool(b) => *b,
            Obj::Int(s) => s != "0",
            Obj::Float(f) => *f != 0.0,
            Obj::Str(s) => !s.is_empty(),
            Obj::Bytes(b) => !b.is_empty(),
            Obj::Date(_) | Obj::DateTime(_) => true,
            Obj::List(v) | Obj::Tuple(v) | Obj::Set(v) => !v.is_empty(),
            Obj::Dict(v) => !v.is_empty(),
        }
    }

    /// `isinstance(x, collections.abc.Hashable)`.
    fn hashable(&self) -> bool {
        !matches!(
            self.obj(),
            Obj::List(_) | Obj::Set(_) | Obj::Dict(_) | Obj::Pending
        )
    }

    /// `==` của Python giữa hai khoá dict (`1 == 1.0 == True`, `datetime` có múi giờ so theo UTC).
    fn key_eq(&self, other: &Val<'_>) -> bool {
        if self.same(other) {
            return true;
        }
        match (self.obj(), other.obj()) {
            (Obj::None, Obj::None) => true,
            (Obj::Str(a), Obj::Str(b)) => a == b,
            (Obj::Bytes(a), Obj::Bytes(b)) => a == b,
            (Obj::Date(a), Obj::Date(b)) => a == b,
            (Obj::DateTime(a), Obj::DateTime(b)) => match (a.tz, b.tz) {
                (None, None) => a == b,
                (Some(_), Some(_)) => a.instant() == b.instant(),
                _ => false,
            },
            (Obj::Tuple(a), Obj::Tuple(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b)
                        .all(|(&x, &y)| self.at(x).key_eq(&other.at(y)))
            }
            (a, b) => match (number(a), number(b)) {
                (Some(x), Some(y)) => x.eq(&y),
                _ => false,
            },
        }
    }

    /// `str(x)`. `Err` khi Python cũng ném lỗi: số nguyên quá 4300 chữ số (`ValueError`), lồng quá sâu.
    pub fn py_str(&self) -> Result<String, Fail> {
        match self.obj() {
            Obj::Str(s) => Ok(s.clone()),
            Obj::Date(d) => Ok(date_iso(d)),
            Obj::DateTime(dt) => Ok(datetime_iso(dt, ' ')),
            _ => self.py_repr(),
        }
    }

    /// `repr(x)`. Container đang in mà gặp lại chính nó thì in `[...]`, `{...}`, `(...)` như Python.
    pub fn py_repr(&self) -> Result<String, Fail> {
        let mut out = String::new();
        self.repr_into(&mut out, &mut Vec::new())?;
        Ok(out)
    }

    fn repr_into(&self, out: &mut String, open: &mut Vec<usize>) -> Result<(), Fail> {
        let (start, end, ids): (&str, &str, &[usize]) = match self.obj() {
            Obj::None => {
                out.push_str("None");
                return Ok(());
            }
            Obj::Bool(b) => {
                out.push_str(if *b { "True" } else { "False" });
                return Ok(());
            }
            Obj::Int(s) => {
                if !scalar::int_str_ok(s) {
                    return Err(Fail::Crash("ValueError"));
                }
                out.push_str(s);
                return Ok(());
            }
            Obj::Float(f) => {
                out.push_str(&float_repr(*f));
                return Ok(());
            }
            Obj::Str(s) => {
                out.push_str(&str_repr(s));
                return Ok(());
            }
            Obj::Bytes(b) => {
                out.push_str(&bytes_repr(b));
                return Ok(());
            }
            Obj::Date(d) => {
                out.push_str(&date_repr(d));
                return Ok(());
            }
            Obj::DateTime(dt) => {
                out.push_str(&datetime_repr(dt));
                return Ok(());
            }
            Obj::Pending => {
                out.push_str("[]");
                return Ok(());
            }
            Obj::List(ids) => ("[", "]", ids),
            Obj::Tuple(ids) => ("(", ")", ids),
            Obj::Set(ids) if ids.is_empty() => {
                out.push_str("set()");
                return Ok(());
            }
            // Python in set theo thứ tự băm (đổi theo PYTHONHASHSEED); Rust giữ thứ tự chèn.
            Obj::Set(ids) => ("{", "}", ids),
            Obj::Dict(pairs) => {
                if open.contains(&self.id) {
                    out.push_str("{...}");
                    return Ok(());
                }
                if open.len() >= MAX_DEPTH {
                    return Err(RECURSION);
                }
                open.push(self.id);
                out.push('{');
                for (i, &(k, v)) in pairs.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    self.at(k).repr_into(out, open)?;
                    out.push_str(": ");
                    self.at(v).repr_into(out, open)?;
                }
                out.push('}');
                open.pop();
                return Ok(());
            }
        };
        if open.contains(&self.id) {
            out.push_str(&format!("{start}...{end}"));
            return Ok(());
        }
        if open.len() >= MAX_DEPTH {
            return Err(RECURSION);
        }
        open.push(self.id);
        out.push_str(start);
        for (i, &id) in ids.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            self.at(id).repr_into(out, open)?;
        }
        if matches!(self.obj(), Obj::Tuple(_)) && ids.len() == 1 {
            out.push(',');
        }
        out.push_str(end);
        open.pop();
        Ok(())
    }
}

/// Giá trị số của `bool`, `int`, `float` để so khoá (`True == 1 == 1.0`). Số nguyên so chính xác với số thực
/// nguyên (`10**20 == 1e20` nhưng `10**20 + 1 != 1e20`).
#[derive(Debug, PartialEq)]
enum Num<'a> {
    Int(std::borrow::Cow<'a, str>),
    Float(f64),
}

fn number(o: &Obj) -> Option<Num<'_>> {
    match o {
        Obj::Bool(b) => Some(Num::Int(if *b { "1" } else { "0" }.into())),
        Obj::Int(s) => Some(Num::Int(s.as_str().into())),
        Obj::Float(f) if f.is_finite() && f.fract() == 0.0 => {
            Some(Num::Int(format!("{:.0}", f + 0.0).into()))
        }
        Obj::Float(f) => Some(Num::Float(*f)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Resolver (yaml/resolver.py): danh sách regex theo ký tự đầu, đúng thứ tự đăng ký.

fn re(p: &str) -> Regex {
    Regex::new(p).expect("regex tĩnh hợp lệ")
}

static R_BOOL: LazyLock<Regex> = LazyLock::new(|| {
    re(r"\A(?:yes|Yes|YES|no|No|NO|true|True|TRUE|false|False|FALSE|on|On|ON|off|Off|OFF)\z")
});
static R_FLOAT: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"\A(?:[-+]?(?:[0-9][0-9_]*)\.[0-9_]*(?:[eE][-+][0-9]+)?|\.[0-9][0-9_]*(?:[eE][-+][0-9]+)?|[-+]?[0-9][0-9_]*(?::[0-5]?[0-9])+\.[0-9_]*|[-+]?\.(?:inf|Inf|INF)|\.(?:nan|NaN|NAN))\z",
    )
});
static R_INT: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"\A(?:[-+]?0b[0-1_]+|[-+]?0[0-7_]+|[-+]?(?:0|[1-9][0-9_]*)|[-+]?0x[0-9a-fA-F_]+|[-+]?[1-9][0-9_]*(?::[0-5]?[0-9])+)\z",
    )
});
static R_NULL: LazyLock<Regex> = LazyLock::new(|| re(r"\A(?:~|null|Null|NULL|)\z"));
static R_TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"\A(?:[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]|[0-9][0-9][0-9][0-9]-[0-9][0-9]?-[0-9][0-9]?(?:[Tt]|[ \t]+)[0-9][0-9]?:[0-9][0-9]:[0-9][0-9](?:\.[0-9]*)?(?:[ \t]*(?:Z|[-+][0-9][0-9]?(?::[0-9][0-9])?))?)\z",
    )
});

/// Bộ resolver dùng khi đọc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loader {
    /// `vault.LOADER`: `CSafeLoader` bỏ resolver `timestamp` (ngày giữ nguyên chuỗi).
    Frontmatter,
    /// `CSafeLoader` đủ resolver (`schema.read_schema`).
    Safe,
}

impl Loader {
    /// Tag của scalar trơn không tag (hoặc tag `!`).
    fn resolve_plain(self, value: &str) -> &'static str {
        let candidates: &[(&'static str, &LazyLock<Regex>)] = match value.chars().next() {
            None => &[(NULL, &R_NULL)],
            Some('y' | 'Y' | 't' | 'T' | 'f' | 'F' | 'o' | 'O') => &[(BOOL, &R_BOOL)],
            Some('n' | 'N') => &[(BOOL, &R_BOOL), (NULL, &R_NULL)],
            Some('0'..='9') if self == Loader::Safe => {
                &[(FLOAT, &R_FLOAT), (INT, &R_INT), (TIMESTAMP, &R_TIMESTAMP)]
            }
            Some('-' | '+' | '0'..='9') => &[(FLOAT, &R_FLOAT), (INT, &R_INT)],
            Some('.') => &[(FLOAT, &R_FLOAT)],
            Some('~') => &[(NULL, &R_NULL)],
            Some('<') if value == "<<" => return MERGE,
            Some('=') if value == "=" => return VALUE,
            _ => &[],
        };
        candidates
            .iter()
            .find(|(_, r)| r.is_match(value))
            .map_or(STR, |(t, _)| t)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Composer (CParser._compose_*), viết dạng vòng lặp để note lồng sâu không làm tràn ngăn xếp.

#[derive(Debug, Clone)]
enum Kind {
    Scalar(String),
    Seq(Vec<usize>),
    Map(Vec<(usize, usize)>),
}

#[derive(Debug)]
struct Node {
    tag: String,
    kind: Kind,
}

impl Node {
    fn id(&self) -> &'static str {
        match self.kind {
            Kind::Scalar(_) => "scalar",
            Kind::Seq(_) => "sequence",
            Kind::Map(_) => "mapping",
        }
    }
}

struct Composer<'a> {
    parser: Parser<'a>,
    loader: Loader,
    nodes: Vec<Node>,
    anchors: HashMap<String, usize>,
}

impl Composer<'_> {
    fn next(&mut self) -> Result<Event, Fail> {
        self.parser.next().map_err(Fail::Error)
    }

    fn add(&mut self, anchor: Option<String>, node: Node) -> Result<usize, Fail> {
        let id = self.nodes.len();
        if let Some(a) = anchor {
            if self.anchors.contains_key(&a) {
                return err("found duplicate anchor; first occurrence");
            }
            self.anchors.insert(a, id);
        }
        self.nodes.push(node);
        Ok(id)
    }

    /// Một document: node gốc.
    fn document(&mut self) -> Result<usize, Fail> {
        // (container đang mở, khoá đang chờ giá trị nếu là mapping)
        let mut open: Vec<(usize, Option<usize>)> = Vec::new();
        loop {
            let (id, starts) = match self.next()? {
                // Alias tới container còn đang mở được phép, như CParser.
                Event::Alias(a) => match self.anchors.get(&a) {
                    Some(&id) => (id, false),
                    None => return err("found undefined alias"),
                },
                Event::Scalar {
                    anchor,
                    tag,
                    value,
                    plain_implicit,
                } => {
                    let tag = match tag {
                        Some(t) if t != "!" => t,
                        _ if plain_implicit => self.loader.resolve_plain(&value).to_owned(),
                        _ => STR.to_owned(),
                    };
                    let node = Node {
                        tag,
                        kind: Kind::Scalar(value),
                    };
                    (self.add(anchor, node)?, false)
                }
                Event::SeqStart { anchor, tag } => {
                    let node = Node {
                        tag: tag.filter(|t| t != "!").unwrap_or_else(|| SEQ.into()),
                        kind: Kind::Seq(Vec::new()),
                    };
                    (self.add(anchor, node)?, true)
                }
                Event::MapStart { anchor, tag } => {
                    let node = Node {
                        tag: tag.filter(|t| t != "!").unwrap_or_else(|| MAP.into()),
                        kind: Kind::Map(Vec::new()),
                    };
                    (self.add(anchor, node)?, true)
                }
                Event::SeqEnd | Event::MapEnd => match open.pop() {
                    Some((id, _)) if open.is_empty() => return Ok(id),
                    Some(_) => continue,
                    None => return err("unexpected end of collection"),
                },
                other => return err(format!("unexpected event {other:?}")),
            };
            match open.last_mut() {
                Some((parent, pending)) => match &mut self.nodes[*parent].kind {
                    Kind::Seq(items) => items.push(id),
                    Kind::Map(pairs) => match pending.take() {
                        Some(k) => pairs.push((k, id)),
                        None => *pending = Some(id),
                    },
                    Kind::Scalar(_) => {}
                },
                None if !starts => return Ok(id),
                None => {}
            }
            if starts {
                if open.len() >= MAX_DEPTH {
                    return Err(RECURSION);
                }
                open.push((id, None));
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Constructor (SafeConstructor).
//
// Container được dựng như generator của PyYAML: trả container rỗng ngay, điền ở vòng sau, từng vòng theo hàng đợi
// (`construct_document` chạy `state_generators`). Thứ tự này quyết định lỗi nào hiện ra khi note có nhiều lỗi.

struct Ctor {
    nodes: Vec<Node>,
    objs: Vec<Obj>,
    /// node -> object (`constructed_objects`).
    cache: HashMap<usize, usize>,
    /// (object, node) có generator chạy ở vòng sau.
    pending: Vec<(usize, usize)>,
    /// Hằng `nan_value` dùng chung của `SafeConstructor` (`.nan` nào cũng là cùng một object).
    nan: Option<usize>,
}

impl Ctor {
    fn push(&mut self, o: Obj) -> usize {
        self.objs.push(o);
        self.objs.len() - 1
    }

    fn at(&self, id: usize) -> Val<'_> {
        Val {
            objs: &self.objs,
            id,
        }
    }

    /// `construct_object`: scalar dựng ngay, container để sau.
    fn object(&mut self, id: usize) -> Result<usize, Fail> {
        if let Some(&o) = self.cache.get(&id) {
            return Ok(o);
        }
        let tag = self.nodes[id].tag.clone();
        let v = match tag.as_str() {
            NULL => {
                self.scalar(id, 0)?;
                Obj::None
            }
            BOOL => match self.scalar(id, 0)?.to_lowercase().as_str() {
                "yes" | "true" | "on" => Obj::Bool(true),
                "no" | "false" | "off" => Obj::Bool(false),
                _ => return Err(Fail::Crash("KeyError")),
            },
            INT => Obj::Int(construct_int(&self.scalar(id, 0)?)?),
            FLOAT => match construct_float(&self.scalar(id, 0)?)? {
                Some(f) => Obj::Float(f),
                None => {
                    let o = match self.nan {
                        Some(o) => o,
                        None => {
                            let o = self.push(Obj::Float(f64::NAN));
                            self.nan = Some(o);
                            o
                        }
                    };
                    self.cache.insert(id, o);
                    return Ok(o);
                }
            },
            STR => Obj::Str(self.scalar(id, 0)?),
            BINARY => Obj::Bytes(construct_binary(&self.scalar(id, 0)?)?),
            TIMESTAMP => {
                // PyYAML khớp regex trên `node.value`, không phải trên kết quả của `construct_scalar`.
                let raw = match &self.nodes[id].kind {
                    Kind::Scalar(s) => s.clone(),
                    _ => return Err(Fail::Crash("TypeError")),
                };
                self.scalar(id, 0)?;
                match construct_timestamp(&raw)? {
                    Stamp::Date(d) => Obj::Date(d),
                    Stamp::DateTime(dt) => Obj::DateTime(dt),
                }
            }
            SEQ | MAP | SET | OMAP | PAIRS => {
                let o = self.push(Obj::Pending);
                self.cache.insert(id, o);
                self.pending.push((o, id));
                return Ok(o);
            }
            _ => {
                return err(format!(
                    "could not determine a constructor for the tag {}",
                    str_repr(&tag)
                ));
            }
        };
        let o = self.push(v);
        self.cache.insert(id, o);
        Ok(o)
    }

    /// Thân generator của một container, chạy khi tới vòng của nó.
    fn fill(&mut self, o: usize, id: usize) -> Result<(), Fail> {
        let tag = self.nodes[id].tag.clone();
        let filled = match tag.as_str() {
            SEQ => {
                let Kind::Seq(items) = &self.nodes[id].kind else {
                    return err(format!(
                        "expected a sequence node, but found {}",
                        self.nodes[id].id()
                    ));
                };
                let items = items.clone();
                Obj::List(
                    items
                        .into_iter()
                        .map(|c| self.object(c))
                        .collect::<Result<_, _>>()?,
                )
            }
            MAP => Obj::Dict(self.mapping(id)?),
            SET => Obj::Set(self.mapping(id)?.into_iter().map(|(k, _)| k).collect()),
            _ => self.pairs(id, tag == OMAP)?,
        };
        self.objs[o] = filled;
        Ok(())
    }

    /// `SafeConstructor.construct_scalar`: mapping có khoá `=` thay cho giá trị của khoá đó.
    fn scalar(&mut self, id: usize, depth: usize) -> Result<String, Fail> {
        if depth >= MAX_DEPTH {
            return Err(RECURSION);
        }
        match &self.nodes[id].kind {
            Kind::Scalar(s) => Ok(s.clone()),
            Kind::Map(pairs) => match pairs.iter().find(|(k, _)| self.nodes[*k].tag == VALUE) {
                Some(&(_, v)) => self.scalar(v, depth + 1),
                None => err("expected a scalar node, but found mapping"),
            },
            Kind::Seq(_) => err("expected a scalar node, but found sequence"),
        }
    }

    fn map_pairs(&self, id: usize) -> &[(usize, usize)] {
        match &self.nodes[id].kind {
            Kind::Map(p) => p,
            _ => &[],
        }
    }

    /// `SafeConstructor.flatten_mapping`: kéo các mục `<<` lên trước, khoá `=` thành chuỗi. Sửa node tại chỗ như
    /// Python (mục `<<` bị xoá trước khi đi vào mapping được gộp, nên `<<` trỏ vòng vẫn dừng).
    fn flatten(&mut self, id: usize, depth: usize) -> Result<(), Fail> {
        if depth >= MAX_DEPTH {
            return Err(RECURSION);
        }
        let mut merge: Vec<(usize, usize)> = Vec::new();
        let mut index = 0;
        while index < self.map_pairs(id).len() {
            let (k, v) = self.map_pairs(id)[index];
            if self.nodes[k].tag == MERGE {
                if let Kind::Map(p) = &mut self.nodes[id].kind {
                    p.remove(index);
                }
                match self.nodes[v].kind.clone() {
                    Kind::Map(_) => {
                        self.flatten(v, depth + 1)?;
                        merge.extend_from_slice(self.map_pairs(v));
                    }
                    Kind::Seq(items) => {
                        let mut sub = Vec::new();
                        for s in items {
                            if !matches!(self.nodes[s].kind, Kind::Map(_)) {
                                return err("while constructing a mapping");
                            }
                            self.flatten(s, depth + 1)?;
                            sub.push(self.map_pairs(s).to_vec());
                        }
                        sub.into_iter().rev().for_each(|s| merge.extend(s));
                    }
                    Kind::Scalar(_) => return err("while constructing a mapping"),
                }
            } else {
                if self.nodes[k].tag == VALUE {
                    self.nodes[k].tag = STR.into();
                }
                index += 1;
            }
        }
        if !merge.is_empty() {
            merge.extend_from_slice(self.map_pairs(id));
            self.nodes[id].kind = Kind::Map(merge);
        }
        Ok(())
    }

    /// `construct_mapping` vào dict giữ thứ tự chèn, so khoá bằng `==` của Python: khoá trùng giữ object khoá đầu,
    /// lấy giá trị sau.
    fn mapping(&mut self, id: usize) -> Result<Vec<(usize, usize)>, Fail> {
        if !matches!(self.nodes[id].kind, Kind::Map(_)) {
            return err(format!(
                "expected a mapping node, but found {}",
                self.nodes[id].id()
            ));
        }
        self.flatten(id, 0)?;
        let pairs = self.map_pairs(id).to_vec();
        let mut out: Vec<(usize, usize)> = Vec::new();
        for (k, v) in pairs {
            let key = self.object(k)?;
            if !self.at(key).hashable() {
                return err("while constructing a mapping");
            }
            let val = self.object(v)?;
            let hit = out
                .iter()
                .position(|&(e, _)| self.at(e).key_eq(&self.at(key)));
            match hit {
                Some(i) => out[i].1 = val,
                None => out.push((key, val)),
            }
        }
        Ok(out)
    }

    fn pairs(&mut self, id: usize, omap: bool) -> Result<Obj, Fail> {
        let ctx = if omap {
            "while constructing an ordered map"
        } else {
            "while constructing pairs"
        };
        let Kind::Seq(items) = self.nodes[id].kind.clone() else {
            return err(ctx);
        };
        let mut out = Vec::new();
        for s in items {
            let [(k, v)] = self.map_pairs(s) else {
                return err(ctx);
            };
            let (k, v) = (*k, *v);
            let pair = vec![self.object(k)?, self.object(v)?];
            out.push(self.push(Obj::Tuple(pair)));
        }
        Ok(Obj::List(out))
    }
}

/// `yaml.load(text, Loader=...)`. `Ok(None)` khi stream không có document nào (rỗng, chỉ có chú thích).
pub fn load(text: &str, loader: Loader) -> Result<Option<Doc>, Fail> {
    let parser = Parser::new(text).map_err(|_| Fail::Crash("MemoryError"))?;
    let mut c = Composer {
        parser,
        loader,
        nodes: Vec::new(),
        anchors: HashMap::new(),
    };
    let root = match c.next()? {
        Event::StreamEnd => return Ok(None),
        Event::DocStart => {
            let root = c.document()?;
            match c.next()? {
                Event::DocEnd => {}
                other => return err(format!("unexpected event {other:?}")),
            }
            root
        }
        other => return err(format!("unexpected event {other:?}")),
    };
    if !matches!(c.next()?, Event::StreamEnd) {
        return err("expected a single document in the stream");
    }
    let mut ctor = Ctor {
        nodes: c.nodes,
        objs: Vec::new(),
        cache: HashMap::new(),
        pending: Vec::new(),
        nan: None,
    };
    let root = ctor.object(root)?;
    while !ctor.pending.is_empty() {
        for (o, id) in std::mem::take(&mut ctor.pending) {
            ctor.fill(o, id)?;
        }
    }
    Ok(Some(Doc {
        objs: ctor.objs,
        root,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fm(text: &str) -> Doc {
        load(text, Loader::Frontmatter)
            .expect("YAML hợp lệ")
            .expect("có document")
    }

    fn repr_of(d: &Doc, key: &str) -> String {
        d.root().get(key).expect(key).py_repr().expect("in được")
    }

    #[test]
    fn resolver_follows_yaml_1_1() {
        for (v, t) in [
            ("yes", BOOL),
            ("Off", BOOL),
            ("y", STR),
            ("nULL", STR),
            ("", NULL),
            ("~", NULL),
            ("010", INT),
            ("0o17", STR),
            ("08", STR),
            ("1_000", INT),
            ("10:30", INT),
            ("09:30", STR),
            ("1e3", STR),
            ("1.0e+3", FLOAT),
            ("1.", FLOAT),
            ("2026-01-05", STR),
            ("<<", MERGE),
            ("=", VALUE),
        ] {
            assert_eq!(Loader::Frontmatter.resolve_plain(v), t, "{v:?}");
        }
        assert_eq!(Loader::Safe.resolve_plain("2026-01-05"), TIMESTAMP);
        assert_eq!(Loader::Safe.resolve_plain("2026-1-5 1:00:00"), TIMESTAMP);
        assert_eq!(Loader::Safe.resolve_plain("2026-1-5"), STR);
    }

    #[test]
    fn schema_loader_builds_dates() {
        let d = load("updated: 2026-10-29", Loader::Safe).unwrap().unwrap();
        assert_eq!(repr_of(&d, "updated"), "datetime.date(2026, 10, 29)");
        let d = fm("updated: 2026-10-29");
        assert_eq!(
            d.root().get("updated").unwrap().as_str(),
            Some("2026-10-29")
        );
    }

    #[test]
    fn dict_keys_compare_like_python() {
        let d = fm("1: a\n1.0: b\ntrue: c\n'1': d\n.nan: e\n.nan: f\n");
        assert_eq!(d.root().py_repr().unwrap(), "{1: 'c', '1': 'd', nan: 'f'}");
        let d = fm(
            "? !!timestamp 2026-01-05 10:00:00+07:00\n: a\n? !!timestamp 2026-01-05 03:00:00Z\n: b\n",
        );
        assert_eq!(d.root().items().unwrap().count(), 1);
        let d = fm("? !!float nan\n: x\n? !!float nan\n: y\n");
        assert_eq!(d.root().items().unwrap().count(), 2);
        let d = fm("? !!int 100000000000000000000\n: x\n? 1.0e+20\n: y\n");
        assert_eq!(d.root().items().unwrap().count(), 1);
        let d = fm("? !!int 100000000000000000001\n: x\n? 1.0e+20\n: y\n");
        assert_eq!(d.root().items().unwrap().count(), 2);
    }

    #[test]
    fn recursive_containers_print_like_python() {
        let d = fm("a: &d {b: *d}");
        assert_eq!(d.root().get("a").unwrap().py_str().unwrap(), "{'b': {...}}");
        let d = fm("a: &l [1, {k: *l}]");
        assert_eq!(repr_of(&d, "a"), "[1, {'k': [...]}]");
        let d = fm("a: &p !!pairs [{k: *p}]");
        assert_eq!(repr_of(&d, "a"), "[('k', [...])]");
    }

    #[test]
    fn merge_follows_safe_constructor() {
        let d = fm("b: &b {x: 1, y: 2}\nc: &c {y: 9, z: 3}\nd: {<<: [*b, *c], y: 5}\n");
        assert_eq!(repr_of(&d, "d"), "{'y': 5, 'z': 3, 'x': 1}");
        // `<<` trỏ vào chính mapping chứa nó: Python xoá mục `<<` trước khi gộp nên vẫn dừng.
        let d = fm("a: &a {x: 1, <<: *a}");
        assert_eq!(repr_of(&d, "a"), "{'x': 1}");
    }

    #[test]
    fn errors_follow_pyyaml() {
        let e = |t: &str| load(t, Loader::Frontmatter).unwrap_err();
        assert_eq!(e("a: *x"), Fail::Error("found undefined alias".into()));
        assert_eq!(e("a: !!bool maybe"), Fail::Crash("KeyError"));
        assert_eq!(
            e("a: !!set [1]"),
            Fail::Error("expected a mapping node, but found sequence".into())
        );
        assert_eq!(
            e("a: !!str [1]"),
            Fail::Error("expected a scalar node, but found sequence".into())
        );
        assert_eq!(
            e("a: !!omap [1]"),
            Fail::Error("while constructing an ordered map".into())
        );
        assert_eq!(
            e("[a]: 1"),
            Fail::Error("while constructing a mapping".into())
        );
        assert_eq!(
            e("a: 1\n---\nb: 2"),
            Fail::Error("expected a single document in the stream".into())
        );
        assert!(
            load("# chỉ chú thích", Loader::Frontmatter)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn value_key_stands_for_scalar() {
        let d = fm("a: !!int {=: '12', b: x}");
        assert_eq!(repr_of(&d, "a"), "12");
    }

    #[test]
    fn deep_nesting_is_a_recursion_error_not_a_stack_overflow() {
        let deep = format!("a: {}{}", "[".repeat(5000), "]".repeat(5000));
        assert_eq!(load(&deep, Loader::Frontmatter).unwrap_err(), RECURSION);
        let ok = format!("a: {}{}", "[".repeat(900), "]".repeat(900));
        assert!(load(&ok, Loader::Frontmatter).is_ok());
    }

    #[test]
    fn huge_int_fails_only_when_printed() {
        let d = fm(&format!("a: !!int 0x{}", "f".repeat(4000)));
        assert_eq!(
            d.root().get("a").unwrap().py_str(),
            Err(Fail::Crash("ValueError"))
        );
    }
}
