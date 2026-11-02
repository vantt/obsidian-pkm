//! Test của `tools`, chuyển từ `scripts/kg/tests/test_kg.py` (`Api.test_long_result_envelope`) và
//! `scripts/e11/tests/test_kg_adapter.py` (lớp `HiddenAndLayers`) ở sha ghim, cộng điểm vào chung
//! [`call`] như `Server.call`.

use std::path::Path;

use serde_json::{Value, json};

use super::*;
use crate::index::testutil::{TestVault, clean_notes, write};
use crate::index::{self, Build, EdgeKeys, Options};

fn api() -> TestVault {
    let v = TestVault::new(&clean_notes());
    v.build(&["business"]);
    v
}

fn ctx(v: &TestVault) -> Context {
    let t = Tools::open(&v.db()).expect("mở index");
    Context::new(t, v.path(), Some("Kho của tôi".into()))
}

fn ids(r: &Map<String, Value>, k: &str) -> Vec<String> {
    r["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| match x {
            Value::String(s) => s.clone(),
            x => x[k].as_str().unwrap().to_owned(),
        })
        .collect()
}

#[test]
fn long_result_envelope() {
    let v = api();
    let mut t = Tools::with_limit(Graph::open(&v.db()).unwrap(), 3);
    let r = t
        .select(json!({"type": "claim"}).as_object().unwrap(), None, false)
        .unwrap();
    assert_eq!(r["truncated"], true);
    assert_eq!(r["items"].as_array().unwrap().len(), 3);
    assert_eq!(r["by_type"], json!({"claim": r["count"]}));
    let rest = t.more(r["handle"].as_str().unwrap(), 3, Some(100));
    assert_eq!(
        r["items"].as_array().unwrap().len() + rest["items"].as_array().unwrap().len(),
        usize::try_from(r["count"].as_u64().unwrap()).unwrap()
    );
    let keys: Vec<&str> = r.keys().map(String::as_str).collect();
    assert_eq!(keys, ["count", "by_type", "items", "truncated", "handle"]);
}

#[test]
fn handle_like_python() {
    // Handle của golden kg-solo: `find(name="Chất", type="claim")`, `select(type="claim")`.
    assert_eq!(handle("find", &json!(["Chất", "claim"])), "find-1f64a289cb");
    assert_eq!(
        handle("select", &json!([{"type": "claim"}, false])),
        "select-145687450a"
    );
}

#[test]
fn short_results_have_no_handle_and_cache_is_bounded() {
    let v = api();
    let mut t = Tools::with_limit(Graph::open(&v.db()).unwrap(), 1);
    let r = t.find("cong ty abc", None, None).unwrap();
    assert_eq!(
        r,
        json!({"count": 1, "items": r["items"]})
            .as_object()
            .unwrap()
            .clone()
    );
    for i in 0..(CACHE + 5) {
        let w = json!({"type": "claim", "n": i.to_string()});
        // Không note nào có `n`: kết quả rỗng, không vào bộ nhớ đệm.
        assert!(
            !t.select(w.as_object().unwrap(), None, false)
                .unwrap()
                .contains_key("handle")
        );
        t.find(&format!("cong ty {}", "x".repeat(i)), None, Some(-1))
            .unwrap();
    }
    assert!(t.cache.len() <= CACHE);
    assert!(t.more("find-0000000000", 0, None).contains_key("error"));
}

#[test]
fn call_reports_unknown_notes_with_suggestions() {
    let v = api();
    let mut c = ctx(&v);
    let r = call(&mut c, "neighbors", json!({"node": "Công ty ABC"})).unwrap();
    assert_eq!(
        r,
        json!({"error": "no note with id 'Công ty ABC'; ids are exact vault paths",
               "did_you_mean": ["Khách hàng/Công ty ABC.md"]})
    );
    let r = call(&mut c, "note", json!({"node": "Khác/Không có.md"})).unwrap();
    assert_eq!(r["did_you_mean"], json!([]));
    // Note id được chuẩn hoá NFC trước khi tra.
    let nfd: String =
        unicode_normalization::UnicodeNormalization::nfd("Khách hàng/Công ty ABC.md").collect();
    let r = call(&mut c, "provenance", json!({"claim": nfd})).unwrap();
    assert_eq!(r["claim"], "Khách hàng/Công ty ABC.md");
}

#[test]
fn default_vault_name_like_abspath_basename() {
    assert_eq!(
        default_vault_name(Path::new("/x/kg-solo/")).as_deref(),
        Some("kg-solo")
    );
    assert_eq!(
        default_vault_name(Path::new("/x/kg-solo/sub/..")).as_deref(),
        Some("kg-solo")
    );
    assert_eq!(
        default_vault_name(Path::new("/x/./Kho")).as_deref(),
        Some("Kho")
    );
    #[cfg(unix)]
    assert_eq!(default_vault_name(Path::new("/")), None);
}

#[test]
fn call_note_has_link_and_body() {
    let v = api();
    let mut c = ctx(&v);
    let r = call(&mut c, "note", json!({"node": "Họp/Họp 2026-01-05.md"})).unwrap();
    assert_eq!(
        r["url"],
        "obsidian://open?vault=Kho%20c%E1%BB%A7a%20t%C3%B4i&file=H%E1%BB%8Dp%2FH%E1%BB%8Dp%202026-01-05"
    );
    assert_eq!(
        r["body"],
        "# Họp 2026-01-05\n\n- Lead tăng thì doanh thu tăng. ^q1"
    );
    assert_eq!(r["blocks"], json!(["q1"]));
    let r = call(
        &mut c,
        "note",
        json!({"node": "Họp/Họp 2026-01-05.md", "body_chars": 4}),
    )
    .unwrap();
    assert_eq!(r["body"], "# Họ … [53 chars]");
    let r = call(
        &mut c,
        "note",
        json!({"node": "Họp/Họp 2026-01-05.md", "body_chars": 0}),
    )
    .unwrap();
    assert!(!r.as_object().unwrap().contains_key("body"));
    let mut quiet = Context::new(Tools::open(&v.db()).unwrap(), v.path(), None);
    let r = call(&mut quiet, "note", json!({"node": "Họp/Họp 2026-01-05.md"})).unwrap();
    assert!(!r.as_object().unwrap().contains_key("url"));
}

#[test]
fn call_errors_like_server_call() {
    let v = api();
    let mut c = ctx(&v);
    let n = "Yếu tố/Doanh thu.md";
    assert_eq!(
        call(&mut c, "loops", json!({"node": n})).unwrap(),
        json!({"error": "loops needs max_len (e.g. 6)"})
    );
    assert_eq!(
        call(&mut c, "select", json!({})).unwrap(),
        json!({"error": "select needs at least one filter (type, status, proposed_by or where)"})
    );
    assert_eq!(
        call(&mut c, "more", json!({"handle": "find-0000000000"})).unwrap(),
        json!({"error": "không còn kết quả find-0000000000; gọi lại tool gốc"})
    );
    assert_eq!(
        call(&mut c, "nope", json!({})).unwrap(),
        json!({"error": "unknown tool nope"})
    );
    let e = call(&mut c, "find", json!({})).unwrap_err();
    assert_eq!(e.repr(), "KeyError(\"'name'\")");
    assert_eq!(
        call(&mut c, "loops", json!({"node": n, "max_len": "x"}))
            .unwrap_err()
            .kind,
        "ValueError"
    );
    assert_eq!(
        call(&mut c, "more", json!({"handle": []}))
            .unwrap_err()
            .kind,
        "TypeError"
    );
    assert_eq!(
        call(&mut c, "related", json!({})).unwrap(),
        json!({"error": "related needs query, queries, node or hits"})
    );
    assert_eq!(
        call(&mut c, "related", json!({"query": 3}))
            .unwrap_err()
            .kind,
        "TypeError"
    );
}

#[test]
fn call_runs_each_tool_with_mcp_arguments() {
    let v = api();
    let mut c = ctx(&v);
    let r = call(&mut c, "find", json!({"name": "cong ty", "limit": 1})).unwrap();
    assert_eq!(
        (r["count"].clone(), r["truncated"].clone()),
        (json!(2), json!(true))
    );
    let h = r["handle"].as_str().unwrap().to_owned();
    let rest = call(&mut c, "more", json!({"handle": h, "offset": 1})).unwrap();
    assert_eq!(rest["items"][0]["id"], "Khách hàng/Công ty XYZ.md");
    let r = call(
        &mut c,
        "neighbors",
        json!({"node": "Người/An.md", "rels": ["works_at"], "as_of": "2026-01-01"}),
    )
    .unwrap();
    assert_eq!(
        ids(r.as_object().unwrap(), "node"),
        ["Khách hàng/Công ty XYZ.md"]
    );
    assert_eq!(r["items"][0]["type"], "organization");
    let r = call(
        &mut c,
        "reach",
        json!({"node": "Yếu tố/Mệt mỏi.md", "max_depth": 1, "rels": ["decreases"]}),
    )
    .unwrap();
    assert_eq!(
        r["items"],
        json!([{"id": "Yếu tố/Doanh thu.md", "depth": 1}, {"id": "Yếu tố/Số lead.md", "depth": 1}])
    );
    let r = call(
        &mut c,
        "paths",
        json!({"from": "Yếu tố/Mệt mỏi.md", "to": "Yếu tố/Doanh thu.md", "accepted_only": true, "rels": ["increases", "decreases"]}),
    )
    .unwrap();
    assert_eq!(r["items"][0]["sign"], "-");
    assert_eq!(r["count"], 1);
    let r = call(
        &mut c,
        "loops",
        json!({"node": "Yếu tố/Doanh thu.md", "max_len": "5"}),
    )
    .unwrap();
    assert_eq!(r["kinds"], json!({"reinforcing": 1}));
    let r = call(
        &mut c,
        "select",
        json!({"status": "proposed", "proposed_by": "llm"}),
    )
    .unwrap();
    assert_eq!(r["items"], json!(["Claim/Mệt giảm doanh thu.md"]));
    let r = call(
        &mut c,
        "valid",
        json!({"type": "contract", "as_of": "2025-06-01"}),
    )
    .unwrap();
    assert_eq!(r["items"], json!(["Hợp đồng/HĐ ABC 2025.md"]));
}

/// Mười tool giữ nguyên chữ của Python ở sha ghim; `search`, `read` (mới) và `related` (viết lại) là lệch có chủ ý
/// của một cửa, đứng đúng chỗ trong danh sách.
#[test]
fn definitions_match_the_golden_tools_list_except_one_door_tools() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let lock = std::fs::read_to_string(root.join("spec/spec.lock")).unwrap();
    let sha = lock
        .lines()
        .find_map(|l| l.trim().strip_prefix("sha = \"")?.strip_suffix('"'))
        .unwrap();
    let path = root.join(format!("parity/golden/{sha}/mcp/tools_list.json"));
    let golden: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let golden: Vec<&Value> = golden["tools"].as_array().unwrap().iter().collect();
    let ours = tools_list(false);
    let ours: Vec<&Value> = ours["tools"].as_array().unwrap().iter().collect();
    let names: Vec<&str> = ours.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, NAMES_IN_LIST);
    let one_door = ["search", "read", "related"];
    let kept: Vec<&&Value> = ours
        .iter()
        .filter(|t| !one_door.contains(&t["name"].as_str().unwrap()))
        .collect();
    let golden_kept: Vec<&&Value> = golden
        .iter()
        .filter(|t| !one_door.contains(&t["name"].as_str().unwrap()))
        .collect();
    assert_eq!(kept.len(), 10);
    assert_eq!(kept, golden_kept);
    let mut sorted = names.clone();
    sorted.sort_unstable();
    let mut all = NAMES.to_vec();
    all.sort_unstable();
    assert_eq!(sorted, all);
}

const NAMES_IN_LIST: [&str; 13] = [
    "find",
    "search",
    "note",
    "read",
    "neighbors",
    "reach",
    "paths",
    "loops",
    "provenance",
    "select",
    "valid",
    "related",
    "more",
];

/// Cỡ của `tools/list` (JSON gọn) không phình dần: mỗi phiên agent nạp cả danh sách. Đo khi thêm một cửa: 17.664
/// ký tự không có trang hệ thống (11 tool của Python: 13.403), 19.160 khi có; ngân sách chừa khoảng 5%.
#[test]
fn tools_list_stays_within_its_size_budget() {
    let size = |hidden| tools_list(hidden).to_string().chars().count();
    assert!(size(false) <= TOOLS_LIST_BUDGET, "{}", size(false));
    assert!(size(true) <= TOOLS_LIST_BUDGET_HIDDEN, "{}", size(true));
}

const TOOLS_LIST_BUDGET: usize = 18_500;
const TOOLS_LIST_BUDGET_HIDDEN: usize = 20_000;

#[test]
fn definitions_add_system_only_when_vault_has_system_pages() {
    let keys = |d: &ToolDef| d.properties.keys().cloned().collect::<Vec<_>>();
    for d in definitions(true) {
        let k = keys(&d);
        match d.name {
            "neighbors" | "reach" | "loops" => {
                let i = k.iter().position(|x| x == "accepted_only").unwrap();
                assert_eq!(k[i + 1..], ["system", "limit"], "{}", d.name);
            }
            "paths" | "related" | "search" => {
                assert_eq!(k.last().map(String::as_str), Some("system"), "{}", d.name)
            }
            "select" | "valid" => assert_eq!(k[k.len() - 2..], ["limit", "system"], "{}", d.name),
            _ => assert!(!k.contains(&"system".to_owned()), "{}", d.name),
        }
    }
    assert!(
        definitions(false)
            .iter()
            .all(|d| !keys(d).contains(&"system".to_owned()))
    );
}

// ------------------------------------------------------------------------------------------------ vault e11

const P: &str = "_Đề xuất/";

/// Note đề xuất của bản B (`prop` của `test_kg_adapter.py`).
fn prop(
    kind: &str,
    status: &str,
    notes: &[&str],
    by: &str,
    data: Option<Value>,
    extra: &str,
) -> String {
    let ex = if by == "model" {
        "multilingual-e5-small@schema_v3".to_owned()
    } else {
        format!("propose.py {kind}@schema_v3")
    };
    let links: Vec<String> = notes.iter().map(|n| format!("[[{n}]]")).collect();
    let mut fm = vec![
        format!("type: {}", json!("proposal")),
        format!("kind: {}", json!(kind)),
        format!("status: {}", json!(status)),
        format!("proposed_by: {}", json!(by)),
        format!("extracted_by: {}", json!(ex)),
        format!("notes: {}", json!(links)),
    ];
    if let Some(d) = data {
        fm.push(format!("data: {d}"));
    }
    let body: String = notes.iter().map(|n| format!("![[{n}]]\n")).collect();
    format!("---\n{}{extra}\n---\n{body}", fm.join("\n"))
}

/// Vault giả kiểu bản B của `test_kg_adapter.py` (không có nội dung thật).
fn e11_vault(root: &Path) {
    let notes: Vec<(String, String)> = vec![
        (
            "Dự án/Alpha/Alpha (chủ đề).md".into(),
            "---\ntype: toc\n---\n".into(),
        ),
        (
            "Dự án/Alpha/a1.md".into(),
            "---\ntype: note\n---\nXem [[a2]].\n".into(),
        ),
        ("Dự án/Alpha/a2.md".into(), "---\ntype: note\n---\n".into()),
        (
            "Dự án/Alpha/con/a3.md".into(),
            "---\ntype: note\n---\n".into(),
        ),
        (
            "Dự án/Beta/Beta (chủ đề).md".into(),
            "---\ntype: toc\n---\n".into(),
        ),
        (
            "Dự án/Beta/b1.md".into(),
            "---\ntype: note\n---\nHôm nay: [[Hôm nay]]\n".into(),
        ),
        (
            "Dự án/Beta/b2.md".into(),
            "---\ntype: note\nduplicate_of: \"[[b1]]\"\n---\n".into(),
        ),
        (
            "Đọc/t1.md".into(),
            "---\ntype: quote\nauthor: \"[[@ai_do]]\"\n---\n".into(),
        ),
        ("Người/Lan.md".into(), "---\ntype: person\n---\n".into()),
        (
            "Trang chủ.md".into(),
            "---\ntype: toc\n---\n[[a1]] [[b1]] [[a2]] [[b2]]\n".into(),
        ),
        (
            "_Xưởng/Hôm nay.md".into(),
            "---\ntype: toc\n---\n[[a1]]\n".into(),
        ),
        (
            format!("{P}P1 liên quan.md"),
            prop(
                "related",
                "chờ",
                &["Dự án/Alpha/a1", "Dự án/Beta/b1"],
                "model",
                Some(json!({"score": 0.95})),
                "",
            ),
        ),
        (
            format!("{P}P2 trùng.md"),
            prop(
                "duplicate",
                "đã áp dụng",
                &["Dự án/Beta/b1", "Dự án/Beta/b2"],
                "rule",
                None,
                "\nasserted_at: \"2026-10-20\"",
            ),
        ),
        (
            format!("{P}P3 bỏ.md"),
            prop(
                "related",
                "bỏ",
                &["Dự án/Alpha/a2", "Dự án/Beta/b2"],
                "model",
                Some(json!({"score": 0.91})),
                "",
            ),
        ),
        (
            format!("{P}P4 người.md"),
            prop(
                "person",
                "chờ",
                &["Dự án/Alpha/con/a3"],
                "rule",
                Some(json!({"value": "Lan"})),
                "",
            ),
        ),
        (
            format!("{P}P5 sửa link.md"),
            prop(
                "fix-link",
                "chờ",
                &["Dự án/Alpha/a1", "Dự án/Alpha/a2"],
                "rule",
                Some(json!({"from_text": "a 2", "to": "Dự án/Alpha/a2.md"})),
                "",
            ),
        ),
        (
            format!("{P}P6 nhận.md"),
            prop(
                "related",
                "nhận",
                &["Dự án/Alpha/con/a3", "Dự án/Beta/b2"],
                "model",
                Some(json!({"score": 0.9})),
                "\nasserted_at: \"2026-10-21\"",
            ),
        ),
        (
            format!("{P}P7 ý.md"),
            prop("idea", "chờ", &["Dự án/Alpha/a1"], "rule", None, ""),
        ),
    ];
    for (p, t) in notes {
        write(root, &p, &t);
    }
    let pairs = json!([
        {"a": "Dự án/Alpha/a1.md", "b": "Dự án/Beta/b1.md", "score": 0.95},
        {"a": "Dự án/Alpha/a2.md", "b": "Dự án/Beta/b2.md", "score": 0.91},
        {"a": "Dự án/Alpha/a1.md", "b": "Dự án/Beta/b2.md", "score": 0.92},
        {"a": "Dự án/Alpha/a1.md", "b": "Không có.md", "score": 0.99}
    ]);
    write(root, ".e11/similar-pairs.json", &pairs.to_string());
    let log = json!({"path": "Đọc/t1.md", "rules": ["R2"], "old_frontmatter": "---\ntype: quote\n---",
                     "new_frontmatter": "---\ntype: quote\nauthor: \"[[@ai_do]]\"\n---"});
    write(root, "_E11/normalize-log.jsonl", &format!("{log}\n"));
    write(
        root,
        "_E11/workshop-ui-manifest.json",
        &json!({"files": {"_Xưởng/Hôm nay.md": "x"}}).to_string(),
    );
}

/// `HiddenAndLayers.setUpClass`: build vault e11 (gói `capture`, đọc như Obsidian, adapter `e11`), mở `Tools`.
fn e11() -> (tempfile::TempDir, Tools) {
    let tmp = tempfile::tempdir().unwrap();
    let vault = tmp.path().join("vault");
    e11_vault(&vault);
    let db = tmp.path().join("graph.db");
    let a = crate::adapter::load("e11").unwrap().unwrap();
    let b = Build {
        packs: vec!["capture".into()],
        opts: Options {
            aliases: false,
            edge_keys: EdgeKeys::Links,
            body_edges: true,
        },
        adapter: Some(a.as_ref()),
        ..Build::default()
    };
    index::build(&vault, &db, &b).expect("build");
    let t = Tools::open(&db).unwrap();
    (tmp, t)
}

fn sys() -> EdgeArgs {
    EdgeArgs {
        system: true,
        ..EdgeArgs::default()
    }
}

#[test]
fn system_hidden_unless_asked() {
    let (_tmp, mut t) = e11();
    let a1 = "Dự án/Alpha/a1.md";
    let got = ids(
        &t.neighbors(a1, "both", &EdgeArgs::default(), None).unwrap(),
        "node",
    );
    for hub in [
        "_Xưởng/Hôm nay.md",
        "Trang chủ.md",
        &format!("{P}P1 liên quan.md"),
    ] {
        assert!(!got.iter().any(|x| x == hub), "{hub}");
    }
    let got = ids(&t.neighbors(a1, "both", &sys(), None).unwrap(), "node");
    for hub in [
        "_Xưởng/Hôm nay.md",
        "Trang chủ.md",
        &format!("{P}P1 liên quan.md"),
    ] {
        assert!(got.iter().any(|x| x == hub), "{hub}");
    }
    let reach = ids(
        &t.reach(a1, "both", None, &EdgeArgs::default(), Some(1000))
            .unwrap(),
        "id",
    );
    assert!(
        !reach
            .iter()
            .any(|r| r.starts_with('_') || r.starts_with("Trang chủ"))
    );
}

#[test]
fn paths_avoid_hubs() {
    let (_tmp, mut t) = e11();
    let acc = EdgeArgs {
        accepted_only: true,
        ..EdgeArgs::default()
    };
    let ps = t
        .paths("Dự án/Beta/b1.md", "Dự án/Alpha/a1.md", 6, &acc, None)
        .unwrap();
    // Chỉ nối qua trang `_Xưởng/`.
    assert_eq!(ps["items"], json!([]));
    let acc_sys = EdgeArgs {
        system: true,
        ..acc
    };
    let ps = t
        .paths("Dự án/Beta/b1.md", "Dự án/Alpha/a1.md", 6, &acc_sys, None)
        .unwrap();
    assert_eq!(
        ps["items"][0]["nodes"],
        json!(["Dự án/Beta/b1.md", "_Xưởng/Hôm nay.md", "Dự án/Alpha/a1.md"])
    );
    let ps = t
        .paths(
            "Trang chủ.md",
            "Dự án/Beta/b1.md",
            6,
            &EdgeArgs::default(),
            None,
        )
        .unwrap();
    assert_eq!(
        ps["items"][0]["nodes"],
        json!(["Trang chủ.md", "Dự án/Beta/b1.md"])
    );
}

#[test]
fn layers() {
    let (_tmp, mut t) = e11();
    let rel = |accepted_only: bool, statuses: Option<&[&str]>| EdgeArgs {
        rels: Some(vec!["related".into()]),
        statuses: statuses.map(|s| s.iter().map(|x| (*x).to_owned()).collect()),
        accepted_only,
        ..EdgeArgs::default()
    };
    let n = t
        .neighbors("Dự án/Alpha/a1.md", "both", &rel(false, None), None)
        .unwrap();
    let mut got: Vec<(Value, Value, Value)> = n["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| (x["node"].clone(), x["layer"].clone(), x["origin"].clone()))
        .collect();
    got.sort_by_key(|x| x.0.to_string());
    assert_eq!(
        got,
        [
            (
                json!("Dự án/Beta/b1.md"),
                json!("proposed"),
                json!("proposal")
            ),
            (json!("Dự án/Beta/b2.md"), json!("proposed"), json!("ohs")),
        ]
    );
    let n = t
        .neighbors("Dự án/Alpha/a1.md", "both", &rel(true, None), None)
        .unwrap();
    assert_eq!(n["items"], json!([]));
    let n = t
        .neighbors("Dự án/Alpha/a2.md", "both", &rel(false, None), None)
        .unwrap();
    assert_eq!(n["items"], json!([]));
    let n = t
        .neighbors(
            "Dự án/Alpha/a2.md",
            "both",
            &rel(false, Some(&["rejected"])),
            None,
        )
        .unwrap();
    let layers: Vec<&Value> = n["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| &x["layer"])
        .collect();
    assert_eq!(layers, [&json!("rejected")]);
}

#[test]
fn select_skips_system() {
    let (_tmp, mut t) = e11();
    let w = json!({"type": "toc"});
    let w = w.as_object().unwrap();
    assert_eq!(
        ids(&t.select(w, None, false).unwrap(), ""),
        [
            "Dự án/Alpha/Alpha (chủ đề).md",
            "Dự án/Beta/Beta (chủ đề).md"
        ]
    );
    let all = ids(&t.select(w, None, true).unwrap(), "");
    assert!(all.iter().any(|x| x == "Trang chủ.md"));
    assert!(all.iter().any(|x| x == "_Xưởng/Hôm nay.md"));
}

#[test]
fn related_skips_system() {
    let (tmp, t) = e11();
    let mut c = Context::new(t, &tmp.path().join("vault"), None);
    let r = call(
        &mut c,
        "related",
        json!({"hits": [["Dự án/Alpha/a1.md", 1.0]], "k": 10}),
    )
    .unwrap();
    let got: Vec<&str> = r["related"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["id"].as_str().unwrap())
        .collect();
    assert!(got.contains(&"Dự án/Beta/b1.md"), "{got:?}");
    assert!(
        !got.iter()
            .any(|g| g.starts_with('_') || g.starts_with("Trang chủ"))
    );
}

/// `Protocol.test_related` của `test_mcp.py`, qua điểm vào chung.
#[test]
fn call_related_with_hits() {
    let v = api();
    let mut c = ctx(&v);
    let hits = json!([
        {"path": "Yếu tố/Số lead.md", "score": 0.9},
        {"path": "Khách hàng/Không có.md", "score": 0.5},
        "Người/An.md"
    ]);
    let r = call(&mut c, "related", json!({"hits": hits, "k": 5})).unwrap();
    assert_eq!(r["missing"], json!(["Khách hàng/Không có.md"]));
    assert_eq!(r["seed_source"], "hits");
    let seeds: Vec<&str> = r["seeds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["id"].as_str().unwrap())
        .collect();
    assert_eq!(seeds, ["Yếu tố/Số lead.md", "Người/An.md"]);
    let ids: Vec<&str> = r["related"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["id"].as_str().unwrap())
        .collect();
    assert!(ids[..3].contains(&"Yếu tố/Doanh thu.md"), "{ids:?}");
    let top = &r["related"][0];
    assert_eq!(top["found_by"], "graph");
    assert!(seeds.contains(&top["via"][0]["from"].as_str().unwrap()));
    let keys: Vec<&str> = r.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        ["seeds", "related", "missing", "method", "seed_source"]
    );

    let r2 = call(
        &mut c,
        "related",
        json!({"hits": hits, "k": 5, "types": ["organization"]}),
    )
    .unwrap();
    let rel = r2["related"].as_array().unwrap();
    assert!(!rel.is_empty() && rel.iter().all(|x| x["type"] == "organization"));

    let r3 = call(
        &mut c,
        "related",
        json!({"hits": ["Yếu tố/Mệt mỏi.md"], "k": 5, "accepted_only": true}),
    )
    .unwrap();
    let rel = r3["related"].as_array().unwrap();
    assert!(!rel.is_empty());
    for x in rel {
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

/// Chỉ có câu hỏi mà phiên không có OHS, hoặc OHS thiếu gói: `ohs_not_ready` với `state: broken` (D7), không lùi
/// về kết quả kém.
#[test]
fn call_related_query_without_ohs_is_not_ready() {
    let v = api();
    let mut c = ctx(&v);
    let r = call(&mut c, "related", json!({"query": "lead"})).unwrap();
    let keys: Vec<&str> = r.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["error", "state", "progress", "eta_s", "action"]);
    assert_eq!(r["error"], "ohs_not_ready");
    assert_eq!(r["state"], "broken");
    assert!(r["action"].as_str().unwrap().contains("--ohs"));

    let missing = v.path().join("không có gói OHS");
    let ohs = crate::ohs::Ohs::launch(Some(&missing), crate::ohs::OhsConfig::new(v.path()));
    let mut c = ctx(&v).with_ohs(Arc::new(ohs));
    assert!(c.ohs().is_some());
    let r = call(&mut c, "related", json!({"query": "lead", "hits": []})).unwrap();
    assert_eq!(r["error"], "ohs_not_ready");
    assert_eq!(r["state"], "broken");
    assert!(r["action"].as_str().unwrap().contains("Chưa cài gói OHS"));
    // `hits` có thì không cần OHS.
    let r = call(
        &mut c,
        "related",
        json!({"query": "lead", "hits": ["Người/An.md"]}),
    )
    .unwrap();
    assert_eq!(r["seed_source"], "hits");
    c.set_ohs(None);
    assert!(c.ohs().is_none());
}

// ------------------------------------------------------------------------------------------------ một cửa

/// `read` đọc thẳng file: không cần OHS, đúng thứ tự, id sai chỉ hỏng mục của nó.
#[test]
fn read_returns_bodies_in_order_and_per_item_errors() {
    let v = api();
    let n = "Yếu tố/Doanh thu.md";
    write(
        v.path(),
        n,
        "---\ntype: factor\n---\n\n# Doanh thu\nDòng một \"trích\".\nDòng hai.\n\n",
    );
    let mut c = ctx(&v);
    let r = call(
        &mut c,
        "read",
        json!({"nodes": [n, "Yếu tố/Doanh thu", "../ngoài.md", "Người/An.md"]}),
    )
    .unwrap();
    let items = r["items"].as_array().unwrap();
    assert_eq!(items.len(), 4);
    let body = "# Doanh thu\nDòng một \"trích\".\nDòng hai.";
    assert_eq!(
        items[0],
        json!({"id": n, "title": "Doanh thu", "type": "factor",
               "url": "obsidian://open?vault=Kho%20c%E1%BB%A7a%20t%C3%B4i&file=Y%E1%BA%BFu%20t%E1%BB%91%2FDoanh%20thu",
               "chars": body.chars().count(), "truncated": false, "body": body})
    );
    assert_eq!(items[1]["id"], "Yếu tố/Doanh thu");
    assert!(
        items[1]["error"]
            .as_str()
            .unwrap()
            .contains("no note with id")
    );
    assert_eq!(items[1]["did_you_mean"][0], n);
    assert_eq!(items[2]["id"], "../ngoài.md");
    assert!(items[2]["error"].as_str().unwrap().contains("'..'"));
    assert_eq!(items[3]["id"], "Người/An.md");
    assert_eq!(items[3]["type"], "person");

    // `max_chars` và `offset` tính theo ký tự; `chars` là độ dài cả thân.
    let r = call(
        &mut c,
        "read",
        json!({"nodes": n, "max_chars": 5, "offset": 2}),
    )
    .unwrap();
    let it = &r["items"][0];
    assert_eq!(it["body"], "Doanh");
    assert_eq!(it["truncated"], true);
    assert_eq!(it["chars"], body.chars().count());
    let r = call(
        &mut c,
        "read",
        json!({"nodes": [n], "max_chars": 0, "offset": 2}),
    )
    .unwrap();
    assert_eq!(r["items"][0]["truncated"], false);
    assert_eq!(r["items"][0]["body"].as_str().unwrap(), &body[2..]);
}

#[test]
fn read_rejects_bad_arguments_as_tool_errors() {
    let v = api();
    let mut c = ctx(&v);
    assert_eq!(
        call(&mut c, "read", json!({})).unwrap()["error"],
        "read needs nodes (note ids, as returned by search / find / related)"
    );
    let many: Vec<String> = (0..21).map(|i| format!("n{i}.md")).collect();
    assert!(
        call(&mut c, "read", json!({ "nodes": many })).unwrap()["error"]
            .as_str()
            .unwrap()
            .contains("at most 20")
    );
    assert_eq!(
        call(
            &mut c,
            "read",
            json!({"nodes": ["Người/An.md"], "offset": -1})
        )
        .unwrap(),
        json!({"error": "offset must be >= 0"})
    );
    assert_eq!(
        call(&mut c, "read", json!({"nodes": [3]}))
            .unwrap_err()
            .kind,
        "TypeError"
    );
}

/// Ngân sách của cả kết quả: thân bị cắt đều, kết quả in JSON gọn không vượt ngân sách, thân ngắn giữ nguyên.
#[test]
fn read_cuts_bodies_evenly_to_fit_the_reply_budget() {
    let v = api();
    let long = "Một dòng \"dài\" có dấu.\n".repeat(400);
    for n in ["Yếu tố/Doanh thu.md", "Người/An.md"] {
        write(v.path(), n, &format!("---\ntype: x\n---\n{long}"));
    }
    write(
        v.path(),
        "Yếu tố/Số lead.md",
        "---\ntype: factor\n---\nNgắn.",
    );
    let mut c = ctx(&v);
    let budget = 3000;
    c.set_reply_budget(Some(budget));
    let r = call(
        &mut c,
        "read",
        json!({"nodes": ["Yếu tố/Doanh thu.md", "Yếu tố/Số lead.md", "Người/An.md"], "max_chars": 0}),
    )
    .unwrap();
    let text = r.to_string();
    assert!(text.chars().count() <= budget, "{}", text.chars().count());
    let items = r["items"].as_array().unwrap();
    assert_eq!(items[1]["body"], "Ngắn.");
    assert_eq!(items[1]["truncated"], false);
    let (a, b) = (
        items[0]["body"].as_str().unwrap(),
        items[2]["body"].as_str().unwrap(),
    );
    assert_eq!(items[0]["truncated"], true);
    assert_eq!(items[2]["truncated"], true);
    assert!(a.chars().count() > 500 && a == b, "{}", a.chars().count());
    // Ngân sách lớn: không cắt gì.
    c.set_reply_budget(Some(1_000_000));
    let r = call(
        &mut c,
        "read",
        json!({"nodes": ["Người/An.md"], "max_chars": 0}),
    )
    .unwrap();
    assert_eq!(r["items"][0]["truncated"], false);
}

#[test]
fn json_prefix_counts_escapes() {
    assert_eq!(json_len("a\"b\n\u{1}đ"), 1 + 2 + 1 + 2 + 6 + 1);
    assert_eq!(json_prefix("ab\ncd", 3), "ab");
    assert_eq!(json_prefix("ab\ncd", 4), "ab\n");
    assert_eq!(json_prefix("đđ", 10), "đđ");
}

#[test]
fn inside_vault_accepts_only_relative_paths_without_dot_dot() {
    assert!(inside_vault("Người/An.md"));
    assert!(inside_vault("a..b.md"));
    assert!(!inside_vault("../x.md"));
    assert!(!inside_vault("a/../../x.md"));
    assert!(!inside_vault("a\\..\\x.md"));
    assert!(!inside_vault("/etc/passwd"));
}

/// `search` không có OHS: `ohs_not_ready` đủ năm khoá; lỗi tham số trả trước khi cần OHS.
#[test]
fn search_without_ohs_is_not_ready_and_checks_arguments_first() {
    let v = api();
    let mut c = ctx(&v);
    let r = call(&mut c, "search", json!({"query": "doanh thu"})).unwrap();
    let keys: Vec<&str> = r.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["error", "state", "progress", "eta_s", "action"]);
    assert_eq!(r["state"], "broken");
    assert_eq!(
        call(&mut c, "search", json!({"query": " "})).unwrap(),
        json!({"error": "search needs query, queries or path"})
    );
    assert!(
        call(&mut c, "search", json!({"query": "x", "mode": "vector"})).unwrap()["error"]
            .as_str()
            .unwrap()
            .starts_with("unknown mode 'vector'")
    );
    let r = call(&mut c, "search", json!({"path": "Yếu tố/Doanh thu"})).unwrap();
    assert_eq!(r["did_you_mean"][0], "Yếu tố/Doanh thu.md");
    assert_eq!(
        call(
            &mut c,
            "search",
            json!({"query": "x", "snippet_length": -3})
        )
        .unwrap(),
        json!({"error": "snippet_length must be >= 0"})
    );
}

/// `related`: `hits` đứng trước `node` (node sai bị bỏ qua khi có hits); `node` sai trả `did_you_mean`; `node` hay
/// `queries` không có OHS là `ohs_not_ready`.
#[test]
fn related_seed_precedence_and_new_inputs() {
    let v = api();
    let mut c = ctx(&v);
    let r = call(
        &mut c,
        "related",
        json!({"hits": ["Người/An.md"], "node": "Không có.md", "query": "x"}),
    )
    .unwrap();
    assert_eq!(r["seed_source"], "hits");
    let r = call(&mut c, "related", json!({"node": "Người/An"})).unwrap();
    assert_eq!(r["did_you_mean"][0], "Người/An.md");
    for args in [
        json!({"node": "Người/An.md"}),
        json!({"queries": ["lead", "khách"]}),
    ] {
        let r = call(&mut c, "related", args).unwrap();
        assert_eq!(r["error"], "ohs_not_ready");
        assert!(!r["action"].as_str().unwrap().contains("MCP ohs"));
    }
}
