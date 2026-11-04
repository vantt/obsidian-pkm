//! Test của `material`: phần tính của `scripts/e11/tests/test_material.py` (đọc bản nháp, cả vòng tìm rồi đi graph)
//! trên vault giả nhỏ kiểu bản B. Phần ghi `uses` và ô đã tích thuộc plugin nên không chuyển ở đây.

use std::sync::Mutex;

use serde_json::json;

use super::*;
use crate::index::{self, Build, EdgeKeys};
use crate::ohs::OhsState;

const TEMPLATE_TAIL: &str = "
## Nguyên liệu

Thêm note vào `uses` ở trên; chúng hiện ở đây.

```base
filters:
  and:
    - 'this.uses.contains(file.asLink())'
```

## Bài
";

const DRAFT: &str = "Bài viết/Bài nháp.md";

fn draft_text() -> String {
    format!(
        "---\ntype: post\nstatus: nháp\nuses: []\nchannel: ''\n---\n\n## Ý tưởng một câu\n\nVườn cần người làm vườn.\n{TEMPLATE_TAIL}\nDàn ý: chăm [[Cây táo]] mỗi ngày.\n"
    )
}

fn notes() -> Vec<(&'static str, String)> {
    vec![
        (DRAFT, draft_text()),
        ("Vườn/Vườn (chủ đề).md", "---\ntype: toc\n---\n".into()),
        (
            "Vườn/Cây táo.md",
            "---\ntype: note\nrelated: [\"[[Phân bón]]\"]\n---\nTáo cần nắng.\n".into(),
        ),
        (
            "Vườn/Phân bón.md",
            "---\ntype: quote\nwise: 3\n---\nỦ phân từ lá. ==Tưới sáng sớm==\n".into(),
        ),
        (
            "Vườn/Hàng xóm cùng thư mục.md",
            "---\ntype: note\n---\nKhông liên quan.\n".into(),
        ),
        ("Khác/Đất.md", "---\ntype: note\n---\nĐất tơi.\n".into()),
        ("_Xưởng/Trang sinh.md", "---\ntype: toc\n---\n".into()),
    ]
}

/// Vault và index dựng như bản B (adapter `e11`, mọi khoá có link, link thân note).
fn vault() -> (tempfile::TempDir, Graph) {
    let tmp = tempfile::tempdir().unwrap();
    for (p, t) in notes() {
        crate::index::testutil::write(tmp.path(), p, &t);
    }
    let db = tmp.path().join(".kg").join("graph.db");
    let adapter = crate::adapter::load("e11").unwrap().unwrap();
    index::build(
        tmp.path(),
        &db,
        &Build {
            packs: vec!["capture".into()],
            opts: index::Options {
                aliases: false,
                edge_keys: EdgeKeys::Links,
                body_edges: true,
            },
            adapter: Some(adapter.as_ref()),
            ..Build::default()
        },
    )
    .unwrap();
    let g = Graph::open(&db).unwrap();
    (tmp, g)
}

fn no_hits(_: &str, _: usize) -> Result<Vec<Hit>, OhsError> {
    Ok(Vec::new())
}

fn ids(r: &Value) -> Vec<(String, String)> {
    r["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            (
                i["id"].as_str().unwrap().to_owned(),
                i["found_by"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[test]
fn sections_and_queries_skip_material_section_code_and_default_title() {
    let (fm, secs) = split_draft(&draft_text());
    assert!(fm.contains("uses: []"));
    let heads: Vec<&str> = secs.iter().map(|(h, _)| h.as_str()).collect();
    assert!(!heads.contains(&"Nguyên liệu"));
    assert!(!secs.iter().any(|(_, t)| t.contains("this.uses")));
    let qs = queries("Bài mới", &secs);
    let labels: Vec<&str> = qs.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(labels, ["Ý tưởng một câu", "Bài"]);
    assert_eq!(qs[1].1, "Dàn ý: chăm Cây táo mỗi ngày.");
    assert_eq!(
        queries("Vườn và người", &secs)[0],
        ("tên bài".to_owned(), "Vườn và người".to_owned())
    );
    assert_eq!(queries("untitled 3", &secs).len(), 2);
}

#[test]
fn queries_clean_markup_label_the_lead_and_stop_at_eight() {
    let secs = vec![
        (
            String::new(),
            "- [[A/B/Ghi chú dài|tên]] *đậm* `mã` > trích".to_owned(),
        ),
        (
            "Mục".to_owned(),
            (0..12)
                .map(|i| format!("Đoạn số {i} có đủ chữ để tìm"))
                .collect::<Vec<_>>()
                .join("\n\n"),
        ),
        ("Ngắn".to_owned(), "quá ngắn".to_owned()),
    ];
    let qs = queries("Bài mới", &secs);
    assert_eq!(qs.len(), MAX_QUERIES);
    assert_eq!(
        qs[0],
        ("đầu bài".to_owned(), "Ghi chú dài đậm mã trích".to_owned())
    );
    assert_eq!(qs[1].0, "Mục");
    let long = vec![("x".to_owned(), "chữ ".repeat(200))];
    assert_eq!(queries("Bài mới", &long)[0].1.chars().count(), 400);
}

#[test]
fn python_splitlines_breaks() {
    assert_eq!(py_lines("a\r\nb\rc\u{2028}d\n"), ["a", "b", "c", "d"]);
    assert_eq!(py_lines(""), Vec::<&str>::new());
    assert_eq!(py_lines("a\n\nb"), ["a", "", "b"]);
}

#[test]
fn draft_is_resolved_and_system_pages_refused() {
    let (tmp, _g) = vault();
    assert_eq!(
        resolve_draft(tmp.path(), "Bài viết\\Bài nháp").unwrap(),
        DRAFT
    );
    assert!(
        matches!(resolve_draft(tmp.path(), "Bài viết/Không có"), Err(Error::Draft(m)) if m.contains("không thấy bài"))
    );
    assert!(
        matches!(resolve_draft(tmp.path(), "_Xưởng/Trang sinh"), Err(Error::Draft(m)) if m.contains("hệ thống"))
    );
}

#[test]
fn graph_reaches_notes_through_edges_not_topic_pages() {
    // Không có hit nào: link trong thân bài là cửa vào duy nhất; graph đi từ bản nháp.
    let (tmp, g) = vault();
    let r = gather(&g, tmp.path(), DRAFT, &Options::default(), &[], &no_hits).unwrap();
    let items = ids(&r);
    let find = |id: &str| {
        r["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["id"] == id)
            .cloned()
    };
    let apple = find("Vườn/Cây táo.md").expect("note đã link");
    assert_eq!(apple["used"], true);
    let manure = find("Vườn/Phân bón.md").expect("graph đưa tới");
    assert_eq!(manure["found_by"], "graph");
    assert_eq!(manure["highlights"], 1);
    assert_eq!(manure["first_highlights"], json!(["Tưới sáng sớm"]));
    assert_eq!(manure["wise"], "3");
    let via = manure["via"].as_array().unwrap();
    assert_eq!(via.last().unwrap()["to"], "Vườn/Phân bón.md");
    assert!(via.iter().all(|s| s.get("origin").is_some()), "{via:?}");
    for absent in [
        "Vườn/Hàng xóm cùng thư mục.md",
        "Vườn/Vườn (chủ đề).md",
        DRAFT,
    ] {
        assert!(
            !items.iter().any(|(i, _)| i == absent),
            "{absent}: {items:?}"
        );
    }
    assert_eq!(r["draft_seed"], true);
    assert_eq!(r["methods"], json!({"ohs": 3}));
    assert_eq!(r["queries"][0], json!(["tên bài", "Bài nháp"]));
    assert_eq!(
        r["queries"][1],
        json!(["Ý tưởng một câu", "Vườn cần người làm vườn."])
    );
    assert!(r["edges"]["accepted"].as_i64().unwrap() > 0);
}

#[test]
fn every_paragraph_is_its_own_search_and_scores_are_normalised_per_query() {
    let (tmp, g) = vault();
    let seen = Mutex::new(Vec::new());
    let search = |q: &str, limit: usize| -> Result<Vec<Hit>, OhsError> {
        seen.lock().unwrap().push((q.to_owned(), limit));
        Ok(match q {
            "Vườn cần người làm vườn." => vec![
                ("Khác/Đất.md".into(), 0.5),
                ("Vườn/Cây táo.md".into(), 0.25),
                ("_Xưởng/Trang sinh.md".into(), 0.2),
                (DRAFT.into(), 0.1),
            ],
            "Bài nháp" => Vec::new(),
            _ => vec![("Vườn/Cây táo.md".into(), 2.0), ("Không có.md".into(), 1.0)],
        })
    };
    let extra = vec![("Vườn/Phân bón.md".to_owned(), 1.0)];
    let o = Options {
        k: 12,
        per_query: 4,
    };
    let r = gather(&g, tmp.path(), DRAFT, &o, &extra, &search).unwrap();
    let mut seen = seen.lock().unwrap().clone();
    seen.sort();
    assert_eq!(
        seen,
        [
            ("Bài nháp".to_owned(), 4),
            ("Dàn ý: chăm Cây táo mỗi ngày.".to_owned(), 4),
            ("Vườn cần người làm vườn.".to_owned(), 4)
        ]
    );
    assert_eq!(r["methods"], json!({"ohs-hits": 1, "ohs": 3}));
    let text: Vec<(String, String)> = ids(&r).into_iter().filter(|(_, f)| f == "ohs").collect();
    let mut found: Vec<&str> = text.iter().map(|(i, _)| i.as_str()).collect();
    found.sort_unstable();
    assert_eq!(
        found,
        ["Khác/Đất.md", "Vườn/Cây táo.md", "Vườn/Phân bón.md"]
    );
    let item = |id: &str| {
        r["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["id"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(
        item("Vườn/Cây táo.md")["sections"],
        json!(["Ý tưởng một câu", "Bài"])
    );
    assert_eq!(item("Vườn/Cây táo.md")["ohs_score"], 1.0);
    assert_eq!(item("Khác/Đất.md")["ohs_score"], 1.0);
    assert_eq!(item("Vườn/Phân bón.md")["sections"], json!(["OHS (agent)"]));
    assert_eq!(r["missing"], json!(["Không có.md"]));
    assert!(
        r["topics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"] == "Vườn/Vườn (chủ đề).md")
    );
    assert!(!ids(&r).iter().any(|(i, _)| i.starts_with("_Xưởng/")));

    let short = gather(
        &g,
        tmp.path(),
        DRAFT,
        &Options { k: 1, per_query: 4 },
        &extra,
        &search,
    )
    .unwrap();
    assert_eq!(ids(&short).iter().filter(|(_, f)| f == "ohs").count(), 1);
}

#[test]
fn ohs_not_ready_is_returned_without_fallback() {
    let (tmp, g) = vault();
    let search = |_: &str, _: usize| -> Result<Vec<Hit>, OhsError> {
        Err(OhsError::NotReady(OhsState::Starting))
    };
    match gather(&g, tmp.path(), DRAFT, &Options::default(), &[], &search) {
        Err(Error::Ohs(e)) => assert_eq!(e.not_ready_error().unwrap()["state"], "starting"),
        other => panic!("phải là ohs_not_ready: {other:?}"),
    }
}

#[test]
fn hit_files_accept_lists_and_search_results() {
    assert_eq!(
        hits_from_value(&json!(["A.md", {"path": "B.md", "score": 0.5}, {"path": "C.md"}]))
            .unwrap(),
        [
            ("A.md".to_owned(), 1.0),
            ("B.md".to_owned(), 0.5),
            ("C.md".to_owned(), 1.0)
        ]
    );
    assert_eq!(
        hits_from_value(&json!({"results": [{"path": "A.md", "score": 2}]})).unwrap(),
        [("A.md".to_owned(), 2.0)]
    );
    assert!(hits_from_value(&json!({"x": 1})).is_err());
    assert!(hits_from_value(&json!([1])).is_err());
}
