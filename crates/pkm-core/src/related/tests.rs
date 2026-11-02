//! Test của `related`: phần lõi của `Protocol.test_related` (`scripts/kg/tests/test_mcp.py`) gọi thẳng hàm, cộng các
//! chỗ phải tính như Python (`sum`, `round`, dạng hit, `in`).

use serde_json::{Value, json};
use unicode_normalization::UnicodeNormalization;

use super::*;
use crate::index::testutil::{TestVault, clean_notes};

fn graph() -> (TestVault, Graph) {
    let v = TestVault::new(&clean_notes());
    v.build(&["business"]);
    let g = Graph::open(&v.db()).expect("mở index");
    (v, g)
}

fn hits(v: Value) -> Vec<(Value, Value)> {
    hits_from(&v).expect("hit hợp lệ")
}

fn col(r: &Map<String, Value>, part: &str, k: &str) -> Vec<Value> {
    r[part]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x[k].clone())
        .collect()
}

#[test]
fn sum_compensates_like_python_312() {
    // Python 3.12+: `sum([0.1] * 10) == 1.0`, `sum([1e100, 1.0, -1e100]) == 1.0`.
    assert_eq!(py_sum([0.1; 10]), 1.0);
    assert_eq!(py_sum([1e100, 1.0, -1e100]), 1.0);
    assert_eq!(py_sum([]), 0.0);
    assert_eq!(py_sum([f64::INFINITY, 1.0]), f64::INFINITY);
}

#[test]
fn round_half_even_on_the_binary_value() {
    assert_eq!(py_round(0.015625, 5), 0.01562);
    assert_eq!(py_round(0.046875, 5), 0.04688);
    assert_eq!(py_round(2.675, 2), 2.67);
    assert_eq!(py_round(0.204_031_234, 5), 0.20403);
}

#[test]
fn hit_shapes_like_tools_related() {
    let h = hits(json!([
        "A.md",
        {"path": "B.md", "score": 0.5},
        {"id": "C.md"},
        {"path": "", "id": "D.md", "score": "0.25"},
        ["E.md", 0.9],
        ["F.md"],
        "",
        {"score": 1}
    ]));
    assert_eq!(
        h,
        vec![
            (json!("A.md"), Value::Null),
            (json!("B.md"), json!(0.5)),
            (json!("C.md"), Value::Null),
            (json!("D.md"), json!("0.25")),
            (json!("E.md"), json!(0.9)),
            (json!("F.md"), Value::Null),
        ]
    );
    // Chuỗi lặp theo ký tự, object theo khoá, như `for h in hits`.
    assert_eq!(hits(json!("ab")).len(), 2);
    assert!(hits(json!(null)).is_empty());
    assert_eq!(hits_from(&json!([[]])).unwrap_err().kind, "IndexError");
    assert_eq!(hits_from(&json!([3])).unwrap_err().kind, "TypeError");
    assert_eq!(hits_from(&json!(3)).unwrap_err().kind, "TypeError");
}

#[test]
fn type_filter_like_python_in() {
    let t = Some("person".to_owned());
    assert!(type_in(&json!(["person"]), t.as_ref()).unwrap());
    assert!(!type_in(&json!(["claim"]), t.as_ref()).unwrap());
    assert!(type_in(&json!([null]), None).unwrap());
    assert!(type_in(&json!("persons"), t.as_ref()).unwrap());
    assert_eq!(type_in(&json!("x"), None).unwrap_err().kind, "TypeError");
    assert!(type_in(&json!({"person": 1}), t.as_ref()).unwrap());
    assert_eq!(
        type_in(&json!(1), t.as_ref()).unwrap_err().kind,
        "TypeError"
    );
}

#[test]
fn seeds_reranked_missing_reported_and_graph_notes_found() {
    let (_v, g) = graph();
    let h = hits(json!([
        {"path": "Yếu tố/Số lead.md", "score": 0.9},
        {"path": "Khách hàng/Không có.md", "score": 0.5},
        "Người/An.md"
    ]));
    let r = related(
        &g,
        &h,
        &Options {
            k: 5,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(r["missing"], json!(["Khách hàng/Không có.md"]));
    let mut seeds: Vec<Value> = col(&r, "seeds", "id");
    seeds.sort_by_key(|x| x.as_str().unwrap().to_owned());
    assert_eq!(seeds, [json!("Người/An.md"), json!("Yếu tố/Số lead.md")]);
    assert_eq!(col(&r, "seeds", "found_by"), [json!("ohs"), json!("ohs")]);
    let ranks: Vec<Value> = col(&r, "seeds", "ohs_rank");
    assert!(ranks.contains(&json!(1)) && ranks.contains(&json!(3)));
    let ids = col(&r, "related", "id");
    assert!(ids.len() <= 5);
    assert!(ids[..3].contains(&json!("Yếu tố/Doanh thu.md")), "{ids:?}");
    let top = &r["related"][0];
    assert_eq!(top["found_by"], "graph");
    let from = &top["via"][0]["from"];
    assert!(from == "Yếu tố/Số lead.md" || from == "Người/An.md");
    // Điểm giảm dần, làm tròn 5 chữ số; `method` như Python.
    let scores: Vec<f64> = col(&r, "related", "score")
        .iter()
        .map(|x| x.as_f64().unwrap())
        .collect();
    assert!(scores.windows(2).all(|w| w[0] >= w[1]));
    assert!(scores.iter().all(|s| py_round(*s, 5) == *s));
    let keys: Vec<&str> = r["method"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["alpha", "eps", "pushes", "touched"]);
    assert_eq!(r["method"]["alpha"], json!(0.15));
    let keys: Vec<&str> = r["related"][0]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["id", "type", "score", "found_by", "layer", "via"]);
    let keys: Vec<&str> = r["seeds"][0]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        ["id", "type", "score", "found_by", "ohs_rank", "degree"]
    );
}

#[test]
fn types_and_accepted_only_filter_graph_notes() {
    let (_v, g) = graph();
    let h = hits(json!([
        {"path": "Yếu tố/Số lead.md", "score": 0.9},
        "Người/An.md"
    ]));
    let o = Options {
        k: 5,
        types: json!(["organization"]),
        ..Options::default()
    };
    let r = related(&g, &h, &o).unwrap();
    let types = col(&r, "related", "type");
    assert!(!types.is_empty() && types.iter().all(|t| t == "organization"));

    let o = Options {
        k: 5,
        filter: Filter::new(None, None, None, true, false),
        ..Options::default()
    };
    let r = related(&g, &hits(json!(["Yếu tố/Mệt mỏi.md"])), &o).unwrap();
    assert!(!r["related"].as_array().unwrap().is_empty());
    for x in r["related"].as_array().unwrap() {
        assert_eq!(x["layer"], "accepted");
        assert!(
            x["via"]
                .as_array()
                .unwrap()
                .iter()
                .all(|s| s["status"] != "proposed")
        );
    }
}

#[test]
fn no_known_hit_gives_empty_result_without_method() {
    let (_v, g) = graph();
    let r = related(
        &g,
        &hits(json!(["Không có.md", "Không có.md"])),
        &Options::default(),
    )
    .unwrap();
    assert_eq!(
        Value::Object(r),
        json!({"seeds": [], "related": [], "missing": ["Không có.md"]})
    );
    let r = related(&g, &[], &Options::default()).unwrap();
    assert_eq!(r["missing"], json!([]));
}

#[test]
fn nfd_hit_paths_match_nfc_note_ids() {
    let (_v, g) = graph();
    let nfd: String = "Yếu tố/Số lead.md".nfd().collect();
    let r = related(&g, &[(json!(nfd), Value::Null)], &Options::default()).unwrap();
    assert_eq!(r["seeds"][0]["id"], "Yếu tố/Số lead.md");
    assert_eq!(r["missing"], json!([]));
}

#[test]
fn bad_hit_values_raise_like_python() {
    let (_v, g) = graph();
    let o = Options::default();
    let e = related(&g, &[(json!(3), Value::Null)], &o).unwrap_err();
    assert_eq!(e.kind, "TypeError");
    let e = related(&g, &[(json!("Người/An.md"), json!("cao"))], &o).unwrap_err();
    assert_eq!(e.kind, "ValueError");
    let e = related(&g, &[(json!("Người/An.md"), json!([1]))], &o).unwrap_err();
    assert_eq!(e.kind, "TypeError");
    // Điểm dạng chuỗi số được `float()` nhận; điểm âm hay thiếu thì theo thứ tự hit.
    assert!(related(&g, &[(json!("Người/An.md"), json!(" 0.5 "))], &o).is_ok());
}

#[test]
fn isolated_seed_keeps_all_its_mass() {
    let adj = Adjacency::default();
    let seeds: IndexMap<String, f64> = [("A.md".to_owned(), 1.0)].into_iter().collect();
    let r = ppr_push(&adj, &seeds, ALPHA, EPS, MAX_PUSHES);
    assert_eq!(r.pushes, 0);
    assert_eq!(
        r.p.into_iter().collect::<Vec<_>>(),
        [("A.md".to_owned(), 1.0)]
    );
}

#[test]
fn push_spreads_mass_both_ways_and_conserves_it() {
    let (_v, g) = graph();
    let adj = load_adjacency(&g, &Filter::new(None, None, None, false, false)).unwrap();
    let seeds: IndexMap<String, f64> = [("Yếu tố/Số lead.md".to_owned(), 1.0)]
        .into_iter()
        .collect();
    let r = ppr_push(&adj, &seeds, ALPHA, EPS, MAX_PUSHES);
    assert!(r.pushes > 0);
    let total = py_sum(r.p.values().copied());
    assert!(total > 0.5 && total <= 1.0 + 1e-12, "{total}");
    // Note đầu tiên được ghi là hạt giống (lần đẩy đầu).
    assert_eq!(
        r.p.keys().next().map(String::as_str),
        Some("Yếu tố/Số lead.md")
    );
    // Mỗi cạnh có hai chiều.
    let n: usize = (0..adj.ids.len()).map(|i| adj.steps(i).len()).sum();
    assert_eq!(n % 2, 0);
}

#[test]
fn max_pushes_bounds_the_work() {
    let (_v, g) = graph();
    let adj = load_adjacency(&g, &Filter::new(None, None, None, false, false)).unwrap();
    let seeds: IndexMap<String, f64> = [("Yếu tố/Số lead.md".to_owned(), 1.0)]
        .into_iter()
        .collect();
    let r = ppr_push(&adj, &seeds, ALPHA, EPS, 3);
    assert_eq!(r.pushes, 3);
}
