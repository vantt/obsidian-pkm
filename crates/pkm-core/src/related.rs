//! `related`: note liên quan tới các hit tìm kiếm (của OHS), xếp bằng PageRank cá nhân hoá trên graph có loại.
//!
//! Đặc tả: `scripts/kg/related.py` (`load_adjacency`, `ppr_push`, `_via`, `related`) và `Tools.related` của
//! `scripts/kg/tools.py` ([`hits_from`]) ở sha ghim. Hạt giống nhận phần khởi đầu tỉ lệ với điểm tìm kiếm; phép đẩy
//! cục bộ (Andersen, Chung, Lang 2006) lan khối lượng theo cạnh có loại, cả hai chiều. Trọng số cạnh theo lớp
//! ([`WEIGHT`]), cạnh `proposed` nhân thêm `score` của cạnh khi trong (0, 1].
//!
//! Điểm phải khớp Python tới 1e-6 (docs/DECISIONS.md, "Thứ tự liệt kê file"), nên mọi phép tính số thực đi đúng thứ
//! tự của Python: cạnh đọc theo thứ tự SQLite trả (cùng câu SQL), bậc của node cộng bằng [`py_sum`] (`sum` của
//! Python 3.12+, có bù sai số), hàng đợi và thứ tự chèn `p` như `collections.deque` và `dict`, sắp ổn định.
//!
//! Module không gọi OHS: hạt giống do người gọi đưa vào (tool `related` lấy từ `hits`, hoặc từ `pkm_core::ohs` khi chỉ
//! có câu hỏi).

use std::collections::{HashMap, HashSet, VecDeque};

use indexmap::IndexMap;
use rusqlite::params_from_iter;
use rusqlite::types::Value as SqlValue;
use serde_json::{Map, Value, json};

use crate::query::{Filter, Graph, QueryError, Result, py};
use crate::vault::nfc;

/// Trọng số cạnh theo lớp (`status`); lớp khác không đi.
pub const WEIGHT: [(&str, f64); 3] = [("accepted", 1.0), ("superseded", 0.3), ("proposed", 0.5)];
/// Xác suất nhảy về hạt giống.
pub const ALPHA: f64 = 0.15;
/// Ngưỡng khối lượng dư (nhân bậc) để còn đẩy tiếp.
pub const EPS: f64 = 1e-6;
/// Số lần đẩy tối đa.
pub const MAX_PUSHES: u64 = 2_000_000;
/// Số note graph đưa tới mặc định.
pub const K: i64 = 15;
/// Độ sâu tối đa khi tìm `via`.
const MAX_HOPS: usize = 4;

fn weight(status: &str) -> f64 {
    WEIGHT
        .iter()
        .find(|(s, _)| *s == status)
        .map_or(0.0, |(_, w)| *w)
}

/// `sum` của Python 3.12+ trên số thực: cộng lần lượt kèm bù sai số Neumaier, cộng phần bù vào cuối khi hữu hạn.
pub fn py_sum(xs: impl IntoIterator<Item = f64>) -> f64 {
    let (mut f, mut c) = (0.0_f64, 0.0_f64);
    for x in xs {
        let t = f + x;
        if f.abs() >= x.abs() {
            c += (f - t) + x;
        } else {
            c += (x - t) + f;
        }
        f = t;
    }
    if c != 0.0 && c.is_finite() {
        f += c;
    }
    f
}

/// `round(x, ndigits)` của Python với số thực: làm tròn đúng theo giá trị nhị phân, hoà thì về số chẵn.
fn py_round(x: f64, ndigits: usize) -> f64 {
    if !x.is_finite() {
        return x;
    }
    format!("{x:.ndigits$}").parse().unwrap_or(x)
}

/// Một bước trên danh sách kề: láng giềng, trọng số, tên quan hệ theo hướng đi, lớp, claim.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub to: usize,
    pub weight: f64,
    pub rel: String,
    pub status: String,
    pub claim: Value,
}

/// Danh sách kề vô hướng có loại; node đánh số theo thứ tự gặp.
#[derive(Debug, Default)]
pub struct Adjacency {
    ids: Vec<String>,
    index: HashMap<String, usize>,
    steps: Vec<Vec<Step>>,
}

impl Adjacency {
    /// Số của node, thêm node mới (không cạnh) khi chưa có.
    fn intern(&mut self, id: &str) -> usize {
        if let Some(&i) = self.index.get(id) {
            return i;
        }
        let i = self.ids.len();
        self.ids.push(id.to_owned());
        self.index.insert(id.to_owned(), i);
        self.steps.push(Vec::new());
        i
    }

    pub fn id(&self, i: usize) -> &str {
        &self.ids[i]
    }

    pub fn steps(&self, i: usize) -> &[Step] {
        &self.steps[i]
    }

    /// `len(adj.get(x, ()))`.
    pub fn degree_of(&self, id: &str) -> usize {
        self.index.get(id).map_or(0, |&i| self.steps[i].len())
    }
}

/// Truthiness của một ô SQLite.
fn sql_truthy(v: &SqlValue) -> bool {
    match v {
        SqlValue::Null => false,
        SqlValue::Integer(i) => *i != 0,
        SqlValue::Real(x) => *x != 0.0,
        SqlValue::Text(s) => !s.is_empty(),
        SqlValue::Blob(b) => !b.is_empty(),
    }
}

fn sql_text(v: &SqlValue) -> String {
    py::str_of(&py::sql_to_json(v.clone()))
}

/// `load_adjacency`: danh sách kề vô hướng của các cạnh qua bộ lọc, bỏ vòng tự thân, note hệ thống (trừ khi
/// `system`), cạnh trọng số 0. Chiều ngược mang tên quan hệ nghịch (`inverse`, hoặc `~rel`), quan hệ đối xứng giữ tên.
pub fn load_adjacency(g: &Graph, f: &Filter) -> Result<Adjacency> {
    let mut params = Vec::new();
    let w = g.edge_where(f, &mut params);
    let sql = format!(
        "SELECT e.src, e.rel, e.dst, e.status, e.claim, e.score, x.inverse, x.symmetric FROM edges e LEFT JOIN schema_edge x ON x.key = e.rel WHERE {w}"
    );
    let hide = !g.hidden().is_empty() && !f.system;
    let mut adj = Adjacency::default();
    let mut st = g.conn().prepare(&sql)?;
    let mut rows = st.query(params_from_iter(params.iter()))?;
    while let Some(r) = rows.next()? {
        let cell = |i: usize| r.get::<_, SqlValue>(i);
        let (src, rel, dst) = (
            sql_text(&cell(0)?),
            sql_text(&cell(1)?),
            sql_text(&cell(2)?),
        );
        if src == dst || (hide && (g.is_hidden(&src) || g.is_hidden(&dst))) {
            continue;
        }
        let status = match cell(3)? {
            SqlValue::Text(s) => s,
            _ => continue,
        };
        let mut wt = weight(&status);
        if status == "proposed" {
            let score = match cell(5)? {
                SqlValue::Integer(i) => Some(i as f64),
                SqlValue::Real(x) => Some(x),
                _ => None,
            };
            if let Some(s) = score.filter(|s| 0.0 < *s && *s <= 1.0) {
                wt *= s;
            }
        }
        if wt <= 0.0 {
            continue;
        }
        let claim = py::sql_to_json(cell(4)?);
        let inverse = cell(6)?;
        let back = if sql_truthy(&cell(7)?) {
            rel.clone()
        } else if sql_truthy(&inverse) {
            sql_text(&inverse)
        } else {
            format!("~{rel}")
        };
        let (s, d) = (adj.intern(&src), adj.intern(&dst));
        adj.steps[s].push(Step {
            to: d,
            weight: wt,
            rel,
            status: status.clone(),
            claim: claim.clone(),
        });
        adj.steps[d].push(Step {
            to: s,
            weight: wt,
            rel: back,
            status,
            claim,
        });
    }
    Ok(adj)
}

/// Kết quả của [`ppr_push`]: khối lượng đã dừng của từng node theo thứ tự node được ghi lần đầu (như `dict` của
/// Python), và số lần đẩy.
pub struct Ppr {
    pub p: IndexMap<String, f64>,
    pub pushes: u64,
}

/// `ppr_push`: PageRank cá nhân hoá xấp xỉ (lazy random walk, đẩy ACL). `seeds`: node và khối lượng (tổng 1) theo
/// thứ tự hit.
pub fn ppr_push(
    adj: &Adjacency,
    seeds: &IndexMap<String, f64>,
    alpha: f64,
    eps: f64,
    max_pushes: u64,
) -> Ppr {
    // Hạt giống không có cạnh nằm ngoài danh sách kề: đánh số tiếp theo, bậc 0.
    let mut ids: Vec<String> = adj.ids.clone();
    let mut index = adj.index.clone();
    for s in seeds.keys() {
        if !index.contains_key(s) {
            index.insert(s.clone(), ids.len());
            ids.push(s.clone());
        }
    }
    let n = ids.len();
    let deg: Vec<f64> = (0..n)
        .map(|i| {
            adj.steps
                .get(i)
                .map_or(0.0, |st| py_sum(st.iter().map(|x| x.weight)))
        })
        .collect();
    let mut p: IndexMap<usize, f64> = IndexMap::new();
    let mut r = vec![0.0_f64; n];
    for (s, w) in seeds {
        r[index[s]] = *w;
    }
    let mut queue: VecDeque<usize> = seeds
        .keys()
        .map(|s| index[s])
        .filter(|&u| r[u] > eps * deg[u].max(1e-12))
        .collect();
    let mut inq: HashSet<usize> = queue.iter().copied().collect();
    let mut pushes = 0;
    while pushes < max_pushes {
        let Some(u) = queue.pop_front() else { break };
        inq.remove(&u);
        let du = deg[u];
        if du == 0.0 {
            *p.entry(u).or_insert(0.0) += r[u];
            r[u] = 0.0;
            continue;
        }
        let ru = r[u];
        if ru <= eps * du {
            continue;
        }
        pushes += 1;
        *p.entry(u).or_insert(0.0) += alpha * ru;
        r[u] = (1.0 - alpha) * ru / 2.0;
        let spread = (1.0 - alpha) * ru / 2.0 / du;
        if r[u] > eps * du && !inq.contains(&u) {
            queue.push_back(u);
            inq.insert(u);
        }
        for x in adj.steps(u) {
            let v = x.to;
            r[v] += spread * x.weight;
            if !inq.contains(&v) && r[v] > eps * deg[v] {
                queue.push_back(v);
                inq.insert(v);
            }
        }
    }
    Ppr {
        p: p.into_iter().map(|(i, x)| (ids[i].clone(), x)).collect(),
        pushes,
    }
}

/// `_via`: BFS nhiều nguồn từ các hạt giống, cạnh nặng trước khi cùng độ sâu; đường ngắn nhất (tối đa
/// [`MAX_HOPS`] bước) tới từng note đích, mỗi bước `{from, rel, to, status[, claim]}`.
fn via(adj: &Adjacency, seeds: &[String], targets: &[String]) -> HashMap<String, Vec<Value>> {
    // `parent[v] = (u, bước)`; hạt giống có mặt với `None`.
    let mut parent: HashMap<usize, Option<(usize, &Step)>> = HashMap::new();
    let mut frontier: Vec<usize> = Vec::new();
    for s in seeds {
        if let Some(&i) = adj.index.get(s) {
            parent.insert(i, None);
            frontier.push(i);
        }
    }
    let seed_set: HashSet<&String> = seeds.iter().collect();
    let mut need: HashSet<usize> = targets
        .iter()
        .filter(|t| !seed_set.contains(t))
        .filter_map(|t| adj.index.get(t).copied())
        .collect();
    // Đích không có cạnh không bao giờ tới được, nhưng Python vẫn giữ nó trong `need` (BFS chạy hết độ sâu).
    let unreachable = targets
        .iter()
        .any(|t| !seed_set.contains(t) && !adj.index.contains_key(t));
    let mut depth = 0;
    while !frontier.is_empty() && (!need.is_empty() || unreachable) && depth < MAX_HOPS {
        let mut next = Vec::new();
        for &u in &frontier {
            let mut steps: Vec<&Step> = adj.steps(u).iter().collect();
            steps.sort_by(|a, b| b.weight.total_cmp(&a.weight));
            for x in steps {
                if let std::collections::hash_map::Entry::Vacant(e) = parent.entry(x.to) {
                    e.insert(Some((u, x)));
                    next.push(x.to);
                    need.remove(&x.to);
                }
            }
        }
        frontier = next;
        depth += 1;
    }
    let mut out = HashMap::new();
    for t in targets {
        let mut steps = Vec::new();
        let mut cur = adj.index.get(t).copied();
        while let Some(c) = cur {
            let Some(Some((u, x))) = parent.get(&c) else {
                break;
            };
            let mut s = Map::new();
            s.insert("from".into(), adj.id(*u).into());
            s.insert("rel".into(), x.rel.clone().into());
            s.insert("to".into(), adj.id(c).into());
            s.insert("status".into(), x.status.clone().into());
            if py::truthy(&x.claim) {
                s.insert("claim".into(), x.claim.clone());
            }
            steps.push(Value::Object(s));
            cur = Some(*u);
        }
        steps.reverse();
        out.insert(t.clone(), steps);
    }
    out
}

/// Tham số của [`related`] ngoài hạt giống.
#[derive(Debug, Clone)]
pub struct Options {
    /// Số note graph đưa tới tối đa (`xs[:k]` như Python: số âm tính từ cuối).
    pub k: i64,
    pub filter: Filter,
    /// `types` như Python nhận: rỗng hay `null` là mọi type; danh sách (type của note phải là một phần tử); chuỗi
    /// (`in` của chuỗi con); object (khoá).
    pub types: Value,
    pub alpha: f64,
    pub eps: f64,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            k: K,
            filter: Filter::new(None, None, None, false, false),
            types: Value::Null,
            alpha: ALPHA,
            eps: EPS,
        }
    }
}

fn type_error(msg: impl Into<String>) -> QueryError {
    QueryError::new("TypeError", msg)
}

/// `info.get(x) in types`.
fn type_in(types: &Value, t: Option<&String>) -> Result<bool> {
    match types {
        Value::Array(a) => Ok(a.iter().any(|e| match (e, t) {
            (Value::String(s), Some(t)) => s == t,
            (Value::Null, None) => true,
            _ => false,
        })),
        Value::Object(m) => Ok(t.is_some_and(|t| m.contains_key(t))),
        Value::String(s) => match t {
            Some(t) => Ok(s.contains(t.as_str())),
            None => Err(type_error(
                "'in <string>' requires string as left operand, not NoneType",
            )),
        },
        v => Err(type_error(format!(
            "argument of type '{}' is not iterable",
            py::type_name(v)
        ))),
    }
}

/// `float(score or 0)`.
fn py_float(v: &Value) -> Result<f64> {
    if !py::truthy(v) {
        return Ok(0.0);
    }
    match v {
        Value::Bool(b) => Ok(f64::from(u8::from(*b))),
        Value::Number(n) => Ok(n.as_f64().unwrap_or(0.0)),
        Value::String(s) => crate::index::py_float(s).ok_or_else(|| {
            QueryError::new(
                "ValueError",
                format!("could not convert string to float: {}", py::repr(v)),
            )
        }),
        v => Err(type_error(format!(
            "float() argument must be a string or a real number, not '{}'",
            py::type_name(v)
        ))),
    }
}

/// Phần tử của một giá trị khi Python lặp `for h in hits` (danh sách; chuỗi thành từng ký tự; object thành khoá).
fn py_iter(v: &Value) -> Result<Vec<Value>> {
    match v {
        Value::Array(a) => Ok(a.clone()),
        Value::String(s) => Ok(s.chars().map(|c| Value::String(c.into())).collect()),
        Value::Object(m) => Ok(m.keys().map(|k| Value::String(k.clone())).collect()),
        v => Err(type_error(format!(
            "'{}' object is not iterable",
            py::type_name(v)
        ))),
    }
}

/// `Tools.related`: hit dạng chuỗi (đường dẫn), object (`path` hoặc `id`, và `score`) hoặc danh sách
/// `[đường dẫn, điểm]`, thành cặp (đường dẫn, điểm) theo thứ tự; bỏ hit không có đường dẫn. Giá trị Python giữ
/// nguyên để [`related`] đọc như Python. `hits` rỗng hay `null` là không hit nào.
pub fn hits_from(hits: &Value) -> Result<Vec<(Value, Value)>> {
    if !py::truthy(hits) {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for h in py_iter(hits)? {
        let pair = match &h {
            Value::String(_) => (h.clone(), Value::Null),
            Value::Object(m) => {
                let path = m
                    .get("path")
                    .filter(|p| py::truthy(p))
                    .or_else(|| m.get("id"));
                (
                    path.cloned().unwrap_or(Value::Null),
                    m.get("score").cloned().unwrap_or(Value::Null),
                )
            }
            Value::Array(a) => match a.first() {
                Some(p) => (p.clone(), a.get(1).cloned().unwrap_or(Value::Null)),
                None => {
                    return Err(QueryError::new("IndexError", "list index out of range"));
                }
            },
            v => {
                return Err(type_error(format!(
                    "'{}' object is not subscriptable",
                    py::type_name(v)
                )));
            }
        };
        if py::truthy(&pair.0) {
            out.push(pair);
        }
    }
    Ok(out)
}

/// `related`: `hits` là cặp (đường dẫn, điểm) theo thứ tự tìm kiếm trả (xem [`hits_from`]). Trả object `seeds`
/// (hạt giống xếp lại theo điểm graph, `found_by: "ohs"`), `related` (note chỉ graph đưa tới, `found_by: "graph"`,
/// kèm `via`), `missing` (hit không có trong index) và `method` (khi có hạt giống).
pub fn related(g: &Graph, hits: &[(Value, Value)], o: &Options) -> Result<Map<String, Value>> {
    let mut missing: Vec<String> = Vec::new();
    let mut rank: HashMap<String, usize> = HashMap::new();
    let mut seed_w: IndexMap<String, f64> = IndexMap::new();
    for (i, (path, score)) in hits.iter().enumerate() {
        let Value::String(path) = path else {
            return Err(type_error(format!(
                "normalize() argument 2 must be str, not {}",
                py::type_name(path)
            )));
        };
        let path = nfc(path);
        if rank.contains_key(&path) || missing.contains(&path) {
            continue;
        }
        if !g.has_node(&path)? {
            missing.push(path);
            continue;
        }
        rank.insert(path.clone(), i + 1);
        let x = py_float(score)?;
        // `max(x, 0.0) or 1.0 / (i + 1)`: không có điểm thì theo thứ tự.
        let w = if 0.0 > x { 0.0 } else { x };
        let w = if w == 0.0 { 1.0 / (i + 1) as f64 } else { w };
        seed_w.insert(path, w);
    }
    let missing_v = Value::from(missing);
    if seed_w.is_empty() {
        let mut out = Map::new();
        out.insert("seeds".into(), json!([]));
        out.insert("related".into(), json!([]));
        out.insert("missing".into(), missing_v);
        return Ok(out);
    }
    let tot = py_sum(seed_w.values().copied());
    let seeds: IndexMap<String, f64> = seed_w.into_iter().map(|(p, w)| (p, w / tot)).collect();
    let adj = load_adjacency(g, &o.filter)?;
    let ppr = ppr_push(&adj, &seeds, o.alpha, o.eps, MAX_PUSHES);
    let p = &ppr.p;
    let mut ids: Vec<String> = p.keys().cloned().collect();
    // `sorted(p, key=lambda x: -p[x])`: ổn định, giữ thứ tự chèn khi bằng nhau.
    ids.sort_by(|a, b| p[b].partial_cmp(&p[a]).unwrap_or(std::cmp::Ordering::Equal));
    let mut ask = py::head(&ids, o.k.saturating_mul(4).max(200));
    ask.extend(seeds.keys().cloned());
    let info = g.types_of(&ask)?;
    let type_of = |x: &str| info.get(x).cloned().flatten();
    let pval = |x: &str| p.get(x).copied();

    let mut seed_rows: Vec<String> = seeds.keys().cloned().collect();
    seed_rows.sort_by(|a, b| {
        let (pa, pb) = (pval(a).unwrap_or(0.0), pval(b).unwrap_or(0.0));
        pb.partial_cmp(&pa).unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut graph_rows = Vec::new();
    for x in &ids {
        if seeds.contains_key(x) {
            continue;
        }
        if !py::truthy(&o.types) || type_in(&o.types, type_of(x).as_ref())? {
            graph_rows.push(x.clone());
        }
    }
    let graph_rows = py::head(&graph_rows, o.k);
    let seed_list: Vec<String> = seeds.keys().cloned().collect();
    let mut via = via(&adj, &seed_list, &graph_rows);

    let row = |x: &str, via: &mut HashMap<String, Vec<Value>>| -> Value {
        let mut d = Map::new();
        d.insert("id".into(), x.into());
        d.insert("type".into(), json!(type_of(x)));
        // `round(p.get(x, 0), 5)`: note chưa có khối lượng ra số nguyên 0 như Python.
        d.insert(
            "score".into(),
            pval(x).map_or(json!(0), |s| json!(py_round(s, 5))),
        );
        if seeds.contains_key(x) {
            d.insert("found_by".into(), "ohs".into());
            d.insert("ohs_rank".into(), json!(rank[x]));
            d.insert("degree".into(), json!(adj.degree_of(x)));
        } else {
            let steps = via.remove(x).unwrap_or_default();
            let proposed = steps.iter().any(|s| s["status"] == "proposed");
            d.insert("found_by".into(), "graph".into());
            d.insert(
                "layer".into(),
                (if proposed { "proposed" } else { "accepted" }).into(),
            );
            d.insert("via".into(), Value::Array(steps));
        }
        Value::Object(d)
    };
    let seed_out: Vec<Value> = seed_rows.iter().map(|x| row(x, &mut via)).collect();
    let graph_out: Vec<Value> = graph_rows.iter().map(|x| row(x, &mut via)).collect();
    let mut out = Map::new();
    out.insert("seeds".into(), Value::Array(seed_out));
    out.insert("related".into(), Value::Array(graph_out));
    out.insert("missing".into(), missing_v);
    out.insert(
        "method".into(),
        json!({"alpha": o.alpha, "eps": o.eps, "pushes": ppr.pushes, "touched": p.len()}),
    );
    Ok(out)
}

#[cfg(test)]
mod tests;
