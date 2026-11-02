//! Chuẩn hoá trước khi so và so có dung sai. Mọi luật ở đây đều ghi ở `parity/README.md`,
//! mục "Chuẩn hoá"; thêm luật thì ghi lý do ở cả hai chỗ.
//!
//! Luật chỉ bỏ những gì không phải hành vi: thời điểm dựng, thời gian chạy, và thứ tự của danh
//! sách không có thứ tự. Thứ tự liệt kê note (quyết định `edges.id` và kết quả `related`) không
//! chuẩn hoá ở đây mà được ghim lúc dump (`dump-python.py --listing`).

use std::cmp::Ordering;

use serde_json::{Map, Value};

use crate::jsonpos::escape;

/// Sai số tuyệt đối cho số thực (điểm `related`, `score` của cạnh): PRD mục 6.
pub const FLOAT_TOLERANCE: f64 = 1e-6;
/// Sai số của `nodes.mtime` (ms): PowerShell và `stat` làm tròn khác nhau (PRD mục 6, E12.4).
pub const MTIME_TOLERANCE: i64 = 1;
/// Khoá `meta` ghi thời điểm và thời gian dựng.
const VOLATILE_META: [&str; 2] = ["built_at", "ms"];

/// Chuỗi chuẩn của một giá trị JSON (khoá object đã sắp), dùng làm khoá so và khoá sắp.
pub fn canonical(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

fn sort_canonical(items: &mut [Value]) {
    items.sort_by_cached_key(canonical);
}

// ------------------------------------------------------------------------------------------ tầng index

/// Một dòng bảng trước khi so: giá trị `meta` của `built_at`, `ms` thành null.
pub fn table_row(table: &str, row: &mut Map<String, Value>) {
    if table != "meta" {
        return;
    }
    let volatile = row
        .get("key")
        .and_then(Value::as_str)
        .is_some_and(|k| VOLATILE_META.contains(&k));
    if volatile {
        row.insert("value".into(), Value::Null);
    }
}

/// Dung sai số nguyên theo cột.
pub fn int_tolerance(table: &str, column: &str) -> i64 {
    if table == "nodes" && column == "mtime" {
        MTIME_TOLERANCE
    } else {
        0
    }
}

// ------------------------------------------------------------------------------------------ tầng câu đáp án

/// Như `norm` của `fixture_check.py`: danh sách kết quả so không theo thứ tự; kết quả `provenance`
/// (object có `sources`) thì từng danh sách bên trong so không theo thứ tự.
pub fn answer_result(v: &mut Value) {
    match v {
        Value::Array(items) => sort_canonical(items),
        Value::Object(map) if map.contains_key("sources") => {
            for x in map.values_mut() {
                if let Value::Array(items) = x {
                    sort_canonical(items);
                }
            }
        }
        _ => {}
    }
}

// ------------------------------------------------------------------------------------------ tầng tool

/// JSON của một tool trước khi so.
/// - `aliases`, `blocks`: không có thứ tự.
/// - `related`: `seeds` và `related` sắp theo điểm giảm dần rồi id, để chỉ so tập note, thứ hạng
///   theo điểm và điểm (dung sai `FLOAT_TOLERANCE`), không so thứ tự giữa các note bằng điểm.
pub fn tool_result(tool: &str, v: &mut Value) {
    walk(v);
    if tool == "related"
        && let Value::Object(map) = v
    {
        for key in ["seeds", "related"] {
            if let Some(Value::Array(items)) = map.get_mut(key) {
                items.sort_by(by_score_then_id);
            }
        }
    }
}

fn walk(v: &mut Value) {
    match v {
        Value::Object(map) => {
            for (k, x) in map.iter_mut() {
                if (k == "aliases" || k == "blocks")
                    && let Value::Array(items) = x
                {
                    sort_canonical(items);
                }
                walk(x);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(walk),
        _ => {}
    }
}

fn by_score_then_id(a: &Value, b: &Value) -> Ordering {
    let score = |x: &Value| x.get("score").and_then(Value::as_f64).unwrap_or(0.0);
    let id = |x: &Value| x.get("id").and_then(Value::as_str).unwrap_or("").to_owned();
    score(b)
        .partial_cmp(&score(a))
        .unwrap_or(Ordering::Equal)
        .then_with(|| id(a).cmp(&id(b)))
}

// ------------------------------------------------------------------------------------------ tầng CLI

/// JSON của một lệnh CLI: bỏ thời điểm và thời gian dựng (`build --json`, `stats`), danh sách
/// nhóm của `stats` không có thứ tự (SQL sắp theo số đếm, bằng nhau thì tuỳ SQLite), rồi như tool.
pub fn cli_json(v: &mut Value) {
    if let Value::Object(map) = v {
        for k in VOLATILE_META {
            map.remove(k);
        }
        if let Some(Value::Object(meta)) = map.get_mut("meta") {
            for k in VOLATILE_META {
                meta.remove(k);
            }
        }
        for k in ["edges_by_origin", "edges_by_rel"] {
            if let Some(Value::Array(items)) = map.get_mut(k) {
                sort_canonical(items);
            }
        }
    }
    tool_result("", v);
}

/// Văn bản của một lệnh CLI: thời gian từng phần của `build` (`; ms {...}`) thay bằng `; ms {…}`.
pub fn cli_text(text: &str) -> String {
    const MARK: &str = "; ms {";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(MARK) {
        out.push_str(&rest[..i + MARK.len()]);
        rest = &rest[i + MARK.len()..];
        match rest.find('}') {
            Some(j) => {
                out.push('…');
                rest = &rest[j..];
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

// ------------------------------------------------------------------------------------------ so có dung sai

/// Một chỗ lệch trong một giá trị JSON.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDiff {
    /// JSON pointer tới chỗ lệch (`/items/3/node`); với dòng bảng là `/tên cột`.
    pub path: String,
    pub expected: Option<Value>,
    pub actual: Option<Value>,
}

/// Hai số bằng nhau theo dung sai: số thực lệch tối đa `FLOAT_TOLERANCE`, số nguyên lệch tối đa `int_tol`.
/// Số nguyên và số thực không bao giờ bằng nhau (kiểu giá trị cũng là hành vi, ví dụ cột REAL).
pub fn numbers_equal(a: &Value, b: &Value, int_tol: i64) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            if let (Some(i), Some(j)) = (x.as_i64(), y.as_i64()) {
                return i.abs_diff(j) <= int_tol.unsigned_abs();
            }
            if x.is_f64()
                && y.is_f64()
                && let (Some(p), Some(q)) = (x.as_f64(), y.as_f64())
            {
                return (p - q).abs() <= FLOAT_TOLERANCE;
            }
            x == y
        }
        _ => false,
    }
}

/// Mọi chỗ lệch giữa `expected` và `actual` (đã chuẩn hoá). Danh sách so theo vị trí; độ dài khác
/// thì thêm một chỗ lệch ở chính danh sách. `int_tol` áp cho số nguyên ở gốc (cột bảng).
pub fn diff(expected: &Value, actual: &Value, path: &str, int_tol: i64, out: &mut Vec<FieldDiff>) {
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => {
            for (k, ev) in e {
                let p = format!("{path}/{}", escape(k));
                match a.get(k) {
                    Some(av) => diff(ev, av, &p, 0, out),
                    None => out.push(FieldDiff {
                        path: p,
                        expected: Some(ev.clone()),
                        actual: None,
                    }),
                }
            }
            for (k, av) in a {
                if !e.contains_key(k) {
                    out.push(FieldDiff {
                        path: format!("{path}/{}", escape(k)),
                        expected: None,
                        actual: Some(av.clone()),
                    });
                }
            }
        }
        (Value::Array(e), Value::Array(a)) => {
            for (i, (ev, av)) in e.iter().zip(a).enumerate() {
                diff(ev, av, &format!("{path}/{i}"), 0, out);
            }
            if e.len() != a.len() {
                out.push(FieldDiff {
                    path: path.to_owned(),
                    expected: Some(Value::from(format!("{} phần tử", e.len()))),
                    actual: Some(Value::from(format!("{} phần tử", a.len()))),
                });
            }
        }
        (Value::Number(_), Value::Number(_)) => {
            if !numbers_equal(expected, actual, int_tol) {
                out.push(FieldDiff {
                    path: path.to_owned(),
                    expected: Some(expected.clone()),
                    actual: Some(actual.clone()),
                });
            }
        }
        _ => {
            if expected != actual {
                out.push(FieldDiff {
                    path: path.to_owned(),
                    expected: Some(expected.clone()),
                    actual: Some(actual.clone()),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn floats_within_tolerance_ints_exact_and_types_distinct() {
        assert!(numbers_equal(&json!(0.1), &json!(0.1000005), 0));
        assert!(!numbers_equal(&json!(0.1), &json!(0.10001), 0));
        assert!(!numbers_equal(&json!(1), &json!(1.0), 0));
        assert!(numbers_equal(&json!(5), &json!(6), 1));
        assert!(!numbers_equal(&json!(5), &json!(7), 1));
    }

    #[test]
    fn aliases_and_blocks_unordered_edge_ids_kept() {
        let mut v = json!({"items": [{"rel": "uses", "id": 7, "aliases": ["b", "a"]}], "blocks": ["q2", "q1"]});
        tool_result("neighbors", &mut v);
        assert_eq!(
            v,
            json!({"items": [{"rel": "uses", "id": 7, "aliases": ["a", "b"]}], "blocks": ["q1", "q2"]})
        );
    }

    #[test]
    fn related_sorted_by_score_then_id() {
        let mut v = json!({"related": [{"id": "b", "score": 0.1}, {"id": "a", "score": 0.1}, {"id": "c", "score": 0.5}]});
        tool_result("related", &mut v);
        let ids: Vec<_> = v["related"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["c", "a", "b"]);
    }

    #[test]
    fn cli_text_masks_build_timings_only() {
        let s = "<DB> (full): 3 note; ms {\"list\": 1.5, \"total\": 9}\ncạnh theo lớp: {'accepted': 2}\n";
        assert_eq!(
            cli_text(s),
            "<DB> (full): 3 note; ms {…}\ncạnh theo lớp: {'accepted': 2}\n"
        );
    }

    #[test]
    fn meta_timing_values_ignored_other_rows_kept() {
        let mut row = json!({"key": "built_at", "value": "2026-11-02T10:00:00"});
        table_row("meta", row.as_object_mut().unwrap());
        assert_eq!(row["value"], Value::Null);
        let mut row = json!({"key": "notes", "value": "3"});
        table_row("meta", row.as_object_mut().unwrap());
        assert_eq!(row["value"], json!("3"));
        let mut e = json!({"id": 4, "src": "a"});
        table_row("edges", e.as_object_mut().unwrap());
        assert_eq!(e, json!({"id": 4, "src": "a"}));
    }

    #[test]
    fn answer_lists_unordered() {
        let mut a = json!(["b", "a"]);
        answer_result(&mut a);
        assert_eq!(a, json!(["a", "b"]));
        let mut p = json!({"sources": [2, 1], "evidence": [{"x": 2}, {"x": 1}]});
        answer_result(&mut p);
        assert_eq!(
            p,
            json!({"sources": [1, 2], "evidence": [{"x": 1}, {"x": 2}]})
        );
    }

    #[test]
    fn diff_reports_paths() {
        let mut out = Vec::new();
        diff(
            &json!({"a": [1, 2], "b": "x"}),
            &json!({"a": [1, 3], "c": 1}),
            "",
            0,
            &mut out,
        );
        let paths: Vec<_> = out.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(paths, ["/a/1", "/b", "/c"]);
    }
}
