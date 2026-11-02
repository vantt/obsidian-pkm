//! Câu có đáp án của vault kg-solo (obsidian-test-fixture, `.kg-answers.json`) chạy trên [`Graph`].
//!
//! Đặc tả: `scripts/kg/tests/fixture_check.py` ở sha ghim. Phép graph (`neighbors`, `reach`, `paths`, `cycles`,
//! `chain`, `provenance`: 70 câu ở fixture v0.7.0) cộng các phép bảng `find`, `select`, `valid`. Mặc định status
//! của fixture là `accepted` và `superseded`, khác mặc định của bộ tool (thêm `proposed`), nên câu không ghi
//! `statuses` được chạy với [`ACCEPTED`].

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{Map, Value, json};

use super::{ACCEPTED, Dir, Filter, Graph, QueryError, Result, py};

/// Phép graph của bộ câu.
pub const GRAPH_OPS: [&str; 6] = [
    "neighbors",
    "reach",
    "paths",
    "cycles",
    "chain",
    "provenance",
];
/// Phép bảng của bộ câu.
pub const TABLE_OPS: [&str; 3] = ["find", "select", "valid"];

fn key_error(k: &str) -> QueryError {
    QueryError::new("KeyError", py::repr(&Value::String(k.to_owned())))
}

/// `q[k]`.
fn req<'a>(q: &'a Value, k: &str) -> Result<&'a Value> {
    q.get(k).ok_or_else(|| key_error(k))
}

fn string(v: &Value, what: &str) -> Result<String> {
    match v {
        Value::String(s) => Ok(s.clone()),
        _ => Err(QueryError::new(
            "TypeError",
            format!("{what} phải là chuỗi, không phải {}", py::type_name(v)),
        )),
    }
}

fn opt_string(v: Option<&Value>, what: &str) -> Result<Option<String>> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(v) => string(v, what).map(Some),
    }
}

fn strings(v: Option<&Value>, what: &str) -> Result<Option<Vec<String>>> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| string(x, what))
            .collect::<Result<_>>()
            .map(Some),
        Some(v) => Err(QueryError::new(
            "TypeError",
            format!(
                "{what} phải là danh sách chuỗi, không phải {}",
                py::type_name(v)
            ),
        )),
    }
}

fn number(v: Option<&Value>, default: i64, what: &str) -> Result<i64> {
    match v {
        None => Ok(default),
        Some(v) => py::int(v).map_err(|(k, m)| QueryError::new(k, format!("{what}: {m}"))),
    }
}

fn dir(v: Option<&Value>) -> Dir {
    v.and_then(Value::as_str).map_or(Dir::Both, Dir::parse)
}

/// `run(g, q)`: kết quả một câu, đúng dạng `fixture_check.run` trả.
pub fn run(g: &Graph, q: &Value) -> Result<Value> {
    let op = string(req(q, "op")?, "op")?;
    let st: Vec<String> = match strings(q.get("statuses"), "statuses")? {
        Some(s) if !s.is_empty() => s,
        _ => ACCEPTED.iter().map(|s| (*s).to_owned()).collect(),
    };
    let rels = strings(q.get("rels"), "rels")?;
    let as_of = opt_string(q.get("asOf"), "asOf")?;
    let f = Filter::new(rels.as_deref(), as_of.as_deref(), Some(&st), false, false);
    Ok(match op.as_str() {
        "neighbors" => {
            let node = string(req(q, "node")?, "node")?;
            let d = match q.get("dir") {
                None => Dir::Both,
                v => dir(v),
            };
            let pairs: BTreeSet<(String, String)> = g
                .neighbors(&node, d, &f)?
                .iter()
                .map(|x| (py::str_of(&x["rel"]), py::str_of(&x["node"])))
                .collect();
            Value::Array(
                pairs
                    .into_iter()
                    .map(|(r, n)| json!({"rel": r, "node": n}))
                    .collect(),
            )
        }
        "reach" => {
            let node = string(req(q, "node")?, "node")?;
            let d = match q.get("dir") {
                None => Dir::Out,
                v => dir(v),
            };
            let depth = match q.get("maxDepth") {
                None | Some(Value::Null) => None,
                v => Some(number(v, 0, "maxDepth")?),
            };
            g.reach(&node, d, depth, &f)?
                .into_iter()
                .map(|(i, _)| Value::String(i))
                .collect()
        }
        "paths" => {
            let from = string(req(q, "from")?, "from")?;
            let to = string(req(q, "to")?, "to")?;
            let max_len = number(q.get("maxLen"), 6, "maxLen")?;
            g.paths(&from, &to, max_len, &f, None)?
                .into_iter()
                .map(|p| Value::Object(pick(&p, &["nodes", "rels", "sign"])))
                .collect()
        }
        "cycles" => {
            let max_len = number(q.get("maxLen"), 10, "maxLen")?;
            match q.get("through").filter(|t| py::truthy(t)) {
                Some(t) => {
                    let through = string(t, "through")?;
                    let mut out = Vec::new();
                    // Quay về id nhỏ nhất như lib/kg.mjs.
                    for c in g.loops(&through, max_len, &f)? {
                        let nodes = str_list(&c["nodes"]);
                        let rels = str_list(&c["rels"]);
                        let i = nodes
                            .iter()
                            .enumerate()
                            .min_by(|a, b| a.1.cmp(b.1))
                            .map_or(0, |(i, _)| i);
                        let rot = |xs: &[String]| -> Vec<String> {
                            xs[i.min(xs.len())..]
                                .iter()
                                .chain(&xs[..i.min(xs.len())])
                                .cloned()
                                .collect()
                        };
                        out.push(json!({"nodes": rot(&nodes), "rels": rot(&rels),
                                        "negatives": c["negatives"], "kind": c["kind"]}));
                    }
                    Value::Array(out)
                }
                None => g
                    .all_loops(max_len, &f)?
                    .into_iter()
                    .map(|c| Value::Object(pick(&c, &["nodes", "rels", "negatives", "kind"])))
                    .collect(),
            }
        }
        "chain" => {
            let start = string(req(q, "start")?, "start")?;
            let mut cur = vec![start.clone()];
            let steps = match req(q, "steps")? {
                Value::Array(a) => a.clone(),
                v => {
                    return Err(QueryError::new(
                        "TypeError",
                        format!("steps phải là danh sách, không phải {}", py::type_name(v)),
                    ));
                }
            };
            for s in &steps {
                let rel = string(req(s, "rel")?, "rel")?;
                let as_of = opt_string(s.get("asOf"), "asOf")?;
                let d = match s.get("dir") {
                    None => Dir::Both,
                    v => dir(v),
                };
                let acc: Vec<String> = ACCEPTED.iter().map(|x| (*x).to_owned()).collect();
                let sf = Filter::new(Some(&[rel]), as_of.as_deref(), Some(&acc), false, false);
                let mut nxt = BTreeSet::new();
                for n in &cur {
                    for x in g.neighbors(n, d, &sf)? {
                        nxt.insert(py::str_of(&x["node"]));
                    }
                }
                cur = nxt.into_iter().collect();
            }
            let exclude = strings(q.get("exclude"), "exclude")?.unwrap_or_default();
            cur.into_iter()
                .filter(|n| *n != start && !exclude.contains(n))
                .map(Value::String)
                .collect()
        }
        "provenance" => {
            let claim = string(req(q, "claim")?, "claim")?;
            let p = g.provenance(&claim)?;
            let sources: Vec<Value> = p["sources"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|s| json!({"note": s["note"], "block": s["block"]}))
                .collect();
            let evidence: Vec<Value> = p["evidence"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|e| json!({"rel": e["rel"], "from": e["from"]}))
                .collect();
            json!({"sources": sources, "evidence": evidence})
        }
        "find" => {
            let name = string(req(q, "name")?, "name")?;
            let t = opt_string(q.get("type"), "type")?;
            g.find(&name, t.as_deref(), None)?
                .into_iter()
                .map(|x| x["id"].clone())
                .collect()
        }
        "select" => {
            let w = match req(q, "where")? {
                Value::Object(m) => m.clone(),
                v => {
                    return Err(QueryError::new(
                        "TypeError",
                        format!("where phải là object, không phải {}", py::type_name(v)),
                    ));
                }
            };
            g.select(&w, false)?
                .into_iter()
                .map(Value::String)
                .collect()
        }
        "valid" => {
            let t = string(req(q, "type")?, "type")?;
            let a = string(req(q, "asOf")?, "asOf")?;
            g.valid(&t, &a, false)?
                .into_iter()
                .map(Value::String)
                .collect()
        }
        _ => return Err(key_error(&op)),
    })
}

fn pick(m: &Map<String, Value>, keys: &[&str]) -> Map<String, Value> {
    keys.iter()
        .map(|k| ((*k).to_owned(), m.get(*k).cloned().unwrap_or(Value::Null)))
        .collect()
}

fn str_list(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().map(py::str_of).collect()
}

/// `norm`: danh sách so không theo thứ tự; kết quả `provenance` (object có `sources`) thì từng danh sách bên
/// trong so không theo thứ tự.
pub fn norm(x: &Value) -> Value {
    match x {
        Value::Object(m) if m.contains_key("sources") => {
            Value::Object(m.iter().map(|(k, v)| (k.clone(), norm(v))).collect())
        }
        Value::Array(a) => {
            let mut s: Vec<String> = a.iter().map(py::json_dumps_sorted).collect();
            s.sort();
            Value::Array(s.into_iter().map(Value::String).collect())
        }
        _ => x.clone(),
    }
}

/// Câu sai đáp án hoặc lỗi khi chạy.
#[derive(Debug, Clone, PartialEq)]
pub struct Fail {
    pub case: String,
    pub q: usize,
    pub op: Option<String>,
    /// Lỗi khi chạy (`repr` của ngoại lệ), hoặc `None` khi chạy được nhưng sai đáp án.
    pub error: Option<String>,
    pub got: Option<Value>,
    pub expect: Option<Value>,
}

/// Kết quả [`check`]: `[đúng, tổng]` của câu graph và câu bảng, các câu sai.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Check {
    pub graph: [usize; 2],
    pub table: [usize; 2],
    pub fails: Vec<Fail>,
}

/// Lỗi đọc `.kg-answers.json`.
#[derive(Debug)]
pub enum CheckError {
    Read(std::io::Error),
    Json(serde_json::Error),
    Shape(&'static str),
    Query(QueryError),
}

impl std::fmt::Display for CheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckError::Read(e) => write!(f, "không đọc được .kg-answers.json: {e}"),
            CheckError::Json(e) => write!(f, ".kg-answers.json hỏng: {e}"),
            CheckError::Shape(m) => write!(f, ".kg-answers.json sai dạng: {m}"),
            CheckError::Query(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for CheckError {}

/// `check(vault, db)`: chạy mọi câu graph và bảng của `vault/.kg-answers.json` trên index `db`, so với đáp án.
pub fn check(vault: &Path, db: &Path) -> std::result::Result<Check, CheckError> {
    let text = std::fs::read_to_string(vault.join(".kg-answers.json")).map_err(CheckError::Read)?;
    let key: Value = serde_json::from_str(&text).map_err(CheckError::Json)?;
    let g = Graph::open(db).map_err(CheckError::Query)?;
    let cases = key
        .get("cases")
        .and_then(Value::as_array)
        .ok_or(CheckError::Shape("thiếu `cases`"))?;
    let mut res = Check::default();
    for c in cases {
        let id = c.get("id").map(py::str_of).unwrap_or_default();
        let multi = c.get("queries").filter(|q| py::truthy(q));
        let (qs, exp): (Vec<Value>, Vec<Value>) = match multi {
            Some(qs) => (
                qs.as_array()
                    .cloned()
                    .ok_or(CheckError::Shape("`queries` không phải danh sách"))?,
                c.get("expect")
                    .and_then(Value::as_array)
                    .cloned()
                    .ok_or(CheckError::Shape("`expect` không phải danh sách"))?,
            ),
            None => (
                vec![
                    c.get("query")
                        .cloned()
                        .ok_or(CheckError::Shape("thiếu `query`"))?,
                ],
                vec![c.get("expect").cloned().unwrap_or(Value::Null)],
            ),
        };
        for (i, (q, x)) in qs.iter().zip(exp.iter()).enumerate() {
            let op = q.get("op").and_then(Value::as_str).unwrap_or_default();
            let slot = if GRAPH_OPS.contains(&op) {
                &mut res.graph
            } else if TABLE_OPS.contains(&op) {
                &mut res.table
            } else {
                continue;
            };
            slot[1] += 1;
            match run(&g, q) {
                Err(e) => res.fails.push(Fail {
                    case: id.clone(),
                    q: i,
                    op: None,
                    error: Some(e.repr()),
                    got: None,
                    expect: None,
                }),
                Ok(got) if norm(&got) == norm(x) => slot[0] += 1,
                Ok(got) => res.fails.push(Fail {
                    case: id.clone(),
                    q: i,
                    op: Some(op.to_owned()),
                    error: None,
                    got: Some(got),
                    expect: Some(x.clone()),
                }),
            }
        }
    }
    Ok(res)
}
