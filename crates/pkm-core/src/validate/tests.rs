//! Test của validator, chuyển từ `scripts/kg/tests/test_kg.py` (lớp `Planted`, `Api.test_clean_vault_has_no_deviation`,
//! `Fixture.test_fixture_planted_errors`) ở sha ghim.

use std::collections::HashSet;

use super::*;
use crate::index::testutil::{EXPECT, NOTES, TestVault, clean_notes};

fn planted() -> (TestVault, Vec<Deviation>) {
    let v = TestVault::new(NOTES);
    v.build(&["business"]);
    let dev = deviations(&v.open(), None, None).expect("deviations");
    (v, dev)
}

#[test]
fn every_planted_error_fires() {
    let (_v, dev) = planted();
    let got: HashSet<(&str, &str)> = dev
        .iter()
        .map(|d| (d.rule.as_str(), d.node.as_deref().unwrap_or_default()))
        .collect();
    let missing: Vec<_> = EXPECT
        .iter()
        .filter(|(n, r)| !got.contains(&(*r, *n)))
        .collect();
    assert_eq!(missing, Vec::<&(&str, &str)>::new());
}

#[test]
fn every_rule_category() {
    let (v, _) = planted();
    let rules: HashSet<String> = counts(&v.open()).unwrap().into_keys().collect();
    for i in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 15] {
        assert!(
            rules.contains(&format!("R{i}")),
            "R{i} thiếu trong {rules:?}"
        );
    }
}

#[test]
fn clean_notes_have_no_deviation() {
    let (_v, dev) = planted();
    let bad: Vec<_> = dev
        .iter()
        .filter(|d| {
            let n = d.node.as_deref().unwrap_or_default();
            !n.starts_with("Lỗi/") && n != "Khách hàng/Công ty ABC.md"
        })
        .collect();
    assert_eq!(bad, Vec::<&Deviation>::new());
}

#[test]
fn r9_kinds() {
    let (_v, dev) = planted();
    let r9 = |n: &str| -> String {
        dev.iter()
            .filter(|d| d.rule == "R9" && d.node.as_deref() == Some(n))
            .filter_map(|d| d.detail.clone())
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert!(r9("Lỗi/R9 thiếu trường.md").contains("confidence"));
    assert!(r9("Lỗi/R9 thiếu khi conditions.md").contains("counterfactual"));
    assert!(r9("Lỗi/R9 enum.md").contains("ngoài tập confidence"));
    assert!(r9("Lỗi/R9 quan hệ lạ.md").contains("ngoài tập relation"));
    assert!(r9("Lỗi/R9 mẫu.md").contains("sai mẫu"));
}

/// Mọi chỗ lệch của vault cài lỗi, đúng như `validate.deviations` của Python ở sha ghim trả (đã chạy và chép lại).
#[test]
fn planted_deviations_equal_python() {
    let (v, dev) = planted();
    let want: &[(&str, &str, &str)] = &[
        (
            "R0",
            "Lỗi/R0 yaml.md",
            "YAML lỗi: while parsing a flow sequence",
        ),
        ("R1", "Lỗi/R1 thiếu type.md", "thiếu type"),
        ("R1", "Lỗi/R1 type lạ.md", "type ngoài schema: recipe"),
        (
            "R10",
            "Lỗi/R10 conditions máy.md",
            "conditions do máy (llm) ghi phải để proposed, đang accepted",
        ),
        (
            "R11",
            "Lỗi/R11 cửa sổ.md",
            "valid_from 2026-05-01 sau valid_to 2026-01-01",
        ),
        (
            "R11",
            "Lỗi/R11 ngày.md",
            "valid_from sai định dạng ngày: 2026-13-01",
        ),
        (
            "R11",
            "Lỗi/R11 ngày.md",
            "valid_from sai định dạng ngày: 2026-13-01",
        ),
        (
            "R12",
            "Lỗi/R12 superseded.md",
            "superseded thiếu superseded_by",
        ),
        ("R12", "Lỗi/R12 superseded.md", "superseded thiếu valid_to"),
        (
            "R12",
            "Lỗi/R12 vòng A.md",
            "chuỗi superseded_by quay lại chính note",
        ),
        (
            "R12",
            "Lỗi/R12 vòng B.md",
            "chuỗi superseded_by quay lại chính note",
        ),
        (
            "R13",
            "Lỗi/R13 block.md",
            "khối không có trong note đích: Họp 2026-01-05#^q9",
        ),
        (
            "R13",
            "Lỗi/R13 không source.md",
            "claim accepted không có source",
        ),
        (
            "R15",
            "Lỗi/R15 hai khách.md",
            "client chỉ được một đích, đang có 2",
        ),
        (
            "R2",
            "Lỗi/R2 khoá lạ.md",
            "khoá \"dẫn_đến\" có wikilink nhưng không là cạnh hay trường liên kết của schema",
        ),
        (
            "R3",
            "Lỗi/R3 đích không có.md",
            "uses: [[Figma]] không có note",
        ),
        (
            "R4",
            "Lỗi/R4 claim.md",
            "cause: claim → concept (Khái niệm/Marketing.md) ngoài domain/range",
        ),
        (
            "R4",
            "Lỗi/R4 claim.md",
            "increases của claim: concept → factor ngoài domain/range",
        ),
        (
            "R4",
            "Lỗi/R4 domain.md",
            "increases: concept → factor (Yếu tố/Doanh thu.md) ngoài domain/range",
        ),
        (
            "R4",
            "Lỗi/R4 range.md",
            "up: concept → moc (MOC/MOC Kinh doanh.md) ngoài domain/range",
        ),
        (
            "R5",
            "Lỗi/R5 nghịch đảo.md",
            "khoá nghịch đảo viết tay: used_by",
        ),
        (
            "R6",
            "Lỗi/R6 S27.md",
            "vừa related vừa up với Khái niệm/Marketing.md",
        ),
        ("R7", "Lỗi/R7 tự trỏ.md", "related trỏ về chính note"),
        (
            "R8",
            "Khách hàng/Công ty ABC.md",
            "title trùng (không phân biệt hoa thường, dấu): Công ty ABC",
        ),
        (
            "R8",
            "Lỗi/Cong ty ABC.md",
            "title trùng (không phân biệt hoa thường, dấu): Cong ty ABC",
        ),
        (
            "R9",
            "Lỗi/R12 superseded.md",
            "thiếu superseded_by (bắt buộc khi status = superseded)",
        ),
        (
            "R9",
            "Lỗi/R12 superseded.md",
            "thiếu valid_to (bắt buộc khi status = superseded)",
        ),
        (
            "R9",
            "Lỗi/R13 không source.md",
            "thiếu trường bắt buộc của claim: source",
        ),
        (
            "R9",
            "Lỗi/R9 enum.md",
            "confidence = chắc chắn ngoài tập confidence",
        ),
        (
            "R9",
            "Lỗi/R9 mẫu.md",
            "extracted_by = claude sai mẫu ^.+@schema_v[0-9]+$",
        ),
        (
            "R9",
            "Lỗi/R9 quan hệ lạ.md",
            "relation = causes ngoài tập relation",
        ),
        (
            "R9",
            "Lỗi/R9 thiếu khi conditions.md",
            "thiếu counterfactual (bắt buộc khi relation = conditions)",
        ),
        (
            "R9",
            "Lỗi/R9 thiếu trường.md",
            "thiếu trường bắt buộc của claim: confidence",
        ),
    ];
    let got: Vec<(&str, &str, &str)> = dev
        .iter()
        .map(|d| {
            (
                d.rule.as_str(),
                d.node.as_deref().unwrap_or_default(),
                d.detail.as_deref().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(got, want);
    let c: Vec<(String, i64, i64)> = counts(&v.open())
        .unwrap()
        .into_iter()
        .map(|(r, c)| (r, c.deviations, c.notes))
        .collect();
    let want_counts: Vec<(String, i64, i64)> = [
        ("R0", 1, 1),
        ("R1", 2, 2),
        ("R2", 1, 1),
        ("R3", 1, 1),
        ("R4", 4, 3),
        ("R5", 1, 1),
        ("R6", 1, 1),
        ("R7", 1, 1),
        ("R8", 2, 2),
        ("R9", 8, 7),
        ("R10", 1, 1),
        ("R11", 3, 2),
        ("R12", 4, 3),
        ("R13", 2, 2),
        ("R15", 1, 1),
    ]
    .into_iter()
    .map(|(r, d, n)| (r.to_owned(), d, n))
    .collect();
    assert_eq!(c, want_counts);
}

#[test]
fn clean_vault_has_no_deviation() {
    let v = TestVault::new(&clean_notes());
    v.build(&["business"]);
    assert_eq!(deviations(&v.open(), None, None).unwrap(), Vec::new());
    assert!(counts(&v.open()).unwrap().is_empty());
}

#[test]
fn rule_filter_and_limit() {
    let (v, _) = planted();
    let db = v.open();
    let r9 = deviations(&db, Some("R9"), None).unwrap();
    assert_eq!(r9.len(), 8);
    assert!(r9.iter().all(|d| d.rule == "R9"));
    assert_eq!(deviations(&db, Some("R9"), Some(3)).unwrap(), r9[..3]);
    // `if limit:`: 0 là không giới hạn; số âm thành `LIMIT -1` của SQLite.
    assert_eq!(deviations(&db, Some("R9"), Some(0)).unwrap(), r9);
    assert_eq!(deviations(&db, Some("R9"), Some(-1)).unwrap(), r9);
    assert_eq!(deviations(&db, Some("R14"), None).unwrap(), Vec::new());
}

#[test]
fn report_like_the_validate_command() {
    let (v, _) = planted();
    let db = v.open();
    // Không `--rule`: mọi luật có lệch, mỗi luật tối đa 5 ví dụ (`a.limit or 5`).
    let r = report(&db, None, None).unwrap();
    assert_eq!(r.examples.len(), r.counts.len());
    assert_eq!(r.examples["R9"].len(), 5);
    assert_eq!(report(&db, None, Some(0)).unwrap(), r);
    let one = report(&db, Some("R8"), Some(1)).unwrap();
    assert_eq!(one.counts, r.counts);
    assert_eq!(one.examples.keys().collect::<Vec<_>>(), ["R8"]);
    assert_eq!(one.examples["R8"].len(), 1);
    // Luật không có lệch: không có ví dụ.
    assert!(report(&db, Some("R14"), None).unwrap().examples.is_empty());

    let j = one.to_json();
    assert_eq!(j["counts"]["R9"], json!({"deviations": 8, "notes": 7}));
    assert_eq!(
        j["examples"]["R8"],
        json!([{"rule": "R8", "node": "Khách hàng/Công ty ABC.md",
                "detail": "title trùng (không phân biệt hoa thường, dấu): Công ty ABC"}])
    );
    let keys: Vec<&String> = j["counts"].as_object().unwrap().keys().collect();
    assert_eq!(keys.first().map(|k| k.as_str()), Some("R0"));
    assert_eq!(keys.last().map(|k| k.as_str()), Some("R15"));

    let t = one.to_text();
    assert!(t.starts_with("Lệch schema v3 (luật | số chỗ | số note):\n  R0        1      1\n"));
    assert!(t.contains("\n  R10       1      1\n"));
    assert!(t.ends_with(
        "\nR8:\n  Khách hàng/Công ty ABC.md: title trùng (không phân biệt hoa thường, dấu): Công ty ABC\n"
    ));
}

#[test]
fn views_are_recreated_and_sql_kept() {
    let (v, dev) = planted();
    let db = crate::index::connect(&v.db(), false).unwrap();
    create_views(&db).unwrap();
    create_views(&db).unwrap();
    assert_eq!(deviations(&db, None, None).unwrap(), dev);
    let n: i64 = db
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'view'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n as usize, VIEWS.len() + 1);
    let r: Vec<&str> = rules();
    assert_eq!(r.first(), Some(&"v3_r0"));
    assert_eq!(r.last(), Some(&"v3_r15"));
    assert_eq!(r.len(), 16);
    // Mảnh ghép như Python (`_ANC`, `_KNOWN`).
    let r3 = &VIEWS.iter().find(|(n, _)| *n == "v3_r3").unwrap().1;
    assert!(r3.ends_with(
        "FROM fm_links e WHERE e.dst IS NULL AND EXISTS (SELECT 1 FROM schema_link k WHERE k.key = e.key AND \
         (k.owner = '*' OR EXISTS (SELECT 1 FROM schema_type_ancestor a WHERE a.type = e.st AND a.ancestor = k.owner)))"
    ));
}

/// `Fixture.test_fixture_planted_errors`: 12 lỗi cài sẵn của kg-solo đều bị bắt. Cần vault kg-solo dựng sẵn
/// (`PKM_KG_SOLO=<thư mục>`; CI so cả vault qua bộ so khớp); không có thì bỏ qua như Python khi thiếu node.
#[test]
fn fixture_planted_errors() {
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
    crate::index::build(&vault, &db, &b).unwrap();
    let key: Value =
        serde_json::from_str(&std::fs::read_to_string(vault.join(".kg-answers.json")).unwrap())
            .unwrap();
    let exp: Vec<(String, String)> = key["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["query"]["op"] == "validate")
        .flat_map(|c| c["expect"].as_array().cloned().unwrap_or_default())
        .map(|x| {
            (
                x["rule"].as_str().unwrap().to_owned(),
                x["note"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let conn = crate::index::connect(&db, true).unwrap();
    let got: HashSet<(String, String)> = deviations(&conn, None, None)
        .unwrap()
        .into_iter()
        .map(|d| (d.rule, d.node.unwrap_or_default()))
        .collect();
    let missing: Vec<_> = exp.iter().filter(|e| !got.contains(*e)).collect();
    assert_eq!(missing, Vec::<&(String, String)>::new());
    assert_eq!(exp.len(), 12);
}
