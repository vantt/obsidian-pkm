//! Test của `query`, chuyển từ `scripts/kg/tests/test_kg.py` ở sha ghim (lớp `Api`, phần `Graph` của
//! `Incremental`, `Fixture.test_answer_key`) trên cùng vault nhỏ.

use serde_json::{Value, json};

use super::answers;
use super::*;
use crate::index::testutil::{TestVault, clean_notes};

fn api() -> (TestVault, Graph) {
    let v = TestVault::new(&clean_notes());
    v.build(&["business"]);
    let g = Graph::open(&v.db()).expect("mở index");
    (v, g)
}

fn s(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|x| (*x).to_owned()).collect()
}

fn rels(xs: &[&str]) -> Filter {
    Filter::new(Some(&s(xs)), None, None, false, false)
}

fn col<'a>(items: &'a [Map<String, Value>], k: &str) -> Vec<&'a Value> {
    items.iter().map(|x| &x[k]).collect()
}

#[test]
fn two_layers_default_and_accepted_only() {
    let (_v, g) = api();
    let n = g
        .neighbors(
            "Yếu tố/Doanh thu.md",
            Dir::In,
            &rels(&["increases", "decreases"]),
        )
        .unwrap();
    let by: Vec<(&Value, &Value)> = n.iter().map(|x| (&x["node"], &x["status"])).collect();
    assert!(by.contains(&(&json!("Yếu tố/Mệt mỏi.md"), &json!("proposed"))));
    assert!(by.contains(&(&json!("Yếu tố/Số lead.md"), &json!("accepted"))));
    let p = n.iter().find(|x| x["status"] == "proposed").unwrap();
    assert_eq!(
        (
            &p["layer"],
            &p["proposed_by"],
            &p["extracted_by"],
            &p["score"],
            &p["claim"]
        ),
        (
            &json!("proposed"),
            &json!("llm"),
            &json!("claude-sonnet@schema_v3"),
            &json!(0.62),
            &json!("Claim/Mệt giảm doanh thu.md")
        )
    );
    let f = Filter::new(
        Some(&s(&["increases", "decreases"])),
        None,
        None,
        true,
        false,
    );
    let acc = g.neighbors("Yếu tố/Doanh thu.md", Dir::In, &f).unwrap();
    assert!(!acc.iter().any(|x| x["status"] == "proposed"));
}

#[test]
fn claim_edge_window_source_and_alias() {
    let (_v, g) = api();
    let e = g
        .edge_of_claim("Claim/Lead tăng doanh thu.md")
        .unwrap()
        .unwrap();
    assert_eq!(e["source"], "Họp/Họp 2026-01-05.md#^q1");
    // Đích ghi bằng alias.
    let a = g
        .edge_of_claim("Claim/Doanh thu tăng ngân sách.md")
        .unwrap()
        .unwrap();
    assert_eq!(a["dst"], "Yếu tố/Ngân sách.md");
    let at = |d: &str| Filter::new(Some(&s(&["works_at"])), Some(d), None, false, false);
    let old = g
        .neighbors("Người/An.md", Dir::Out, &at("2025-01-01"))
        .unwrap();
    let new = g
        .neighbors("Người/An.md", Dir::Out, &at("2026-01-01"))
        .unwrap();
    assert_eq!(col(&old, "node"), [&json!("Khách hàng/Công ty ABC.md")]);
    assert_eq!(col(&new, "node"), [&json!("Khách hàng/Công ty XYZ.md")]);
    assert_eq!(old[0]["valid_to"], "2025-06-30");
}

#[test]
fn contract_window() {
    let (_v, g) = api();
    assert_eq!(
        g.valid("contract", "2025-06-01", false).unwrap(),
        ["Hợp đồng/HĐ ABC 2025.md"]
    );
    let f = Filter::new(
        Some(&s(&["covers"])),
        Some("2026-03-01"),
        None,
        false,
        false,
    );
    let cov = g.neighbors("Dự án/Web ABC.md", Dir::In, &f).unwrap();
    assert_eq!(col(&cov, "node"), [&json!("Hợp đồng/HĐ ABC 2026.md")]);
}

#[test]
fn loops_need_start_and_report_polarity() {
    let (_v, g) = api();
    let f = rels(&["increases", "decreases"]);
    assert_eq!(g.loops("", 5, &f).unwrap_err().kind, "ValueError");
    assert_eq!(
        g.loops("Yếu tố/Doanh thu.md", 0, &f).unwrap_err().kind,
        "ValueError"
    );
    let ls = g.loops("Yếu tố/Doanh thu.md", 5, &f).unwrap();
    let got: Vec<(&Value, &Value)> = ls.iter().map(|l| (&l["nodes"], &l["kind"])).collect();
    assert_eq!(
        got,
        [(
            &json!([
                "Yếu tố/Doanh thu.md",
                "Yếu tố/Ngân sách.md",
                "Yếu tố/Số lead.md"
            ]),
            &json!("reinforcing")
        )]
    );
}

#[test]
fn paths_and_provenance() {
    let (_v, g) = api();
    let ps = g
        .paths(
            "Yếu tố/Mệt mỏi.md",
            "Yếu tố/Doanh thu.md",
            6,
            &rels(&["increases", "decreases"]),
            None,
        )
        .unwrap();
    let got: Vec<(&Value, &Value, &Value)> = ps
        .iter()
        .map(|p| (&p["rels"], &p["sign"], &p["layer"]))
        .collect();
    assert_eq!(
        got,
        [
            (&json!(["decreases"]), &json!("-"), &json!("proposed")),
            (
                &json!(["decreases", "increases"]),
                &json!("-"),
                &json!("accepted")
            ),
        ]
    );
    let pv = g.provenance("Claim/An làm ở ABC.md").unwrap();
    assert_eq!(pv["superseded_by"], json!(["Claim/An làm ở XYZ.md"]));
    assert_eq!(pv["sources"][0]["note"], "Họp/Họp 2026-01-05.md");
}

#[test]
fn find_select() {
    let (_v, g) = api();
    let ids = |name: &str| -> Vec<Value> {
        g.find(name, None, None)
            .unwrap()
            .into_iter()
            .map(|x| x["id"].clone())
            .collect()
    };
    assert_eq!(ids("cong ty abc"), [json!("Khách hàng/Công ty ABC.md")]);
    assert_eq!(
        ids("cong ty"),
        [
            json!("Khách hàng/Công ty ABC.md"),
            json!("Khách hàng/Công ty XYZ.md")
        ]
    );
    let w = json!({"status": "proposed", "proposed_by": "llm"});
    assert_eq!(
        g.select(w.as_object().unwrap(), false).unwrap(),
        ["Claim/Mệt giảm doanh thu.md"]
    );
}

/// `Incremental.test_rebuild_after_edit_and_delete`, phần đọc qua `Graph` (phần build ở `index::tests`): sửa
/// status một claim và xoá một claim, build tăng dần, cạnh của claim đã đổi status và vòng qua claim đã xoá mất.
#[test]
fn rebuild_after_edit_and_delete() {
    let v = TestVault::new(&clean_notes());
    assert_eq!(v.build(&["business"]).get("mode"), Some("full"));
    let p = v.path().join("Claim").join("Mệt giảm doanh thu.md");
    let txt = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, txt.replace("status: proposed", "status: accepted")).unwrap();
    std::fs::remove_file(v.path().join("Claim").join("Ngân sách tăng lead.md")).unwrap();
    let info = v.build(&["business"]);
    assert_eq!(
        (info.get("mode"), info.get("notes_removed")),
        (Some("incremental"), Some("1"))
    );
    let g = Graph::open(&v.db()).unwrap();
    let e = g
        .edge_of_claim("Claim/Mệt giảm doanh thu.md")
        .unwrap()
        .unwrap();
    assert_eq!(e["status"], "accepted");
    let ls = g
        .loops("Yếu tố/Doanh thu.md", 5, &rels(&["increases", "decreases"]))
        .unwrap();
    assert!(ls.is_empty());
}

#[test]
fn reach_depth_and_direction() {
    let (_v, g) = api();
    let f = rels(&["increases", "decreases"]);
    let down = g.reach("Yếu tố/Mệt mỏi.md", Dir::Out, Some(2), &f).unwrap();
    assert_eq!(
        down,
        [
            ("Yếu tố/Doanh thu.md".to_owned(), Some(1)),
            ("Yếu tố/Ngân sách.md".to_owned(), Some(2)),
            ("Yếu tố/Số lead.md".to_owned(), Some(1)),
        ]
    );
    let up = g.reach("Yếu tố/Doanh thu.md", Dir::In, None, &f).unwrap();
    let ids: Vec<&str> = up.iter().map(|(i, _)| i.as_str()).collect();
    assert_eq!(
        ids,
        [
            "Yếu tố/Mệt mỏi.md",
            "Yếu tố/Ngân sách.md",
            "Yếu tố/Số lead.md"
        ]
    );
    assert!(up.iter().all(|(_, d)| d.is_none()));
}

#[test]
fn note_card_counts_edges_by_relation_and_layer() {
    let (_v, g) = api();
    assert!(g.note("Không có.md").unwrap().is_none());
    let n = g.note("Yếu tố/Ngân sách.md").unwrap().unwrap();
    let keys: Vec<&str> = n.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "id",
            "title",
            "type",
            "folder",
            "status",
            "valid_from",
            "valid_to",
            "proposed_by",
            "extracted_by",
            "asserted_at",
            "yaml_error",
            "props",
            "blocks",
            "edges",
            "claim_edge"
        ]
    );
    assert_eq!(n["props"]["aliases"], "Ngân sách quảng cáo");
    assert_eq!(n["claim_edge"], Value::Null);
    assert_eq!(n["edges"]["in"]["increased_by"]["accepted"], 1);
    let c = g.note("Claim/Lead tăng doanh thu.md").unwrap().unwrap();
    assert_eq!(c["claim_edge"]["rel"], "increases");
    assert_eq!(c["claim_edge"]["layer"], "accepted");
}

#[test]
fn all_loops_start_at_smallest_id() {
    let (_v, g) = api();
    let f = Filter::new(
        Some(&s(&["increases", "decreases"])),
        None,
        None,
        false,
        false,
    );
    // Không gộp cạnh song song (cạnh frontmatter và cạnh claim cùng cặp): hai vòng giống nhau, như Python.
    let ls = g.all_loops(10, &f).unwrap();
    let one = json!({"nodes": ["Yếu tố/Doanh thu.md", "Yếu tố/Ngân sách.md", "Yếu tố/Số lead.md"],
                     "rels": ["increases", "increases", "increases"], "statuses": ["accepted", "accepted", "accepted"],
                     "negatives": 0, "kind": "reinforcing", "layer": "accepted"});
    let ls: Vec<Value> = ls.into_iter().map(Value::Object).collect();
    assert_eq!(ls, [one.clone(), one]);
}

#[test]
fn answers_like_fixture_check() {
    let (_v, g) = api();
    let q = json!({"op": "chain", "start": "Người/An.md",
                   "steps": [{"rel": "works_at", "dir": "out", "asOf": "2025-01-01"}]});
    assert_eq!(
        answers::run(&g, &q).unwrap(),
        json!(["Khách hàng/Công ty ABC.md"])
    );
    let q =
        json!({"op": "cycles", "through": "Yếu tố/Số lead.md", "rels": ["increases"], "maxLen": 5});
    assert_eq!(
        answers::run(&g, &q).unwrap(),
        json!([{"nodes": ["Yếu tố/Doanh thu.md", "Yếu tố/Ngân sách.md", "Yếu tố/Số lead.md"],
                "rels": ["increases", "increases", "increases"], "negatives": 0, "kind": "reinforcing"}])
    );
    let q = json!({"op": "provenance", "claim": "Claim/Lead tăng doanh thu.md"});
    assert_eq!(
        answers::run(&g, &q).unwrap(),
        json!({"sources": [{"note": "Họp/Họp 2026-01-05.md", "block": "q1"}], "evidence": []})
    );
    assert_eq!(
        answers::run(&g, &json!({"op": "nope"})).unwrap_err().repr(),
        "KeyError(\"'nope'\")"
    );
    assert_eq!(
        answers::run(&g, &json!({"op": "reach"})).unwrap_err().kind,
        "KeyError"
    );
    // `norm`: danh sách so không theo thứ tự.
    assert_eq!(answers::norm(&json!([2, 1])), answers::norm(&json!([1, 2])));
}

/// `Fixture.test_answer_key`: 70 câu graph và 11 câu bảng của kg-solo đúng đáp án. Cần vault kg-solo dựng sẵn
/// (`PKM_KG_SOLO=<thư mục>`; CI so cả bộ câu qua bộ so khớp); không có thì bỏ qua như Python khi thiếu node.
#[test]
fn answer_key() {
    let Some(dir) = std::env::var_os("PKM_KG_SOLO") else {
        eprintln!("bỏ qua: chưa đặt PKM_KG_SOLO");
        return;
    };
    let vault = std::path::PathBuf::from(dir);
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("graph.db");
    let b = crate::index::Build {
        packs: vec!["business".into()],
        ..crate::index::Build::default()
    };
    crate::index::build(&vault, &db, &b).expect("build kg-solo");
    let r = answers::check(&vault, &db).expect("chạy bộ câu");
    assert_eq!(r.fails, []);
    assert_eq!(r.graph, [70, 70]);
    assert_eq!(r.table, [11, 11]);
}
