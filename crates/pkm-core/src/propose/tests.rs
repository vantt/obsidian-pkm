use std::fs;

use serde_json::{Value, json};

use super::dump::{Node, dump};
use super::*;

/// Đầu ra của `yaml.safe_dump({'k': x}, sort_keys=False, allow_unicode=True, width=10_000)` (PyYAML 6.0.3) cho
/// các chuỗi và số khó, và một dict lồng.
const PYYAML: &str = include_str!("pyyaml-dump-cases.json");

fn one(k: Node) -> String {
    dump(&[("k".to_owned(), k)])
}

#[test]
fn strings_are_quoted_like_pyyaml() {
    let cases: Value = serde_json::from_str(PYYAML).unwrap();
    let strings = cases["strings"].as_object().unwrap();
    assert!(strings.len() > 60);
    for (s, want) in strings {
        assert_eq!(one(Node::Str(s.clone())), want.as_str().unwrap(), "{s:?}");
    }
}

#[test]
fn floats_follow_represent_float() {
    let cases: Value = serde_json::from_str(PYYAML).unwrap();
    for (repr, want) in cases["floats"].as_object().unwrap() {
        let x: f64 = repr.parse().unwrap();
        assert_eq!(one(Node::Float(x)), want.as_str().unwrap(), "{repr}");
    }
    assert_eq!(one(Node::Float(f64::NAN)), "k: .nan\n");
    assert_eq!(one(Node::Float(f64::NEG_INFINITY)), "k: -.inf\n");
}

#[test]
fn nested_containers_follow_block_style() {
    let cases: Value = serde_json::from_str(PYYAML).unwrap();
    let s = |v: &str| Node::Str(v.to_owned());
    let doc = vec![
        ("type".to_owned(), s("proposal")),
        ("notes".to_owned(), Node::Seq(vec![s("[[a|a]]"), s("b")])),
        (
            "data".to_owned(),
            Node::Map(vec![
                ("score".to_owned(), Node::Null),
                (
                    "x".to_owned(),
                    Node::Seq(vec![Node::Int("1".into()), s("y")]),
                ),
                ("e".to_owned(), Node::Seq(vec![])),
                ("m".to_owned(), Node::Map(vec![])),
            ]),
        ),
        (
            "l".to_owned(),
            Node::Seq(vec![
                Node::Seq(vec![Node::Int("1".into()), Node::Int("2".into())]),
                Node::Map(vec![
                    ("a".to_owned(), Node::Int("1".into())),
                    ("b".to_owned(), Node::Seq(vec![Node::Int("3".into())])),
                ]),
            ]),
        ),
    ];
    assert_eq!(dump(&doc), cases["nested"].as_str().unwrap());
    assert_eq!(dump(&[]), "{}\n");
}

#[test]
fn json_scores_map_to_yaml_values() {
    assert_eq!(one(Node::from_json(&json!(0.8123))), "k: 0.8123\n");
    assert_eq!(one(Node::from_json(&json!(1))), "k: 1\n");
    assert_eq!(one(Node::from_json(&Value::Null)), "k: null\n");
    assert_eq!(one(Node::from_json(&json!("0.9"))), "k: '0.9'\n");
    assert_eq!(one(Node::from_json(&json!(true))), "k: true\n");
}

#[test]
fn norm_drops_accents_case_digits_and_punctuation() {
    assert_eq!(norm("Hội thảo (chủ đề).md"), "hoi thao chu đe md");
    assert_eq!(norm("hoi-thao_chieu 2"), "hoi thao chieu");
    assert_eq!(norm("123 !!"), "");
    // Chữ số không thập phân là chữ với `\w` của Python.
    assert_eq!(norm("x²"), "x²");
}

#[test]
fn embeds_are_not_links_but_a_third_bracket_is() {
    assert_eq!(
        wikilink_targets("a ![[x]] [[y|z]] ![[[w]]] [[v#h]] [[]]"),
        ["y", "w", "v"]
    );
}

#[test]
fn char_offsets_stay_on_boundaries() {
    let t = "ấy là Trần Văn An";
    let at = t.find("Trần").unwrap();
    assert_eq!(&t[back(t, at, 3)..at], "là ");
    assert_eq!(&t[back(t, at, 99)..at], "ấy là ");
    assert_eq!(&t[at..forward(t, at, 4)], "Trần");
    assert_eq!(forward(t, at, 99), t.len());
}

#[test]
fn similar_file_must_be_a_list_of_string_pairs() {
    let pairs =
        parse_similar(r#"[{"a": "x.md", "b": "y.md", "score": 0.5}, {"a": "x.md", "b": "z.md"}]"#)
            .unwrap();
    assert_eq!(pairs[0].score, json!(0.5));
    assert_eq!(pairs[1].score, Value::Null);
    assert!(parse_similar("{}").is_err());
    assert!(
        parse_similar("[{\"a\": \"x.md\"}]")
            .unwrap_err()
            .contains("`b`")
    );
    assert!(parse_similar("[{\"a\": 1, \"b\": \"y.md\"}]").is_err());
    assert!(parse_similar("không phải json").is_err());
}

fn vault_with(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (rel, text) in files {
        let p = dir.path().join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
    dir
}

#[test]
fn broken_or_non_dict_frontmatter_counts_as_none() {
    let v = vault_with(&[
        (
            "a.md",
            "---\ntype: [toc\n---\n==một đoạn tô đủ dài== ở đây\n",
        ),
        ("b.md", "---\n- toc\n---\n==một đoạn tô khác đủ dài==\n"),
    ]);
    let all = proposals(v.path(), &[]).unwrap();
    let ideas: Vec<_> = all.iter().filter(|p| p.kind == Kind::Idea).collect();
    assert_eq!(ideas.len(), 2);
}

fn idea_note(i: usize) -> (String, String) {
    (
        format!("n{i:02}.md"),
        format!("==đoạn tô thứ {i:02} đủ dài==\n"),
    )
}

#[test]
fn queue_removes_known_proposals_and_caps_open_ones() {
    let files: Vec<(String, String)> = (0..25).map(idea_note).collect();
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let v = vault_with(&refs);
    let first = plan(v.path(), &[]).unwrap();
    assert_eq!(first.total, 25);
    assert_eq!(first.new.len(), 20);
    let written = write(v.path(), &first.new, "2026-11-04").unwrap();
    assert_eq!(written.len(), 20);
    assert_eq!(written[0], "_Đề xuất/Rút ý - n00.md");

    // Hàng chờ đã đủ 20 `idea` đang chờ: không thêm gì.
    let again = plan(v.path(), &[]).unwrap();
    assert_eq!((again.total, again.new.len()), (25, 0));

    // Đóng ba đề xuất: còn chỗ cho ba đề xuất chưa có, đề xuất đã đóng không quay lại.
    for f in &written[..3] {
        let p = v.path().join(f);
        let text = fs::read_to_string(&p).unwrap();
        fs::write(&p, text.replace("status: chờ", "status: đã bỏ")).unwrap();
    }
    let after = plan(v.path(), &[]).unwrap();
    let notes: Vec<&str> = after.new.iter().map(|p| p.notes[0].as_str()).collect();
    assert_eq!(notes, ["n20.md", "n21.md", "n22.md"]);
}

#[test]
fn queue_ignores_folders_other_files_and_odd_proposals() {
    let v = vault_with(&[
        ("a.md", "==một đoạn tô đủ dài==\n"),
        (
            "_Đề xuất/khác.md",
            "---\ntype: note\nkind: idea\nstatus: chờ\n---\n",
        ),
        ("_Đề xuất/lỗi.md", "---\ntype: proposal\nkind: [\n---\n"),
        (
            "_Đề xuất/thiếu notes.md",
            "---\ntype: proposal\nkind: idea\nstatus: chờ\n---\n",
        ),
        ("_Đề xuất/ảnh.png", "\u{89}PNG"),
    ]);
    fs::create_dir_all(v.path().join("_Đề xuất/con")).unwrap();
    let q = queue(v.path()).unwrap();
    assert!(q.keys.is_empty());
    assert_eq!(q.open.get("idea"), Some(&1));
    let p = plan(v.path(), &[]).unwrap();
    assert_eq!(p.new.len(), 1);
}

#[test]
fn writing_never_overwrites_and_numbers_clashing_names() {
    let v = vault_with(&[
        ("x/Ghi.md", "==đoạn tô ở thư mục x==\n"),
        ("y/Ghi.md", "==đoạn tô ở thư mục y==\n"),
        ("_Đề xuất/Rút ý - Ghi.md", "của người dùng\n"),
    ]);
    let p = plan(v.path(), &[]).unwrap();
    let written = write(v.path(), &p.new, "2026-11-04").unwrap();
    assert_eq!(
        written,
        ["_Đề xuất/Rút ý - Ghi 2.md", "_Đề xuất/Rút ý - Ghi 3.md"]
    );
    assert_eq!(
        fs::read_to_string(v.path().join("_Đề xuất/Rút ý - Ghi.md")).unwrap(),
        "của người dùng\n"
    );
    let note = fs::read_to_string(v.path().join(&written[0])).unwrap();
    assert!(
        note.starts_with("---\ntype: proposal\nkind: idea\nstatus: chờ\ncreated: '2026-11-04'\n")
    );
}

#[test]
fn file_names_drop_forbidden_characters_and_stay_short() {
    let mut p = Proposal::new(
        Kind::Person,
        vec!["a.md".into()],
        String::new(),
        String::new(),
    );
    p.value = Some(format!("A/B:C*D?{}", "x".repeat(200)));
    let stem = file_stem(&p);
    assert!(stem.starts_with("Người - A B C D "));
    assert_eq!(stem.chars().count(), 90);
    let p = Proposal::new(
        Kind::Idea,
        vec!["d/[x]# .md".into()],
        String::new(),
        String::new(),
    );
    assert_eq!(file_stem(&p), "Rút ý -  x");
}

#[test]
fn missing_vault_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(proposals(&dir.path().join("không có"), &[]).is_err());
}
