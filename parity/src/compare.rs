//! So hai dump, từng tầng. Mục bên thật khai "chưa có" thì báo chưa có, không bao giờ tính là khớp;
//! mục bên thật thiếu mà không khai là lệch.

use std::collections::{BTreeMap, HashMap};

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};

use crate::dump::{CliCommand, Dump, Line};
use crate::exceptions::Exception;
use crate::jsonpos::{line_of, value_lines};
use crate::normalize::{self, FieldDiff, canonical};

/// Số dòng còn lại tối đa mỗi bên để thử ghép dòng gần giống (tránh O(n²) khi lệch hàng loạt).
const PAIRING_LIMIT: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Match,
    Diff,
    NotAvailable,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Match => "khớp",
            Status::Diff => "lệch",
            Status::NotAvailable => "chưa có",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Status::Match => "match",
            Status::Diff => "diff",
            Status::NotAvailable => "not_available",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    /// Có ở cả hai bên nhưng khác.
    Changed,
    /// Có trong dump mẫu, thiếu ở dump thật.
    Missing,
    /// Có ở dump thật, không có trong dump mẫu.
    Extra,
}

impl DiffKind {
    pub fn code(self) -> &'static str {
        match self {
            DiffKind::Changed => "changed",
            DiffKind::Missing => "missing",
            DiffKind::Extra => "extra",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diff {
    pub kind: DiffKind,
    /// Khoá dòng bảng, id câu, JSON pointer hay `dòng N`.
    pub unit: String,
    /// File trong dump mẫu (với `Extra`: file trong dump thật).
    pub file: String,
    /// Dòng trong dump mẫu.
    pub line: Option<usize>,
    /// Dòng trong dump thật.
    pub actual_line: Option<usize>,
    pub fields: Vec<FieldDiff>,
    /// Tên ngoại lệ đã tha chỗ lệch này.
    pub exception: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ItemResult {
    pub layer: &'static str,
    pub item: String,
    pub status: Status,
    /// Số đơn vị so trong dump mẫu (dòng bảng, câu, dòng văn bản, file).
    pub units: usize,
    pub diffs: Vec<Diff>,
}

impl ItemResult {
    pub fn unnamed(&self) -> usize {
        self.diffs.iter().filter(|d| d.exception.is_none()).count()
    }

    pub fn named(&self) -> usize {
        self.diffs.len() - self.unnamed()
    }
}

#[derive(Debug, Clone)]
pub struct PerfRow {
    pub metric: String,
    pub expected: Option<f64>,
    pub actual: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Comparison {
    pub items: Vec<ItemResult>,
    /// `None`: bên thật chưa có số đo.
    pub perf: Option<Vec<PerfRow>>,
    pub unused_exceptions: Vec<String>,
}

impl Comparison {
    pub fn unnamed(&self) -> usize {
        self.items.iter().map(ItemResult::unnamed).sum()
    }

    pub fn not_available(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.status == Status::NotAvailable)
            .count()
    }
}

/// So dump thật `actual` với dump mẫu `expected`.
pub fn compare(expected: &Dump, actual: &Dump, exceptions: &[Exception]) -> Result<Comparison> {
    let mut items = Vec::new();
    items.extend(index_layer(expected, actual)?);
    items.extend(records_layer(expected, actual, RecordLayer::ANSWERS)?);
    items.extend(records_layer(expected, actual, RecordLayer::TOOLS)?);
    items.extend(mcp_layer(expected, actual)?);
    items.extend(cli_layer(expected, actual)?);
    let mut used = vec![false; exceptions.len()];
    for item in &mut items {
        for d in &mut item.diffs {
            let paths: Vec<String> = d.fields.iter().map(|f| f.path.clone()).collect();
            if let Some(i) = exceptions
                .iter()
                .position(|e| e.covers(item.layer, &item.item, &paths))
            {
                used[i] = true;
                d.exception = Some(exceptions[i].name.clone());
            }
        }
        if item.status != Status::NotAvailable {
            item.status = if item.unnamed() > 0 {
                Status::Diff
            } else {
                Status::Match
            };
        }
    }
    let unused_exceptions = exceptions
        .iter()
        .zip(&used)
        .filter(|(_, u)| !**u)
        .map(|(e, _)| e.name.clone())
        .collect();
    Ok(Comparison {
        items,
        perf: perf(expected, actual)?,
        unused_exceptions,
    })
}

fn item(layer: &'static str, name: &str, units: usize, diffs: Vec<Diff>) -> ItemResult {
    ItemResult {
        layer,
        item: name.to_owned(),
        status: if diffs.is_empty() {
            Status::Match
        } else {
            Status::Diff
        },
        units,
        diffs,
    }
}

fn not_available(layer: &'static str, name: &str, units: usize) -> ItemResult {
    ItemResult {
        layer,
        item: name.to_owned(),
        status: Status::NotAvailable,
        units,
        diffs: Vec::new(),
    }
}

fn whole(kind: DiffKind, unit: &str, file: &str) -> Diff {
    Diff {
        kind,
        unit: unit.to_owned(),
        file: file.to_owned(),
        line: None,
        actual_line: None,
        fields: Vec::new(),
        exception: None,
    }
}

// ------------------------------------------------------------------------------------------ tầng index

fn index_layer(expected: &Dump, actual: &Dump) -> Result<Vec<ItemResult>> {
    let mut out = Vec::new();
    for (name, info) in &expected.manifest.tables {
        let file = format!("tables/{name}.jsonl");
        if actual.not_available("index", name) {
            out.push(not_available("index", name, info.rows));
            continue;
        }
        let exp_rows = expected.jsonl(&file)?;
        if !actual.exists(&file) {
            out.push(item(
                "index",
                name,
                exp_rows.len(),
                vec![whole(DiffKind::Missing, "bảng", &file)],
            ));
            continue;
        }
        let act_rows = actual.jsonl(&file)?;
        let mut diffs = Vec::new();
        if let Some(act_info) = actual.manifest.tables.get(name)
            && act_info.columns != info.columns
        {
            diffs.push(Diff {
                fields: vec![FieldDiff {
                    path: "/columns".into(),
                    expected: Some(Value::from(info.columns.clone())),
                    actual: Some(Value::from(act_info.columns.clone())),
                }],
                ..whole(DiffKind::Changed, "cột", "manifest.json")
            });
        }
        diffs.extend(table_rows(name, &file, &info.key, &exp_rows, &act_rows)?);
        out.push(item("index", name, exp_rows.len(), diffs));
    }
    for name in actual.manifest.tables.keys() {
        if !expected.manifest.tables.contains_key(name) {
            let file = format!("tables/{name}.jsonl");
            out.push(item(
                "index",
                name,
                0,
                vec![whole(DiffKind::Extra, "bảng", &file)],
            ));
        }
    }
    Ok(out)
}

struct Row {
    line: usize,
    map: Map<String, Value>,
    canon: String,
}

fn rows(table: &str, file: &str, lines: &[Line]) -> Result<Vec<Row>> {
    lines
        .iter()
        .map(|l| {
            let Value::Object(map) = &l.value else {
                bail!("{file}:{}: dòng bảng phải là object JSON", l.no);
            };
            let mut map = map.clone();
            normalize::table_row(table, &mut map);
            let canon = canonical(&Value::Object(map.clone()));
            Ok(Row {
                line: l.no,
                map,
                canon,
            })
        })
        .collect()
}

fn row_fields(table: &str, e: &Map<String, Value>, a: &Map<String, Value>) -> Vec<FieldDiff> {
    let mut out = Vec::new();
    for (col, ev) in e {
        let path = format!("/{}", crate::jsonpos::escape(col));
        match a.get(col) {
            Some(av) => normalize::diff(
                ev,
                av,
                &path,
                normalize::int_tolerance(table, col),
                &mut out,
            ),
            None => out.push(FieldDiff {
                path,
                expected: Some(ev.clone()),
                actual: None,
            }),
        }
    }
    for (col, av) in a {
        if !e.contains_key(col) {
            out.push(FieldDiff {
                path: format!("/{}", crate::jsonpos::escape(col)),
                expected: None,
                actual: Some(av.clone()),
            });
        }
    }
    out
}

fn key_of(map: &Map<String, Value>, key: &[String]) -> Option<String> {
    if key.is_empty() {
        return None;
    }
    let vals: Vec<Value> = key
        .iter()
        .map(|k| map.get(k).cloned().unwrap_or(Value::Null))
        .collect();
    Some(canonical(&Value::Array(vals)))
}

/// So dòng bảng như hai tập có lặp: khớp đúng từng chữ, rồi khớp theo dung sai, rồi ghép dòng lệch
/// theo khoá chính (hoặc theo số cột giống nhau) để báo đúng dòng và đúng cột.
fn table_rows(
    table: &str,
    file: &str,
    key: &[String],
    exp_lines: &[Line],
    act_lines: &[Line],
) -> Result<Vec<Diff>> {
    let exp = rows(table, file, exp_lines)?;
    let act = rows(table, file, act_lines)?;
    let mut by_canon: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, r) in act.iter().enumerate().rev() {
        by_canon.entry(r.canon.as_str()).or_default().push(i);
    }
    let mut act_used = vec![false; act.len()];
    let mut left_exp = Vec::new();
    for (i, r) in exp.iter().enumerate() {
        match by_canon.get_mut(r.canon.as_str()).and_then(Vec::pop) {
            Some(j) => act_used[j] = true,
            None => left_exp.push(i),
        }
    }
    let mut left_act: Vec<usize> = (0..act.len()).filter(|j| !act_used[*j]).collect();
    let mut diffs = Vec::new();
    let mut pairs: Vec<(usize, usize)> = Vec::new();

    // ghép theo khoá chính (cặp bằng nhau theo dung sai là khớp)
    if !key.is_empty() {
        let mut by_key: HashMap<String, Vec<usize>> = HashMap::new();
        for &j in left_act.iter().rev() {
            if let Some(k) = key_of(&act[j].map, key) {
                by_key.entry(k).or_default().push(j);
            }
        }
        let mut paired = std::collections::HashSet::new();
        left_exp.retain(|&i| {
            let found = key_of(&exp[i].map, key)
                .and_then(|k| by_key.get_mut(&k))
                .and_then(Vec::pop);
            match found {
                Some(j) => {
                    paired.insert(j);
                    pairs.push((i, j));
                    false
                }
                None => true,
            }
        });
        left_act.retain(|j| !paired.contains(j));
    }
    let small = left_exp.len() <= PAIRING_LIMIT && left_act.len() <= PAIRING_LIMIT;
    // bảng không khoá: khớp theo dung sai (số thực, mtime)
    if small {
        left_exp.retain(|&i| {
            match left_act
                .iter()
                .position(|&j| row_fields(table, &exp[i].map, &act[j].map).is_empty())
            {
                Some(p) => {
                    left_act.remove(p);
                    false
                }
                None => true,
            }
        });
    }
    // ghép theo số cột giống nhau (ít nhất một nửa)
    if small {
        left_exp.retain(|&i| {
            let ncols = exp[i].map.len().max(1);
            let best = left_act
                .iter()
                .enumerate()
                .map(|(p, &j)| {
                    let same = exp[i]
                        .map
                        .iter()
                        .filter(|(k, v)| act[j].map.get(*k) == Some(*v))
                        .count();
                    (p, same)
                })
                .filter(|(_, same)| same * 2 >= ncols)
                .max_by_key(|(p, same)| (*same, std::cmp::Reverse(*p)));
            match best {
                Some((p, _)) => {
                    pairs.push((i, left_act.remove(p)));
                    false
                }
                None => true,
            }
        });
    }
    pairs.sort_unstable();
    for (i, j) in pairs {
        let fields = row_fields(table, &exp[i].map, &act[j].map);
        if fields.is_empty() {
            continue;
        }
        diffs.push(Diff {
            kind: DiffKind::Changed,
            unit: row_unit(&exp[i], key),
            file: file.to_owned(),
            line: Some(exp[i].line),
            actual_line: Some(act[j].line),
            fields,
            exception: None,
        });
    }
    for i in left_exp {
        diffs.push(Diff {
            line: Some(exp[i].line),
            ..whole(DiffKind::Missing, &row_unit(&exp[i], key), file)
        });
    }
    for j in left_act {
        diffs.push(Diff {
            actual_line: Some(act[j].line),
            ..whole(DiffKind::Extra, &row_unit(&act[j], key), file)
        });
    }
    diffs.sort_by_key(|d| (d.line.unwrap_or(usize::MAX), d.actual_line));
    Ok(diffs)
}

fn row_unit(r: &Row, key: &[String]) -> String {
    match key_of(&r.map, key) {
        Some(k) => k,
        None => format!("dòng {}", r.line),
    }
}

// ------------------------------------------------------------------------------------------ câu đáp án, tool

/// Một tầng ghi mỗi câu một dòng JSONL có `id`.
#[derive(Clone, Copy)]
struct RecordLayer {
    layer: &'static str,
    file: &'static str,
    /// Trường nhóm câu thành mục (op của câu đáp án, tên tool).
    group: &'static str,
    normalize: fn(&str, &mut Value),
}

impl RecordLayer {
    const ANSWERS: RecordLayer = RecordLayer {
        layer: "answers",
        file: "answers/answers.jsonl",
        group: "op",
        normalize: |_, v| normalize::answer_result(v),
    };
    const TOOLS: RecordLayer = RecordLayer {
        layer: "tools",
        file: "tools/calls.jsonl",
        group: "tool",
        normalize: normalize::tool_result,
    };
}

struct Record {
    line: usize,
    group: String,
    body: Value,
}

fn records(dump: &Dump, spec: RecordLayer) -> Result<BTreeMap<String, Record>> {
    let mut out = BTreeMap::new();
    for l in dump.jsonl(spec.file)? {
        let id = l.value.get("id").and_then(Value::as_str);
        let group = l.value.get(spec.group).and_then(Value::as_str);
        let (Some(id), Some(group), Value::Object(map)) = (id, group, &l.value) else {
            bail!(
                "{}:{}: thiếu `id` hoặc `{}`",
                dump.path(spec.file).display(),
                l.no,
                spec.group
            );
        };
        let mut body = map.clone();
        body.remove("id");
        if let Some(r) = body.get_mut("result") {
            (spec.normalize)(group, r);
        }
        let rec = Record {
            line: l.no,
            group: group.to_owned(),
            body: Value::Object(body),
        };
        if out.insert(id.to_owned(), rec).is_some() {
            bail!(
                "{}:{}: id {id} ghi hai lần",
                dump.path(spec.file).display(),
                l.no
            );
        }
    }
    Ok(out)
}

fn records_layer(expected: &Dump, actual: &Dump, spec: RecordLayer) -> Result<Vec<ItemResult>> {
    if !expected.exists(spec.file) {
        return Ok(Vec::new());
    }
    let exp = records(expected, spec)?;
    let mut groups: BTreeMap<&str, Vec<(&String, &Record)>> = BTreeMap::new();
    for (id, r) in &exp {
        groups.entry(r.group.as_str()).or_default().push((id, r));
    }
    let act = if actual.exists(spec.file) {
        Some(records(actual, spec)?)
    } else {
        None
    };
    let mut out = Vec::new();
    for (group, recs) in &groups {
        if actual.not_available(spec.layer, group) {
            out.push(not_available(spec.layer, group, recs.len()));
            continue;
        }
        let Some(act) = &act else {
            out.push(item(
                spec.layer,
                group,
                recs.len(),
                vec![whole(DiffKind::Missing, "file", spec.file)],
            ));
            continue;
        };
        let mut diffs = Vec::new();
        for (id, e) in recs {
            match act.get(*id) {
                None => diffs.push(Diff {
                    line: Some(e.line),
                    ..whole(DiffKind::Missing, id, spec.file)
                }),
                Some(a) => {
                    let mut fields = Vec::new();
                    normalize::diff(&e.body, &a.body, "", 0, &mut fields);
                    if !fields.is_empty() {
                        diffs.push(Diff {
                            kind: DiffKind::Changed,
                            unit: (*id).clone(),
                            file: spec.file.to_owned(),
                            line: Some(e.line),
                            actual_line: Some(a.line),
                            fields,
                            exception: None,
                        });
                    }
                }
            }
        }
        out.push(item(spec.layer, group, recs.len(), diffs));
    }
    if let Some(act) = &act {
        let mut extra: BTreeMap<&str, Vec<Diff>> = BTreeMap::new();
        for (id, a) in act {
            if !exp.contains_key(id) && !actual.not_available(spec.layer, &a.group) {
                extra.entry(a.group.as_str()).or_default().push(Diff {
                    actual_line: Some(a.line),
                    ..whole(DiffKind::Extra, id, spec.file)
                });
            }
        }
        for (group, diffs) in extra {
            match out.iter_mut().find(|i| i.item == group) {
                Some(i) => i.diffs.extend(diffs),
                None => out.push(item(spec.layer, group, 0, diffs)),
            }
        }
    }
    Ok(out)
}

// ------------------------------------------------------------------------------------------ MCP

fn mcp_layer(expected: &Dump, actual: &Dump) -> Result<Vec<ItemResult>> {
    let mut out = Vec::new();
    for (name, file) in [
        ("initialize", "mcp/initialize.json"),
        ("tools_list", "mcp/tools_list.json"),
    ] {
        if !expected.exists(file) {
            continue;
        }
        if actual.not_available("mcp", name) {
            out.push(not_available("mcp", name, 1));
            continue;
        }
        if !actual.exists(file) {
            out.push(item(
                "mcp",
                name,
                1,
                vec![whole(DiffKind::Missing, "file", file)],
            ));
            continue;
        }
        let (etext, ev) = expected.json(file)?;
        let (atext, av) = actual.json(file)?;
        out.push(item(
            "mcp",
            name,
            1,
            json_doc_diffs(file, &etext, &ev, &atext, &av),
        ));
    }
    let file = "mcp/instructions.txt";
    if expected.exists(file) {
        let name = "instructions";
        if actual.not_available("mcp", name) {
            out.push(not_available("mcp", name, 1));
        } else if !actual.exists(file) {
            out.push(item(
                "mcp",
                name,
                1,
                vec![whole(DiffKind::Missing, "file", file)],
            ));
        } else {
            let e = expected.text(file)?;
            let a = actual.text(file)?;
            out.push(item(
                "mcp",
                name,
                e.lines().count(),
                text_diffs(file, &e, &a),
            ));
        }
    }
    Ok(out)
}

/// Lệch giữa hai văn bản JSON: mỗi chỗ lệch một mục, kèm dòng ở cả hai bên.
fn json_doc_diffs(file: &str, etext: &str, ev: &Value, atext: &str, av: &Value) -> Vec<Diff> {
    let mut fields = Vec::new();
    normalize::diff(ev, av, "", 0, &mut fields);
    if fields.is_empty() {
        return Vec::new();
    }
    let elines = value_lines(etext);
    let alines = value_lines(atext);
    fields
        .into_iter()
        .map(|f| Diff {
            kind: DiffKind::Changed,
            unit: f.path.clone(),
            file: file.to_owned(),
            line: line_of(&elines, &f.path),
            actual_line: line_of(&alines, &f.path),
            fields: vec![f],
            exception: None,
        })
        .collect()
}

/// Lệch theo dòng (LCS): dòng thay ghép cặp, phần thừa báo thiếu hoặc thêm.
pub fn text_diffs(file: &str, expected: &str, actual: &str) -> Vec<Diff> {
    let e: Vec<&str> = expected.lines().collect();
    let a: Vec<&str> = actual.lines().collect();
    let ops = line_ops(&e, &a);
    let mut out = Vec::new();
    let mut del: Vec<usize> = Vec::new();
    let mut ins: Vec<usize> = Vec::new();
    let flush = |del: &mut Vec<usize>, ins: &mut Vec<usize>, out: &mut Vec<Diff>| {
        let n = del.len().max(ins.len());
        for k in 0..n {
            let (ei, ai) = (del.get(k).copied(), ins.get(k).copied());
            let kind = match (ei, ai) {
                (Some(_), Some(_)) => DiffKind::Changed,
                (Some(_), None) => DiffKind::Missing,
                _ => DiffKind::Extra,
            };
            let unit = match (ei, ai) {
                (Some(i), _) => format!("dòng {}", i + 1),
                (None, Some(j)) => format!("dòng thêm {}", j + 1),
                (None, None) => unreachable!("k < max(len)"),
            };
            out.push(Diff {
                kind,
                unit,
                file: file.to_owned(),
                line: ei.map(|i| i + 1),
                actual_line: ai.map(|j| j + 1),
                fields: vec![FieldDiff {
                    path: "/line".into(),
                    expected: ei.map(|i| Value::from(e[i])),
                    actual: ai.map(|j| Value::from(a[j])),
                }],
                exception: None,
            });
        }
        del.clear();
        ins.clear();
    };
    for op in ops {
        match op {
            LineOp::Same => flush(&mut del, &mut ins, &mut out),
            LineOp::Del(i) => del.push(i),
            LineOp::Ins(j) => ins.push(j),
        }
    }
    flush(&mut del, &mut ins, &mut out);
    out
}

enum LineOp {
    Same,
    Del(usize),
    Ins(usize),
}

fn line_ops(e: &[&str], a: &[&str]) -> Vec<LineOp> {
    // bỏ phần đầu và cuối giống nhau, LCS trên phần giữa
    let pre = e.iter().zip(a).take_while(|(x, y)| x == y).count();
    let suf = e[pre..]
        .iter()
        .rev()
        .zip(a[pre..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (em, am) = (&e[pre..e.len() - suf], &a[pre..a.len() - suf]);
    let mut ops: Vec<LineOp> = (0..pre).map(|_| LineOp::Same).collect();
    if em.len().saturating_mul(am.len()) > 25_000_000 {
        // quá lớn cho LCS: so theo vị trí
        for k in 0..em.len().max(am.len()) {
            match (em.get(k), am.get(k)) {
                (Some(x), Some(y)) if x == y => ops.push(LineOp::Same),
                (ex, ay) => {
                    if ex.is_some() {
                        ops.push(LineOp::Del(pre + k));
                    }
                    if ay.is_some() {
                        ops.push(LineOp::Ins(pre + k));
                    }
                }
            }
        }
    } else {
        let (n, m) = (em.len(), am.len());
        let mut t = vec![0u32; (n + 1) * (m + 1)];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                t[i * (m + 1) + j] = if em[i] == am[j] {
                    t[(i + 1) * (m + 1) + j + 1] + 1
                } else {
                    t[(i + 1) * (m + 1) + j].max(t[i * (m + 1) + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n || j < m {
            if i < n && j < m && em[i] == am[j] {
                ops.push(LineOp::Same);
                i += 1;
                j += 1;
            } else if j < m && (i == n || t[i * (m + 1) + j + 1] >= t[(i + 1) * (m + 1) + j]) {
                ops.push(LineOp::Ins(pre + j));
                j += 1;
            } else {
                ops.push(LineOp::Del(pre + i));
                i += 1;
            }
        }
    }
    ops.extend((0..suf).map(|_| LineOp::Same));
    ops
}

// ------------------------------------------------------------------------------------------ CLI

fn cli_layer(expected: &Dump, actual: &Dump) -> Result<Vec<ItemResult>> {
    const FILE: &str = "cli/commands.json";
    if !expected.exists(FILE) {
        return Ok(Vec::new());
    }
    let exp = expected.cli_commands()?;
    let act: Option<Vec<CliCommand>> = if actual.exists(FILE) {
        Some(actual.cli_commands()?)
    } else {
        None
    };
    let mut out = Vec::new();
    for e in &exp {
        if actual.not_available("cli", &e.id) {
            out.push(not_available("cli", &e.id, 1));
            continue;
        }
        let Some(a) = act.as_ref().and_then(|v| v.iter().find(|a| a.id == e.id)) else {
            out.push(item(
                "cli",
                &e.id,
                1,
                vec![whole(DiffKind::Missing, &e.id, FILE)],
            ));
            continue;
        };
        out.push(item("cli", &e.id, 1, cli_command(expected, actual, e, a)?));
    }
    if let Some(act) = &act {
        for a in act {
            if !exp.iter().any(|e| e.id == a.id) && !actual.not_available("cli", &a.id) {
                out.push(item(
                    "cli",
                    &a.id,
                    0,
                    vec![whole(DiffKind::Extra, &a.id, FILE)],
                ));
            }
        }
    }
    Ok(out)
}

fn cli_command(
    expected: &Dump,
    actual: &Dump,
    e: &CliCommand,
    a: &CliCommand,
) -> Result<Vec<Diff>> {
    let mut diffs = Vec::new();
    let mut meta = Vec::new();
    for (path, ev, av) in [
        (
            "/args",
            Value::from(e.args.clone()),
            Value::from(a.args.clone()),
        ),
        ("/exit", Value::from(e.exit), Value::from(a.exit)),
        (
            "/kind",
            Value::from(e.kind.clone()),
            Value::from(a.kind.clone()),
        ),
    ] {
        if ev != av {
            meta.push(FieldDiff {
                path: path.into(),
                expected: Some(ev),
                actual: Some(av),
            });
        }
    }
    if !meta.is_empty() {
        diffs.push(Diff {
            fields: meta,
            ..whole(DiffKind::Changed, &e.id, "cli/commands.json")
        });
    }
    let efile = format!("cli/{}", e.stdout);
    let etext = expected.text(&efile)?;
    let atext = actual.text(&format!("cli/{}", a.stdout))?;
    if e.kind == "json" && a.kind == "json" {
        let parse = |t: &str, f: &str| -> Result<Value> {
            let mut v: Value = serde_json::from_str(t).with_context(|| format!("{f}: JSON lỗi"))?;
            normalize::cli_json(&mut v);
            Ok(v)
        };
        let ev = parse(&etext, &efile)?;
        let av = parse(&atext, &format!("cli/{}", a.stdout))?;
        diffs.extend(json_doc_diffs(&efile, &etext, &ev, &atext, &av));
    } else {
        diffs.extend(text_diffs(
            &efile,
            &normalize::cli_text(&etext),
            &normalize::cli_text(&atext),
        ));
    }
    let read_err = |d: &Dump, c: &CliCommand| -> Result<String> {
        match &c.stderr {
            Some(f) => d.text(&format!("cli/{f}")),
            None => Ok(String::new()),
        }
    };
    let (ee, ae) = (read_err(expected, e)?, read_err(actual, a)?);
    let efile_err = format!(
        "cli/{}",
        e.stderr.clone().unwrap_or_else(|| format!("{}.err", e.id))
    );
    diffs.extend(text_diffs(&efile_err, &ee, &ae));
    Ok(diffs)
}

// ------------------------------------------------------------------------------------------ hiệu năng

fn perf(expected: &Dump, actual: &Dump) -> Result<Option<Vec<PerfRow>>> {
    if actual.not_available("perf", "*") || !actual.exists("perf.json") {
        return Ok(None);
    }
    let flat = |d: &Dump| -> Result<BTreeMap<String, f64>> {
        let mut m = BTreeMap::new();
        if d.exists("perf.json") {
            let (_, v) = d.json("perf.json")?;
            flatten("", &v, &mut m);
        }
        Ok(m)
    };
    let e = flat(expected)?;
    let a = flat(actual)?;
    let mut keys: Vec<&String> = e.keys().chain(a.keys()).collect();
    keys.sort();
    keys.dedup();
    Ok(Some(
        keys.into_iter()
            .map(|k| PerfRow {
                metric: k.clone(),
                expected: e.get(k).copied(),
                actual: a.get(k).copied(),
            })
            .collect(),
    ))
}

fn flatten(prefix: &str, v: &Value, out: &mut BTreeMap<String, f64>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                let p = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(&p, x, out);
            }
        }
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                out.insert(prefix.to_owned(), f);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_diff_points_at_changed_line_only() {
        let d = text_diffs("f", "a\nb\nc\nd\n", "a\nB\nc\nd\n");
        assert_eq!(d.len(), 1);
        assert_eq!(
            (d[0].kind, d[0].line, d[0].actual_line),
            (DiffKind::Changed, Some(2), Some(2))
        );
        let d = text_diffs("f", "a\nb\nc\n", "a\nc\n");
        assert_eq!(d.len(), 1);
        assert_eq!((d[0].kind, d[0].line), (DiffKind::Missing, Some(2)));
        let d = text_diffs("f", "a\nc\n", "a\nb\nc\n");
        assert_eq!((d[0].kind, d[0].actual_line), (DiffKind::Extra, Some(2)));
    }

    fn lines(rows: &[Value]) -> Vec<Line> {
        rows.iter()
            .enumerate()
            .map(|(i, v)| Line {
                no: i + 1,
                value: v.clone(),
            })
            .collect()
    }

    #[test]
    fn table_rows_pair_changed_row_by_key_and_by_similarity() {
        use serde_json::json;
        let e = lines(&[
            json!({"id": "a", "v": 1}),
            json!({"id": "b", "v": 2}),
            json!({"id": "c", "v": 3}),
        ]);
        let a = lines(&[
            json!({"id": "c", "v": 3}),
            json!({"id": "a", "v": 1}),
            json!({"id": "b", "v": 9}),
        ]);
        let d = table_rows("t", "tables/t.jsonl", &["id".into()], &e, &a).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(
            (d[0].kind, d[0].line, d[0].actual_line),
            (DiffKind::Changed, Some(2), Some(3))
        );
        assert_eq!(d[0].fields[0].path, "/v");
        let d = table_rows("t", "tables/t.jsonl", &[], &e, &a).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].line, Some(2));
    }

    #[test]
    fn mtime_tolerance_one_ms() {
        use serde_json::json;
        let e = lines(&[json!({"id": "a", "mtime": 0})]);
        let ok = lines(&[json!({"id": "a", "mtime": 1})]);
        let bad = lines(&[json!({"id": "a", "mtime": 2})]);
        assert!(
            table_rows("nodes", "f", &["id".into()], &e, &ok)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            table_rows("nodes", "f", &["id".into()], &e, &bad)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn tolerance_holds_for_large_keyed_tables() {
        use serde_json::json;
        let n = PAIRING_LIMIT + 10;
        let e = lines(
            &(0..n)
                .map(|i| json!({"id": i.to_string(), "mtime": 0}))
                .collect::<Vec<_>>(),
        );
        let a = lines(
            &(0..n)
                .map(|i| json!({"id": i.to_string(), "mtime": 1}))
                .collect::<Vec<_>>(),
        );
        assert!(
            table_rows("nodes", "f", &["id".into()], &e, &a)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn duplicate_rows_count_as_multiset() {
        use serde_json::json;
        let e = lines(&[json!({"k": "x"}), json!({"k": "x"})]);
        let a = lines(&[json!({"k": "x"})]);
        let d = table_rows("t", "f", &[], &e, &a).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].kind, DiffKind::Missing);
    }
}
