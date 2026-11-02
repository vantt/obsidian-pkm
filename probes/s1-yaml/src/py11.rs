//! PyYAML layer: compose (CParser), resolve (YAML 1.1 implicit resolvers minus `timestamp`, as the spec's
//! `vault.LOADER` sets up) and construct (`SafeConstructor`) on top of any [`EventSource`].
//!
//! Errors carry the first line of `str(exception)`, the part `vault.parse` stores. Exceptions PyYAML does
//! not wrap (so the spec's indexer would crash) come back as [`Fail::Crash`] with the Python type name.

use crate::events::{Ev, EventSource};
use crate::pyvalue::{DateTime, Py, Tz, str_repr, timedelta_repr};
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

pub const NULL: &str = "tag:yaml.org,2002:null";
pub const BOOL: &str = "tag:yaml.org,2002:bool";
pub const INT: &str = "tag:yaml.org,2002:int";
pub const FLOAT: &str = "tag:yaml.org,2002:float";
pub const STR: &str = "tag:yaml.org,2002:str";
pub const SEQ: &str = "tag:yaml.org,2002:seq";
pub const MAP: &str = "tag:yaml.org,2002:map";
pub const MERGE: &str = "tag:yaml.org,2002:merge";
pub const VALUE: &str = "tag:yaml.org,2002:value";
const BINARY: &str = "tag:yaml.org,2002:binary";
const TIMESTAMP: &str = "tag:yaml.org,2002:timestamp";
const OMAP: &str = "tag:yaml.org,2002:omap";
const PAIRS: &str = "tag:yaml.org,2002:pairs";
const SET: &str = "tag:yaml.org,2002:set";

#[derive(Debug, Clone, PartialEq)]
pub enum Fail {
    /// `yaml.YAMLError` or `ValueError`: first line of the message.
    Error(String),
    /// Any other exception: Python type name. The spec's indexer would stop on this note.
    Crash(&'static str),
}

fn err<T>(msg: impl Into<String>) -> Result<T, Fail> {
    Err(Fail::Error(msg.into()))
}

// ---------------------------------------------------------------------------------------------
// Resolver (yaml/resolver.py, registration order kept per first character).

fn re(p: &str) -> Regex {
    Regex::new(p).expect("static regex compiles")
}

static R_BOOL: LazyLock<Regex> = LazyLock::new(|| {
    re(r"^(?:yes|Yes|YES|no|No|NO|true|True|TRUE|false|False|FALSE|on|On|ON|off|Off|OFF)$")
});
static R_FLOAT: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"^(?:[-+]?(?:[0-9][0-9_]*)\.[0-9_]*(?:[eE][-+][0-9]+)?|\.[0-9][0-9_]*(?:[eE][-+][0-9]+)?|[-+]?[0-9][0-9_]*(?::[0-5]?[0-9])+\.[0-9_]*|[-+]?\.(?:inf|Inf|INF)|\.(?:nan|NaN|NAN))$",
    )
});
static R_INT: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"^(?:[-+]?0b[0-1_]+|[-+]?0[0-7_]+|[-+]?(?:0|[1-9][0-9_]*)|[-+]?0x[0-9a-fA-F_]+|[-+]?[1-9][0-9_]*(?::[0-5]?[0-9])+)$",
    )
});
static R_NULL: LazyLock<Regex> = LazyLock::new(|| re(r"^(?:~|null|Null|NULL|)$"));
/// The timestamp *resolver* is removed by the spec, but `!!timestamp` still constructs with this.
static R_TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| {
    re(
        r"^(?P<year>[0-9][0-9][0-9][0-9])-(?P<month>[0-9][0-9]?)-(?P<day>[0-9][0-9]?)(?:(?:[Tt]|[ \t]+)(?P<hour>[0-9][0-9]?):(?P<minute>[0-9][0-9]):(?P<second>[0-9][0-9])(?:\.(?P<fraction>[0-9]*))?(?:[ \t]*(?P<tz>Z|(?P<tz_sign>[-+])(?P<tz_hour>[0-9][0-9]?)(?::(?P<tz_minute>[0-9][0-9]))?))?)?$",
    )
});

/// Tag for an untagged (or `!`-tagged) plain scalar.
pub fn resolve_plain(value: &str) -> &'static str {
    let first = value.chars().next();
    let candidates: &[(&'static str, &LazyLock<Regex>)] = match first {
        None => &[(NULL, &R_NULL)],
        Some('y' | 'Y' | 't' | 'T' | 'f' | 'F' | 'o' | 'O') => &[(BOOL, &R_BOOL)],
        Some('n' | 'N') => &[(BOOL, &R_BOOL), (NULL, &R_NULL)],
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

// ---------------------------------------------------------------------------------------------
// Composer (CParser._compose_*).

#[derive(Debug)]
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

struct Composer<'s> {
    src: &'s mut dyn EventSource,
    nodes: Vec<Node>,
    anchors: HashMap<String, usize>,
}

impl Composer<'_> {
    fn next(&mut self) -> Result<Ev, Fail> {
        self.src.next_ev().map_err(Fail::Error)
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

    fn compose(&mut self, ev: Ev) -> Result<usize, Fail> {
        match ev {
            // An alias to an enclosing collection is allowed here, as in CParser; see `Ctor::value`.
            Ev::Alias(a) => match self.anchors.get(&a) {
                None => err("found undefined alias"),
                Some(&id) => Ok(id),
            },
            Ev::Scalar {
                anchor,
                tag,
                value,
                plain_implicit,
                ..
            } => {
                let tag = match tag {
                    Some(t) if t != "!" => t,
                    _ if plain_implicit => resolve_plain(&value).to_string(),
                    _ => STR.to_string(),
                };
                self.add(
                    anchor,
                    Node {
                        tag,
                        kind: Kind::Scalar(value),
                    },
                )
            }
            Ev::SeqStart { anchor, tag } => {
                let tag = tag.filter(|t| t != "!").unwrap_or_else(|| SEQ.into());
                let id = self.add(
                    anchor,
                    Node {
                        tag,
                        kind: Kind::Seq(Vec::new()),
                    },
                )?;
                loop {
                    let ev = self.next()?;
                    if matches!(ev, Ev::SeqEnd) {
                        break;
                    }
                    let child = self.compose(ev)?;
                    if let Kind::Seq(items) = &mut self.nodes[id].kind {
                        items.push(child);
                    }
                }
                Ok(id)
            }
            Ev::MapStart { anchor, tag } => {
                let tag = tag.filter(|t| t != "!").unwrap_or_else(|| MAP.into());
                let id = self.add(
                    anchor,
                    Node {
                        tag,
                        kind: Kind::Map(Vec::new()),
                    },
                )?;
                loop {
                    let ev = self.next()?;
                    if matches!(ev, Ev::MapEnd) {
                        break;
                    }
                    let k = self.compose(ev)?;
                    let ev = self.next()?;
                    let v = self.compose(ev)?;
                    if let Kind::Map(pairs) = &mut self.nodes[id].kind {
                        pairs.push((k, v));
                    }
                }
                Ok(id)
            }
            other => err(format!("probe: unexpected event {other:?}")),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Constructor (SafeConstructor).
//
// Collections are built the way PyYAML's generator constructors build them: the empty container is
// returned at once and filled later, one breadth-first round at a time (`construct_document` draining
// `state_generators`). That order decides which error surfaces first when a note has several, so it is
// kept. Values live in an arena so an alias can share a container that is still being filled.

#[derive(Debug, Clone)]
enum Obj {
    Done(Py),
    /// Container whose generator has not run yet.
    Pending,
    List(Vec<usize>),
    Tuple(Vec<usize>),
    Set(Vec<usize>),
    Dict(Vec<(usize, usize)>),
}

struct Ctor {
    nodes: Vec<Node>,
    objs: Vec<Obj>,
    /// node id -> object id (`constructed_objects`).
    cache: HashMap<usize, usize>,
    /// (object, node) pairs whose generator runs in the next round (`state_generators`).
    pending: Vec<(usize, usize)>,
}

impl Ctor {
    fn push(&mut self, o: Obj) -> usize {
        self.objs.push(o);
        self.objs.len() - 1
    }

    /// `construct_object`: scalars at once, containers deferred.
    fn object(&mut self, id: usize) -> Result<usize, Fail> {
        if let Some(&o) = self.cache.get(&id) {
            return Ok(o);
        }
        let tag = self.nodes[id].tag.clone();
        let v = match tag.as_str() {
            NULL => {
                self.scalar(id)?;
                Py::None
            }
            BOOL => match self.scalar(id)?.to_lowercase().as_str() {
                "yes" | "true" | "on" => Py::Bool(true),
                "no" | "false" | "off" => Py::Bool(false),
                _ => return Err(Fail::Crash("KeyError")),
            },
            INT => construct_int(&self.scalar(id)?)?,
            FLOAT => construct_float(&self.scalar(id)?)?,
            STR => Py::Str(self.scalar(id)?),
            BINARY => construct_binary(&self.scalar(id)?)?,
            TIMESTAMP => construct_timestamp(&self.scalar(id)?)?,
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
        let o = self.push(Obj::Done(v));
        self.cache.insert(id, o);
        Ok(o)
    }

    /// The body of a generator constructor, run when its round comes.
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

    /// SafeConstructor.construct_scalar: a mapping with a `=` key stands for that key's value.
    fn scalar(&mut self, id: usize) -> Result<String, Fail> {
        match &self.nodes[id].kind {
            Kind::Scalar(s) => Ok(s.clone()),
            Kind::Map(pairs) => {
                let hit = pairs.iter().find(|(k, _)| self.nodes[*k].tag == VALUE);
                match hit {
                    Some(&(_, v)) => self.scalar(v),
                    None => err("expected a scalar node, but found mapping"),
                }
            }
            Kind::Seq(_) => err("expected a scalar node, but found sequence"),
        }
    }

    /// SafeConstructor.flatten_mapping: pull `<<` entries in front, `=` keys become plain strings.
    fn flatten(&mut self, id: usize) -> Result<(), Fail> {
        let Kind::Map(pairs) = &self.nodes[id].kind else {
            return Ok(());
        };
        let pairs = pairs.clone();
        let (mut merge, mut rest) = (Vec::new(), Vec::new());
        for (k, v) in pairs {
            if self.nodes[k].tag == MERGE {
                match &self.nodes[v].kind {
                    Kind::Map(_) => {
                        self.flatten(v)?;
                        if let Kind::Map(sub) = &self.nodes[v].kind {
                            merge.extend(sub.iter().copied());
                        }
                    }
                    Kind::Seq(items) => {
                        let items = items.clone();
                        let mut subs = Vec::new();
                        for s in items {
                            if !matches!(self.nodes[s].kind, Kind::Map(_)) {
                                return err("while constructing a mapping");
                            }
                            self.flatten(s)?;
                            if let Kind::Map(sub) = &self.nodes[s].kind {
                                subs.push(sub.clone());
                            }
                        }
                        subs.into_iter().rev().for_each(|s| merge.extend(s));
                    }
                    Kind::Scalar(_) => return err("while constructing a mapping"),
                }
            } else {
                if self.nodes[k].tag == VALUE {
                    self.nodes[k].tag = STR.into();
                }
                rest.push((k, v));
            }
        }
        merge.extend(rest);
        self.nodes[id].kind = Kind::Map(merge);
        Ok(())
    }

    fn hashable(&self, o: usize) -> bool {
        match &self.objs[o] {
            Obj::Done(p) => p.hashable(),
            Obj::Tuple(_) => true,
            Obj::Pending | Obj::List(_) | Obj::Set(_) | Obj::Dict(_) => false,
        }
    }

    /// SafeConstructor.construct_mapping into an insertion-ordered dict with Python key equality.
    fn mapping(&mut self, id: usize) -> Result<Vec<(usize, usize)>, Fail> {
        if !matches!(self.nodes[id].kind, Kind::Map(_)) {
            return err(format!(
                "expected a mapping node, but found {}",
                self.nodes[id].id()
            ));
        }
        self.flatten(id)?;
        let Kind::Map(pairs) = &self.nodes[id].kind else {
            unreachable!("flatten keeps a mapping a mapping")
        };
        let pairs = pairs.clone();
        let mut out: Vec<(usize, usize)> = Vec::new();
        for (k, v) in pairs {
            let key = self.object(k)?;
            if !self.hashable(key) {
                return err("while constructing a mapping");
            }
            let val = self.object(v)?;
            let same = |ctor: &Self, e: usize| match (&ctor.objs[e], &ctor.objs[key]) {
                (Obj::Done(a), Obj::Done(b)) => a.key_eq(b),
                _ => e == key,
            };
            match out.iter().position(|(e, _)| same(self, *e)) {
                Some(i) => out[i].1 = val, // dict keeps the first key object, takes the last value
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
        let Kind::Seq(items) = &self.nodes[id].kind else {
            return err(ctx);
        };
        let mut out = Vec::new();
        for s in items.clone() {
            let Kind::Map(p) = &self.nodes[s].kind else {
                return err(ctx);
            };
            let [(k, v)] = p.as_slice() else {
                return err(ctx);
            };
            let (k, v) = (*k, *v);
            let pair = vec![self.object(k)?, self.object(v)?];
            out.push(self.push(Obj::Tuple(pair)));
        }
        Ok(Obj::List(out))
    }

    /// `construct_document`: build the root, then drain generator rounds.
    fn document(&mut self, root: usize) -> Result<usize, Fail> {
        let o = self.object(root)?;
        while !self.pending.is_empty() {
            for (o, id) in std::mem::take(&mut self.pending) {
                self.fill(o, id)?;
            }
        }
        Ok(o)
    }

    /// Arena object -> owned value. A cycle (alias to an enclosing container) is reported as the
    /// RecursionError `vault.flat` would hit on it.
    fn value(&self, o: usize, open: &mut Vec<usize>) -> Result<Py, Fail> {
        if open.contains(&o) {
            return Err(Fail::Crash("RecursionError"));
        }
        open.push(o);
        let all = |ids: &[usize], open: &mut Vec<usize>| {
            ids.iter()
                .map(|&i| self.value(i, open))
                .collect::<Result<Vec<_>, _>>()
        };
        let v = match &self.objs[o] {
            Obj::Done(p) => p.clone(),
            Obj::Pending => unreachable!("every generator ran before values are read"),
            Obj::List(ids) => Py::List(all(ids, open)?),
            Obj::Tuple(ids) => Py::Tuple(all(ids, open)?),
            Obj::Set(ids) => Py::Set(all(ids, open)?),
            Obj::Dict(pairs) => Py::Dict(
                pairs
                    .iter()
                    .map(|&(k, v)| Ok((self.value(k, open)?, self.value(v, open)?)))
                    .collect::<Result<_, Fail>>()?,
            ),
        };
        open.pop();
        Ok(v)
    }
}

/// Python `int(text, base)` for the strings SafeConstructor passes (underscores already removed).
fn py_int(text: &str, base: u32) -> Result<String, Fail> {
    let bad = || {
        err(format!(
            "invalid literal for int() with base {base}: {}",
            str_repr(text)
        ))
    };
    let t = text.trim();
    let (neg, t) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let prefix = match base {
        2 => "0b",
        8 => "0o",
        16 => "0x",
        _ => "",
    };
    let t = if !prefix.is_empty() && t.to_ascii_lowercase().starts_with(prefix) {
        &t[2..]
    } else {
        t
    };
    if t.is_empty() || !t.chars().all(|c| c.is_digit(base)) {
        return bad();
    }
    let mag = if base == 10 {
        let s = t.trim_start_matches('0');
        if s.is_empty() {
            "0".to_string()
        } else {
            s.to_string()
        }
    } else {
        match u128::from_str_radix(t, base) {
            Ok(n) => n.to_string(),
            Err(_) => return err("probe: integer wider than 128 bits"),
        }
    };
    Ok(if neg && mag != "0" {
        format!("-{mag}")
    } else {
        mag
    })
}

fn construct_int(raw: &str) -> Result<Py, Fail> {
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
    let mag = if v == "0" {
        return Ok(Py::Int("0".into()));
    } else if let Some(b) = v.strip_prefix("0b") {
        py_int(b, 2)?
    } else if let Some(h) = v.strip_prefix("0x") {
        py_int(h, 16)?
    } else if v.starts_with('0') {
        py_int(v, 8)?
    } else if v.contains(':') {
        let mut total: i128 = 0;
        for part in v.split(':') {
            let d: i128 = py_int(part, 10)?
                .parse()
                .map_err(|_| Fail::Error("probe: integer wider than 128 bits".into()))?;
            total = total
                .checked_mul(60)
                .and_then(|t| t.checked_add(d))
                .ok_or_else(|| Fail::Error("probe: integer wider than 128 bits".into()))?;
        }
        total.to_string()
    } else {
        py_int(v, 10)?
    };
    Ok(Py::Int(match (neg, mag.strip_prefix('-')) {
        (false, _) => mag,
        (true, Some(pos)) => pos.to_string(),
        (true, None) if mag == "0" => mag,
        (true, None) => format!("-{mag}"),
    }))
}

/// Python `float(text)`.
fn py_float(text: &str) -> Result<f64, Fail> {
    let t = text.trim();
    let lower = t.to_ascii_lowercase();
    let body = lower.trim_start_matches(['+', '-']);
    let ok_word = matches!(body, "inf" | "infinity" | "nan");
    let ok_num = !body.is_empty()
        && body
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | '+' | '-'))
        && body.chars().any(|c| c.is_ascii_digit());
    match (ok_word || ok_num).then(|| t.parse::<f64>()) {
        Some(Ok(f)) => Ok(f),
        _ => err(format!(
            "could not convert string to float: {}",
            str_repr(text)
        )),
    }
}

fn construct_float(raw: &str) -> Result<Py, Fail> {
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
    Ok(Py::Float(if v == ".inf" {
        sign * f64::INFINITY
    } else if v == ".nan" {
        f64::NAN
    } else if v.contains(':') {
        // Same summation order as PyYAML (least significant part first), so rounding matches.
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

fn construct_binary(raw: &str) -> Result<Py, Fail> {
    if !raw.is_ascii() {
        return err("probe: failed to convert base64 data into ascii");
    }
    // binascii.a2b_base64 (non-strict): ignore characters outside the alphabet, stop at padding.
    let mut bits = 0u32;
    let mut nbits = 0;
    let mut count = 0usize;
    let mut out = Vec::new();
    let mut padded = false;
    for c in raw.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => {
                padded = true;
                break;
            }
            _ => continue,
        };
        count += 1;
        bits = (bits << 6) | u32::from(v);
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push((bits >> nbits) as u8);
            bits &= (1 << nbits) - 1;
        }
    }
    match count % 4 {
        0 => {}
        1 => return err("probe: failed to decode base64 data (one extra character)"),
        _ if !padded => return err("failed to decode base64 data: Incorrect padding"),
        _ => {}
    }
    Ok(Py::Bytes(out))
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn check_date(y: i64, m: i64, d: i64) -> Result<(), Fail> {
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
    Ok(())
}

fn construct_timestamp(raw: &str) -> Result<Py, Fail> {
    let Some(c) = R_TIMESTAMP.captures(raw) else {
        return Err(Fail::Crash("AttributeError"));
    };
    let num = |n: &str| -> i64 {
        c.name(n)
            .map_or(0, |m| m.as_str().parse().expect("regex guarantees digits"))
    };
    let (y, m, d) = (num("year"), num("month"), num("day"));
    if c.name("hour").is_none() {
        check_date(y, m, d)?;
        return Ok(Py::Date(y, m, d));
    }
    let micro = c
        .name("fraction")
        .map(|f| f.as_str())
        .filter(|f| !f.is_empty())
        .map_or(0, |f| {
            format!("{:0<6}", &f[..f.len().min(6)])
                .parse()
                .expect("six digits")
        });
    let tz = if let Some(sign) = c.name("tz_sign") {
        let secs = num("tz_hour") * 3600 + num("tz_minute") * 60;
        let secs = if sign.as_str() == "-" { -secs } else { secs };
        if secs.abs() >= 86_400 {
            return err(format!(
                "offset must be a timedelta strictly between -timedelta(hours=24) and timedelta(hours=24), not {}.",
                timedelta_repr(secs)
            ));
        }
        Some(if secs == 0 { Tz::Utc } else { Tz::Offset(secs) })
    } else {
        c.name("tz").map(|_| Tz::Utc)
    };
    check_date(y, m, d)?;
    let (hour, minute, second) = (num("hour"), num("minute"), num("second"));
    for (name, v, hi) in [
        ("hour", hour, 23),
        ("minute", minute, 59),
        ("second", second, 59),
    ] {
        if !(0..=hi).contains(&v) {
            return err(format!("{name} must be in 0..{hi}, not {v}"));
        }
    }
    Ok(Py::DateTime(DateTime {
        year: y,
        month: m,
        day: d,
        hour,
        minute,
        second,
        micro,
        tz,
    }))
}

/// `yaml.load(text, Loader=vault.LOADER)`: `Ok(None)` for an empty stream.
pub fn load(src: &mut dyn EventSource) -> Result<Option<Py>, Fail> {
    let mut c = Composer {
        src,
        nodes: Vec::new(),
        anchors: HashMap::new(),
    };
    let root = match c.next()? {
        Ev::StreamEnd => None,
        Ev::DocStart => {
            let ev = c.next()?;
            let root = c.compose(ev)?;
            match c.next()? {
                Ev::DocEnd => {}
                other => return err(format!("probe: expected document end, got {other:?}")),
            }
            c.anchors.clear();
            Some(root)
        }
        other => return err(format!("probe: expected document start, got {other:?}")),
    };
    if root.is_some() && !matches!(c.next()?, Ev::StreamEnd) {
        return err("expected a single document in the stream");
    }
    let Some(root) = root else { return Ok(None) };
    let mut ctor = Ctor {
        nodes: c.nodes,
        objs: Vec::new(),
        cache: HashMap::new(),
        pending: Vec::new(),
    };
    let o = ctor.document(root)?;
    ctor.value(o, &mut Vec::new()).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

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
            assert_eq!(resolve_plain(v), t, "{v:?}");
        }
    }

    #[test]
    fn int_constructor_matches_python() {
        let int = |s: &str| match construct_int(s) {
            Ok(Py::Int(v)) => v,
            other => panic!("{other:?}"),
        };
        assert_eq!(int("010"), "8");
        assert_eq!(int("-0"), "0");
        assert_eq!(int("-0b1_0"), "-2");
        assert_eq!(int("190:20:30"), "685230");
        assert_eq!(int("0x1F"), "31");
        assert_eq!(
            int("123456789012345678901234567890"),
            "123456789012345678901234567890"
        );
        assert_eq!(
            construct_int("0x_").err(),
            Some(Fail::Error(
                "invalid literal for int() with base 16: ''".into()
            ))
        );
        assert_eq!(construct_int("").err(), Some(Fail::Crash("IndexError")));
    }
}
