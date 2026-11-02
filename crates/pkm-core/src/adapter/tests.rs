//! Test adapter bản B, chuyển từ `scripts/e11/tests/test_kg_adapter.py` ở sha ghim (lớp `AdapterEdges`,
//! `Incremental`) trên cùng vault giả nhỏ kiểu bản B (không có nội dung thật). Phần kiểm qua tool, CLI, MCP chuyển
//! cùng các module đó.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use rusqlite::types::Value as SqlValue;
use serde_json::{Value, json};

use super::e11::{E11, is_system};
use super::*;
use crate::index::testutil::write;
use crate::index::{self, Build, BuildInfo, EdgeKeys, Options};

const P: &str = "_Đề xuất/";

/// `OPTS`: đọc như Obsidian (mọi khoá có link, link thân note), không alias.
fn opts() -> Options {
    Options {
        aliases: false,
        edge_keys: EdgeKeys::Links,
        body_edges: true,
    }
}

/// `prop(kind, status, notes, by, data, extra)`: note đề xuất với frontmatter JSON từng dòng.
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
        ("type", json!("proposal")),
        ("kind", json!(kind)),
        ("status", json!(status)),
        ("proposed_by", json!(by)),
        ("extracted_by", json!(ex)),
        ("notes", json!(links)),
    ];
    if let Some(d) = data {
        fm.push(("data", d));
    }
    let body: String = notes.iter().map(|n| format!("![[{n}]]\n")).collect();
    let lines: Vec<String> = fm.iter().map(|(k, v)| format!("{k}: {v}")).collect();
    format!("---\n{}{extra}\n---\n{body}", lines.join("\n"))
}

fn notes() -> Vec<(String, String)> {
    let n = |p: &str, t: &str| (p.to_owned(), t.to_owned());
    vec![
        n("Dự án/Alpha/Alpha (chủ đề).md", "---\ntype: toc\n---\n"),
        n("Dự án/Alpha/a1.md", "---\ntype: note\n---\nXem [[a2]].\n"),
        n("Dự án/Alpha/a2.md", "---\ntype: note\n---\n"),
        n("Dự án/Alpha/con/a3.md", "---\ntype: note\n---\n"),
        n("Dự án/Beta/Beta (chủ đề).md", "---\ntype: toc\n---\n"),
        n(
            "Dự án/Beta/b1.md",
            "---\ntype: note\n---\nHôm nay: [[Hôm nay]]\n",
        ),
        n(
            "Dự án/Beta/b2.md",
            "---\ntype: note\nduplicate_of: \"[[b1]]\"\n---\n",
        ),
        n(
            "Đọc/t1.md",
            "---\ntype: quote\nauthor: \"[[@ai_do]]\"\n---\n",
        ),
        n("Người/Lan.md", "---\ntype: person\n---\n"),
        n(
            "Trang chủ.md",
            "---\ntype: toc\n---\n[[a1]] [[b1]] [[a2]] [[b2]]\n",
        ),
        n("_Xưởng/Hôm nay.md", "---\ntype: toc\n---\n[[a1]]\n"),
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
    ]
}

fn pairs() -> Value {
    json!([
        {"a": "Dự án/Alpha/a1.md", "b": "Dự án/Beta/b1.md", "score": 0.95},
        {"a": "Dự án/Alpha/a2.md", "b": "Dự án/Beta/b2.md", "score": 0.91},
        {"a": "Dự án/Alpha/a1.md", "b": "Dự án/Beta/b2.md", "score": 0.92},
        {"a": "Dự án/Alpha/a1.md", "b": "Không có.md", "score": 0.99}
    ])
}

fn make_vault(root: &Path) {
    for (p, t) in notes() {
        write(root, &p, &t);
    }
    write(root, ".e11/similar-pairs.json", &pairs().to_string());
    let log = json!({"path": "Đọc/t1.md", "rules": ["R2"], "old_frontmatter": "---\ntype: quote\n---",
                     "new_frontmatter": "---\ntype: quote\nauthor: \"[[@ai_do]]\"\n---"});
    write(root, "_E11/normalize-log.jsonl", &format!("{log}\n"));
    write(
        root,
        "_E11/workshop-ui-manifest.json",
        &json!({"files": {"_Xưởng/Hôm nay.md": "x"}}).to_string(),
    );
}

struct Fixture {
    _tmp: tempfile::TempDir,
    vault: PathBuf,
    db: PathBuf,
    info: BuildInfo,
}

fn build_with(vault: &Path, db: &Path, a: &dyn Adapter) -> BuildInfo {
    let b = Build {
        packs: vec!["capture".into()],
        opts: opts(),
        adapter: Some(a),
        ..Build::default()
    };
    index::build(vault, db, &b).expect("build")
}

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let vault = tmp.path().join("vault");
    make_vault(&vault);
    let db = tmp.path().join("graph.db");
    let a = load("e11").unwrap().unwrap();
    let info = build_with(&vault, &db, a.as_ref());
    Fixture {
        _tmp: tmp,
        vault,
        db,
        info,
    }
}

/// `edges(**w)`: các cạnh khớp mọi điều kiện, theo `id`.
fn edges(db: &Connection, w: &[(&str, &str)]) -> Vec<Value> {
    let cols = [
        "src",
        "rel",
        "dst",
        "dst_text",
        "status",
        "origin",
        "proposed_by",
        "extracted_by",
        "score",
        "source",
        "asserted_at",
    ];
    let mut sql = format!("SELECT {} FROM edges", cols.join(", "));
    if !w.is_empty() {
        let cond: Vec<String> = w.iter().map(|(k, _)| format!("{k} = ?")).collect();
        sql += &format!(" WHERE {}", cond.join(" AND "));
    }
    sql += " ORDER BY id";
    let mut st = db.prepare(&sql).unwrap();
    st.query_map(rusqlite::params_from_iter(w.iter().map(|(_, v)| *v)), |r| {
        let mut m = serde_json::Map::new();
        for (i, c) in cols.iter().enumerate() {
            let v = match r.get::<_, SqlValue>(i)? {
                SqlValue::Null => Value::Null,
                SqlValue::Integer(x) => json!(x),
                SqlValue::Real(x) => json!(x),
                SqlValue::Text(s) => json!(s),
                SqlValue::Blob(_) => Value::Null,
            };
            m.insert((*c).to_owned(), v);
        }
        Ok(Value::Object(m))
    })
    .unwrap()
    .collect::<Result<_, _>>()
    .unwrap()
}

impl Fixture {
    fn edges(&self, w: &[(&str, &str)]) -> Vec<Value> {
        edges(&index::connect(&self.db, true).unwrap(), w)
    }
}

#[test]
fn folder_topic() {
    let f = fixture();
    let e: std::collections::HashMap<String, String> = f
        .edges(&[("origin", "folder")])
        .iter()
        .map(|x| {
            (
                x["src"].as_str().unwrap().to_owned(),
                x["dst"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(e["Dự án/Alpha/a1.md"], "Dự án/Alpha/Alpha (chủ đề).md");
    assert_eq!(e["Dự án/Alpha/con/a3.md"], "Dự án/Alpha/Alpha (chủ đề).md"); // thư mục con: trang gần nhất
    assert_eq!(e["Dự án/Beta/b2.md"], "Dự án/Beta/Beta (chủ đề).md");
    assert!(!e.contains_key("Dự án/Alpha/Alpha (chủ đề).md")); // không tự trỏ
    assert!(
        f.edges(&[("origin", "folder")])
            .iter()
            .all(|x| x["status"] == "accepted" && x["rel"] == "topic")
    );
}

#[test]
fn proposal_waiting_is_proposed() {
    let f = fixture();
    let e = f.edges(&[
        ("origin", "proposal"),
        ("src", "Dự án/Alpha/a1.md"),
        ("rel", "related"),
    ]);
    assert_eq!(e.len(), 1);
    let e = &e[0];
    assert_eq!(
        (
            &e["dst"],
            &e["status"],
            &e["proposed_by"],
            &e["score"],
            &e["source"]
        ),
        (
            &json!("Dự án/Beta/b1.md"),
            &json!("proposed"),
            &json!("model"),
            &json!(0.95),
            &json!(format!("{P}P1 liên quan.md"))
        )
    );
    assert!(e["asserted_at"].is_null());
}

#[test]
fn rejected_and_accepted() {
    let f = fixture();
    let r = f.edges(&[("src", "Dự án/Alpha/a2.md"), ("rel", "related")]);
    assert_eq!(r.len(), 1);
    assert_eq!(
        (&r[0]["status"], &r[0]["origin"]),
        (&json!("rejected"), &json!("proposal"))
    );
    let a = f.edges(&[
        ("src", "Dự án/Alpha/con/a3.md"),
        ("rel", "related"),
        ("origin", "proposal"),
    ]);
    assert_eq!(a.len(), 1);
    assert_eq!(
        (&a[0]["status"], &a[0]["asserted_at"], &a[0]["dst"]),
        (
            &json!("accepted"),
            &json!("2026-10-21"),
            &json!("Dự án/Beta/b2.md")
        )
    );
}

#[test]
fn applied_proposal_marks_vault_edge() {
    let f = fixture();
    let e = f.edges(&[("src", "Dự án/Beta/b2.md"), ("rel", "duplicate_of")]); // không thêm cạnh proposal
    assert_eq!(e.len(), 1);
    let e = &e[0];
    assert_eq!(
        (
            &e["origin"],
            &e["status"],
            &e["proposed_by"],
            &e["source"],
            &e["asserted_at"]
        ),
        (
            &json!("frontmatter"),
            &json!("accepted"),
            &json!("rule"),
            &json!(format!("{P}P2 trùng.md")),
            &json!("2026-10-20")
        )
    );
    assert_eq!(e["extracted_by"], "propose.py duplicate@schema_v3");
}

#[test]
fn who_and_fix_link() {
    let f = fixture();
    let w = f.edges(&[("rel", "who")]);
    assert_eq!(w.len(), 1);
    assert_eq!(
        (&w[0]["src"], &w[0]["dst"], &w[0]["status"]),
        (
            &json!("Dự án/Alpha/con/a3.md"),
            &json!("Người/Lan.md"),
            &json!("proposed")
        )
    );
    let l = f.edges(&[("rel", "link"), ("origin", "proposal")]);
    assert_eq!(l.len(), 1);
    assert_eq!(
        (&l[0]["src"], &l[0]["dst"]),
        (&json!("Dự án/Alpha/a1.md"), &json!("Dự án/Alpha/a2.md"))
    );
    let idea = format!("{P}P7 ý.md");
    assert!(
        f.edges(&[("source", &idea), ("origin", "proposal")])
            .is_empty()
    ); // idea: không cạnh
}

#[test]
fn ohs_pairs() {
    let f = fixture();
    let o: Vec<Value> = f
        .edges(&[("origin", "ohs")])
        .into_iter()
        .map(|x| {
            json!([
                x["src"],
                x["dst"],
                x["status"],
                x["proposed_by"],
                x["score"],
                x["source"]
            ])
        })
        .collect();
    assert_eq!(
        o,
        [json!([
            "Dự án/Alpha/a1.md",
            "Dự án/Beta/b2.md",
            "proposed",
            "model",
            0.92,
            ".e11/similar-pairs.json"
        ])]
    );
}

#[test]
fn provenance_from_logs() {
    let f = fixture();
    let a = f.edges(&[("rel", "author")]);
    assert_eq!(a.len(), 1);
    assert_eq!(
        (&a[0]["proposed_by"], &a[0]["extracted_by"]),
        (&json!("rule"), &json!("normalize.py R2@schema_v3"))
    );
    let b = f.edges(&[("src", "_Xưởng/Hôm nay.md"), ("origin", "body")]);
    assert_eq!(b.len(), 1);
    assert_eq!(
        (&b[0]["proposed_by"], &b[0]["extracted_by"]),
        (&json!("rule"), &json!("workshop_ui.py@schema_v3"))
    );
    assert!(
        f.edges(&[("origin", "frontmatter"), ("src", "Dự án/Alpha/a1.md")])
            .is_empty()
    );
    let h = f.edges(&[("src", "Dự án/Alpha/a1.md"), ("origin", "body")]);
    assert_eq!(h.len(), 1);
    assert_eq!(h[0]["proposed_by"], "human");
}

#[test]
fn proposal_body_links_dropped() {
    let f = fixture();
    assert!(
        f.edges(&[("origin", "body")])
            .iter()
            .all(|e| !e["src"].as_str().unwrap().starts_with(P))
    );
    let p1 = format!("{P}P1 liên quan.md");
    assert!(
        !f.edges(&[("origin", "frontmatter"), ("src", &p1), ("rel", "notes")])
            .is_empty()
    );
}

#[test]
fn counts() {
    let f = fixture();
    assert_eq!(
        f.info.get("edges"),
        Some("{\"accepted\": 27, \"proposed\": 4, \"rejected\": 1}")
    );
    assert_eq!(f.info.get("adapter"), Some("e11"));
    assert_eq!(f.info.get("adapter_spec"), Some("e11"));
    assert_eq!(
        f.info.get("hidden"),
        Some("[\"_E11/\", \"_Xưởng/\", \"_Templates/\", \"_Đề xuất/\", \"Trang chủ.md\"]")
    );
    let dig = f.info.get("schema_digest").unwrap();
    assert!(dig.starts_with("bbaa9f0197ef+e11:"), "{dig}");
    // sổ normalize, sổ migrate (không có: `-`), manifest, hàng chờ (không có: `-`), tập rộng; nối bằng `-`.
    let parts: Vec<&str> = dig["bbaa9f0197ef+e11:".len()..].split("---").collect();
    assert_eq!(parts.len(), 3, "{dig}");
    assert!(
        parts
            .iter()
            .all(|p| p.split('.').all(|n| n.parse::<u64>().is_ok())),
        "{dig}"
    );
    assert!(f.vault.exists());
}

#[test]
fn digest_and_pair_file() {
    let tmp = tempfile::tempdir().unwrap();
    let (vault, db) = (tmp.path().join("v"), tmp.path().join("g.db"));
    make_vault(&vault);
    let a = load("e11").unwrap().unwrap();
    build_with(&vault, &db, a.as_ref());
    let i2 = build_with(&vault, &db, a.as_ref());
    assert_eq!(
        (i2.get("mode"), i2.get("notes_read")),
        (Some("incremental"), Some("0"))
    );
    assert!(i2.get("edges").unwrap().contains("\"proposed\": 4"));
    let mut p = pairs();
    p.as_array_mut()
        .unwrap()
        .push(json!({"a": "Dự án/Alpha/a2.md", "b": "Dự án/Beta/b1.md", "score": 0.93}));
    // Cỡ file cặp đổi nên digest đổi (không cần chờ mtime theo giây).
    write(&vault, ".e11/similar-pairs.json", &p.to_string());
    let i3 = build_with(&vault, &db, load("e11").unwrap().unwrap().as_ref());
    assert!(
        i3.get("edges").unwrap().contains("\"proposed\": 5"),
        "{:?}",
        i3.get("edges")
    );
}

#[test]
fn similar_file_replaces_both_pair_files() {
    let tmp = tempfile::tempdir().unwrap();
    let (vault, db) = (tmp.path().join("v"), tmp.path().join("g.db"));
    make_vault(&vault);
    let other = tmp.path().join("pairs.json");
    std::fs::write(
        &other,
        json!([{"a": "Người/Lan.md", "b": "Đọc/t1.md", "score": 1}]).to_string(),
    )
    .unwrap();
    let a = E11::new(Some(other.clone()));
    let info = build_with(&vault, &db, &a);
    assert_eq!(info.get("adapter_spec"), Some(""));
    assert!(
        info.get("schema_digest")
            .unwrap()
            .ends_with(&format!("|{}", other.display()))
    );
    let o = edges(&index::connect(&db, true).unwrap(), &[("origin", "ohs")]);
    // Số nguyên ghi vào cột REAL thành số thực; nguồn là đường dẫn tương đối với vault.
    assert_eq!(o.len(), 1);
    assert_eq!(
        (&o[0]["src"], &o[0]["score"], &o[0]["source"]),
        (&json!("Người/Lan.md"), &json!(1.0), &json!("../pairs.json"))
    );
}

#[test]
fn broken_private_data_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    let (vault, db) = (tmp.path().join("v"), tmp.path().join("g.db"));
    make_vault(&vault);
    write(&vault, ".e11/similar-pairs.json", "[{\"a\": 1}]");
    let b = Build {
        packs: vec!["capture".into()],
        opts: opts(),
        adapter: Some(&E11::new(None)),
        ..Build::default()
    };
    let e = index::build(&vault, &db, &b).unwrap_err();
    assert!(e.to_string().contains("cặp thiếu a"), "{e}");
    write(&vault, ".e11/similar-pairs.json", "[]");
    write(&vault, "_E11/normalize-log.jsonl", "\n");
    assert!(index::build(&vault, &db, &b).is_err());
    // Dòng YAML lỗi trong sổ thì bỏ qua như Python.
    write(
        &vault,
        "_E11/normalize-log.jsonl",
        "{\"path\": \"x.md\", \"old_frontmatter\": \"a: [\", \"new_frontmatter\": \"b: 1\"}\n",
    );
    index::build(&vault, &db, &b).unwrap();
    // `data.score` không đọc được thành số: Python dừng build.
    write(
        &vault,
        &format!("{P}P1 liên quan.md"),
        &prop(
            "related",
            "chờ",
            &["Dự án/Alpha/a1", "Dự án/Beta/b1"],
            "model",
            Some(json!({"score": "cao"})),
            "",
        ),
    );
    let e = index::build(&vault, &db, &b).unwrap_err();
    assert!(
        e.to_string()
            .contains("could not convert string to float: 'cao'"),
        "{e}"
    );
}

#[test]
fn load_and_system_pages() {
    assert!(load("").unwrap().is_none());
    assert!(load("none").unwrap().is_none());
    let a = load("e11").unwrap().unwrap();
    assert_eq!((a.name(), a.spec()), (Some("e11"), "e11"));
    assert!(a.instructions().unwrap().contains("`_Đề xuất/`"));
    assert_eq!(a.hidden().len(), 5);
    let e = load("vault/kg_adapter.py").err().unwrap();
    assert!(
        e.to_string()
            .contains("không có adapter 'vault/kg_adapter.py'"),
        "{e}"
    );
    assert!(is_system("_E11/x.md") && is_system("Trang chủ.md") && is_system("_Đề xuất/P1.md"));
    assert!(!is_system("Trang chủ 2.md") && !is_system("Dự án/_E11/x.md"));
}
