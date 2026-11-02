//! Test của `index`: phần build của `scripts/kg/tests/test_kg.py` ở sha ghim (lớp `Api` đọc cạnh, `Incremental`),
//! cộng các hành vi của `index.py` mà test Python không kiểm riêng. Giá trị mong đợi đã đối chiếu với `index.py`.

use rusqlite::Connection;
use serde_json::{Value, json};

use super::testutil::{NOTES, TestVault, clean_notes, write};
use super::*;

/// Các cột của cạnh claim `claim`.
fn claim_edge(db: &Connection, claim: &str) -> Value {
    db.query_row(
        "SELECT src, rel, dst, dst_text, status, valid_from, valid_to, source, origin, proposed_by, extracted_by, \
         confidence, score FROM edges WHERE claim = ?",
        [claim],
        |r| {
            Ok(json!({
                "src": r.get::<_, String>(0)?, "rel": r.get::<_, String>(1)?, "dst": r.get::<_, Option<String>>(2)?,
                "dst_text": r.get::<_, Option<String>>(3)?, "status": r.get::<_, Option<String>>(4)?,
                "valid_from": r.get::<_, Option<String>>(5)?, "valid_to": r.get::<_, Option<String>>(6)?,
                "source": r.get::<_, Option<String>>(7)?, "origin": r.get::<_, String>(8)?,
                "proposed_by": r.get::<_, Option<String>>(9)?, "extracted_by": r.get::<_, Option<String>>(10)?,
                "confidence": r.get::<_, Option<String>>(11)?, "score": r.get::<_, Option<f64>>(12)?,
            }))
        },
    )
    .expect("có cạnh của claim")
}

fn count(db: &Connection, sql: &str) -> i64 {
    db.query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn claim_edge_window_source_and_alias() {
    let v = TestVault::new(&clean_notes());
    v.build(&["business"]);
    let db = v.open();
    let e = claim_edge(&db, "Claim/Lead tăng doanh thu.md");
    assert_eq!(e["source"], "Họp/Họp 2026-01-05.md#^q1");
    assert_eq!(
        (
            &e["src"],
            &e["rel"],
            &e["dst"],
            &e["dst_text"],
            &e["origin"]
        ),
        (
            &json!("Yếu tố/Số lead.md"),
            &json!("increases"),
            &json!("Yếu tố/Doanh thu.md"),
            &json!("Doanh thu"),
            &json!("claim")
        )
    );
    // đích ghi bằng alias
    let a = claim_edge(&db, "Claim/Doanh thu tăng ngân sách.md");
    assert_eq!(a["dst"], "Yếu tố/Ngân sách.md");
    let old = claim_edge(&db, "Claim/An làm ở ABC.md");
    assert_eq!(
        (&old["status"], &old["valid_from"], &old["valid_to"]),
        (
            &json!("superseded"),
            &json!("2024-01-01"),
            &json!("2025-06-30")
        )
    );
}

#[test]
fn two_layers_and_provenance() {
    let v = TestVault::new(&clean_notes());
    v.build(&["business"]);
    let db = v.open();
    let p = claim_edge(&db, "Claim/Mệt giảm doanh thu.md");
    assert_eq!(
        (
            &p["status"],
            &p["proposed_by"],
            &p["extracted_by"],
            &p["score"],
            &p["confidence"]
        ),
        (
            &json!("proposed"),
            &json!("llm"),
            &json!("claude-sonnet@schema_v3"),
            &json!(0.62),
            &json!("plausible")
        )
    );
    let acc = claim_edge(&db, "Claim/Lead tăng doanh thu.md");
    assert_eq!(
        (&acc["status"], &acc["proposed_by"]),
        (&json!("accepted"), &json!("human"))
    );
    // Hai đầu của claim không là cạnh frontmatter riêng; cạnh frontmatter mang cửa sổ của note nguồn.
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM edges WHERE src LIKE 'Claim/%' AND rel IN ('cause', 'effect')"
        ),
        0
    );
    let (vf, vt, st): (Option<String>, Option<String>, String) = db
        .query_row(
            "SELECT valid_from, valid_to, status FROM edges WHERE src = 'Hợp đồng/HĐ ABC 2025.md' AND rel = 'covers'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        (vf.as_deref(), vt.as_deref(), st.as_str()),
        (Some("2025-01-01"), Some("2025-12-31"), "accepted")
    );
    // Khoá không là cạnh của schema (`date`, `aliases`) không thành cạnh khi edge_keys = schema.
    assert_eq!(
        count(&db, "SELECT count(*) FROM edges WHERE origin = 'body'"),
        0
    );
}

#[test]
fn rebuild_after_edit_and_delete() {
    let v = TestVault::new(&clean_notes());
    assert_eq!(v.build(&["business"]).get("mode"), Some("full"));
    assert_eq!(v.build(&["business"]).get("notes_read"), Some("0"));
    let p = v.path().join("Claim").join("Mệt giảm doanh thu.md");
    let txt = std::fs::read_to_string(&p)
        .unwrap()
        .replace("status: proposed", "status: accepted");
    std::fs::write(&p, txt).unwrap();
    // mtime có thể không đổi trong cùng một tích đồng hồ; cỡ file đổi nên note vẫn được đọc lại.
    std::fs::remove_file(v.path().join("Claim").join("Ngân sách tăng lead.md")).unwrap();
    let info = v.build(&["business"]);
    assert_eq!(
        (
            info.get("mode"),
            info.get("notes_read"),
            info.get("notes_removed")
        ),
        (Some("incremental"), Some("1"), Some("1"))
    );
    let db = v.open();
    assert_eq!(
        claim_edge(&db, "Claim/Mệt giảm doanh thu.md")["status"],
        "accepted"
    );
    // Vòng Doanh thu -> Ngân sách -> Số lead -> Doanh thu mất cạnh Ngân sách -> Số lead.
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM edges WHERE src = 'Yếu tố/Ngân sách.md' AND dst = 'Yếu tố/Số lead.md'"
        ),
        0
    );
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM nodes WHERE id = 'Claim/Ngân sách tăng lead.md'"
        ),
        0
    );
}

/// Build tăng dần không đổi gì cho đúng các bảng của build đủ (khác ở `meta.mode`, `notes_read`).
#[test]
fn noop_build_keeps_every_table() {
    let v = TestVault::new(NOTES);
    v.build(&["business"]);
    let full = dump_tables(&v.open()).unwrap();
    let info = v.build(&["business"]);
    assert_eq!(
        (info.get("mode"), info.get("notes_read")),
        (Some("incremental"), Some("0"))
    );
    let noop = dump_tables(&v.open()).unwrap();
    assert_eq!(full.len(), noop.len());
    for (a, b) in full.iter().zip(&noop) {
        if a.name != "meta" {
            assert_eq!(a, b, "{}", a.name);
        }
    }
    let names: Vec<&str> = full.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names.last(), Some(&"v3_deviations"));
    assert!(names.contains(&"edges") && !names.iter().any(|n| n.starts_with("sqlite_")));
}

#[test]
fn meta_like_python() {
    let v = TestVault::new(&clean_notes());
    let info = v.build(&["business"]);
    let keys: Vec<&str> = info.entries().iter().map(|(k, _)| *k).collect();
    assert_eq!(
        keys,
        [
            "vault",
            "packs",
            "options",
            "schema_version",
            "schema_digest",
            "built_at",
            "mode",
            "notes",
            "notes_read",
            "notes_removed",
            "adapter",
            "adapter_spec",
            "hidden",
            "ms",
            "edges",
            "schema_problems"
        ]
    );
    let g = |k| info.get(k).unwrap();
    assert_eq!(
        std::path::Path::new(g("vault")),
        std::path::absolute(v.path()).unwrap()
    );
    assert_eq!(g("packs"), "business");
    assert_eq!(g("options"), "aliases=1,edge_keys=schema,body_edges=0");
    assert_eq!(g("schema_version"), "3");
    assert_eq!(g("schema_digest"), "bbaa9f0197ef");
    assert_eq!(g("notes"), "20");
    assert_eq!(
        (g("adapter"), g("adapter_spec"), g("hidden")),
        ("", "", "[]")
    );
    assert_eq!(g("schema_problems"), "");
    assert_eq!(g("built_at").len(), "2026-11-02T14:40:00".len());
    let ms: Value = serde_json::from_str(g("ms")).unwrap();
    assert_eq!(
        ms.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["list", "parse", "edges_schema_views", "total"]
    );
    // Thứ tự gặp lần đầu, như dict Python.
    let edges = g("edges");
    assert!(edges.starts_with("{\"accepted\": "), "{edges}");
    let counts: Value = serde_json::from_str(edges).unwrap();
    let db = v.open();
    assert_eq!(
        counts
            .as_object()
            .unwrap()
            .values()
            .map(|x| x.as_i64().unwrap())
            .sum::<i64>(),
        count(&db, "SELECT count(*) FROM edges")
    );
    let stored: Vec<(String, String)> = db
        .prepare("SELECT key, value FROM meta")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(stored.len(), 16);
    let s = info.summary("db/graph.db");
    assert!(s.starts_with("db/graph.db (full): 20 note, đọc 20, xoá 0; ms {\"list\": "));
    assert!(
        s.ends_with("}\ncạnh theo lớp: {'accepted': 19, 'proposed': 2, 'superseded': 1}\n"),
        "{s}"
    );
}

#[test]
fn counts_escape_non_ascii_status_like_json_dumps() {
    let mut notes = clean_notes();
    notes.push((
        "Claim/Chờ.md",
        "type: claim\nrelation: increases\ncause: \"[[Số lead]]\"\neffect: \"[[Doanh thu]]\"\nstatus: chờ",
    ));
    notes.push((
        "Claim/Không status.md",
        "type: claim\nrelation: increases\ncause: \"[[Mệt mỏi]]\"\neffect: \"[[Doanh thu]]\"",
    ));
    let v = TestVault::new(&notes);
    let info = v.build(&["business"]);
    assert!(info.get("edges").unwrap().contains("\"ch\\u1edd\": 1"));
    assert!(info.get("edges").unwrap().contains("\"(none)\": 1"));
    assert_eq!(info.counts()["chờ"], 1);
    // Build không đổi gì đọc lại số đếm từ `meta`.
    let again = v.build(&["business"]);
    assert_eq!(again.get("edges"), info.get("edges"));
    assert_eq!(again.counts(), info.counts());
}

#[test]
fn changing_packs_or_options_rebuilds_everything() {
    let v = TestVault::new(&clean_notes());
    v.build(&["business"]);
    let info = v.build(&[]);
    assert_eq!(
        (info.get("mode"), info.get("notes_read")),
        (Some("full"), Some("20"))
    );
    let opts = Options {
        body_edges: true,
        ..Options::default()
    };
    let info = v.build_with(&[], opts, false);
    assert_eq!(info.get("mode"), Some("full"));
    assert_eq!(
        info.get("options"),
        Some("aliases=1,edge_keys=schema,body_edges=1")
    );
    let info = v.build_with(&[], opts, true);
    assert_eq!(
        (info.get("mode"), info.get("notes_read")),
        (Some("full"), Some("20"))
    );
}

#[test]
fn body_edges_and_edge_keys_links() {
    let mut notes = clean_notes();
    notes.push((
        "Ghi chú/Đọc.md",
        "type: note\nliên_quan: \"[[Marketing]]\"\naliases: [Đọc thêm]",
    ));
    let v = TestVault::new(&notes);
    write(
        v.path(),
        "Ghi chú/Thân.md",
        "---\ntype: note\n---\n[[SEO]] [[Thân]] [[#Mục]] ![[ảnh.png]] [x](../Khái niệm/SEO.md)\n",
    );
    let opts = Options {
        aliases: false,
        edge_keys: EdgeKeys::Links,
        body_edges: true,
    };
    v.build_with(&[], opts, false);
    let db = v.open();
    let rows: Vec<(String, String, Option<String>, String)> = db
        .prepare("SELECT rel, dst_text, dst, origin FROM edges WHERE src = 'Ghi chú/Thân.md' ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    // Link tự trỏ, `[[#Mục]]`, tệp đính kèm bị bỏ; link markdown trùng đích thì bỏ như cạnh trùng.
    assert_eq!(
        rows,
        [(
            "link".to_owned(),
            "SEO".to_owned(),
            Some("Khái niệm/SEO.md".to_owned()),
            "body".to_owned()
        )]
    );
    // edge_keys = links: mọi khoá có wikilink thành cạnh (trừ aliases).
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM edges WHERE src = 'Ghi chú/Đọc.md' AND rel = 'liên_quan' AND origin = 'frontmatter'"
        ),
        1
    );
}

#[test]
fn options_pick_and_previous() {
    let v = TestVault::new(&clean_notes());
    let db = v.db();
    assert_eq!(previous(&db, "packs", "x"), "x");
    assert_eq!(
        Options::pick(false, None, false, Some(&db)),
        Options::default()
    );
    let o = Options {
        aliases: false,
        edge_keys: EdgeKeys::Links,
        body_edges: true,
    };
    v.build_with(&["capture"], o, false);
    assert_eq!(previous(&db, "packs", ""), "capture");
    assert_eq!(previous(&db, "options", ""), o.key());
    assert_eq!(previous(&db, "không có", "mặc định"), "mặc định");
    // Không cờ nào: lấy cách đọc của lần build trước.
    assert_eq!(Options::pick(false, None, false, Some(&db)), o);
    // Có cờ: chỉ theo cờ.
    assert_eq!(
        Options::pick(false, None, true, Some(&db)),
        Options {
            body_edges: true,
            ..Options::default()
        }
    );
    assert_eq!(
        Options::pick(true, Some(EdgeKeys::Schema), false, None),
        Options {
            aliases: false,
            ..Options::default()
        }
    );
    assert_eq!(EdgeKeys::parse("links"), Some(EdgeKeys::Links));
    assert_eq!(EdgeKeys::parse("khác"), None);
}

#[test]
fn ctx_add_and_drop_deduplicate_by_key() {
    let db = Connection::open_in_memory().unwrap();
    let r = Resolver::default();
    let (s, _) = schema::load(&Source::Embedded, &[]).unwrap();
    let mut a = EdgeRow::new("a.md", "related", "frontmatter");
    a.dst = Some("b.md".into());
    a.dst_text = Some("b".into());
    let mut ctx = Ctx::new(&db, Path::new("."), &r, &s, vec![a.clone()]);
    assert!(!ctx.add(a.clone()));
    // `dst or dst_text`: cạnh không giải được so theo chữ.
    let mut u = EdgeRow::new("a.md", "related", "frontmatter");
    u.dst_text = Some("b.md".into());
    assert!(!ctx.add(u.clone()));
    u.origin = "ohs".into();
    assert!(ctx.add(u.clone()));
    assert_eq!(ctx.drop_where(|e| e.origin == "frontmatter"), 1);
    // Bỏ rồi thì thêm lại được.
    assert!(ctx.add(a));
    assert_eq!(ctx.rows.len(), 2);
}

#[test]
fn regexp_follows_python_re() {
    for (p, s, want) in [
        ("^.+@schema_v[0-9]+$", "claude@schema_v3", true),
        ("^.+@schema_v[0-9]+$", "claude@schema_v3\n", true),
        ("^.+@schema_v[0-9]+$", "claude@schema_v3\n\n", false),
        ("^.+@schema_v[0-9]+$", "claude", false),
        (r"^\d{4}-\d{2}-\d{2}$", "2026-01-05", true),
        (r"a\Z", "a\n", false),
        (r"a\Z", "a", true),
        ("[[]x", "[x", true),
        ("[&&a]", "&", true),
        ("[^]]", "]", false),
        ("[]a]", "]", true),
        ("[$]", "$", true),
    ] {
        let re = py_regex(p).unwrap_or_else(|e| panic!("{p}: {e}"));
        assert_eq!(re.is_match(s), want, "{p:?} trên {s:?}");
    }
    let tmp = tempfile::tempdir().unwrap();
    let db = connect(&tmp.path().join("sub").join("x.db"), false).unwrap();
    let q = |sql: &str| db.query_row(sql, [], |r| r.get::<_, Option<bool>>(0));
    assert_eq!(
        q("SELECT regexp('^a$', 'a' || char(10))").unwrap(),
        Some(true)
    );
    assert_eq!(q("SELECT regexp('a', NULL)").unwrap(), Some(false));
    assert!(q("SELECT regexp('(?<=a)b', 'ab')").is_err());
}

#[test]
fn missing_vault_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    let e = build(
        &tmp.path().join("không có"),
        &tmp.path().join("g.db"),
        &Build::default(),
    )
    .unwrap_err();
    assert!(matches!(e, IndexError::Io { .. }), "{e}");
}

#[test]
fn empty_vault_builds_an_empty_index() {
    let tmp = tempfile::tempdir().unwrap();
    let vault = tmp.path().join("v");
    std::fs::create_dir_all(&vault).unwrap();
    let db = tmp.path().join("g.db");
    let info = build(&vault, &db, &Build::default()).unwrap();
    assert_eq!(
        (info.get("notes"), info.get("edges")),
        (Some("0"), Some("{}"))
    );
    // Không có note nào trong DB thì lần sau vẫn là `full` (như `not old` của Python).
    assert_eq!(
        build(&vault, &db, &Build::default()).unwrap().get("mode"),
        Some("full")
    );
}
