//! Test của `zones`: các test của `scripts/e11/tests/test_zones.py`, cộng bộ xáo giống Python, cạnh người viết từ
//! index, cả vòng trên vault giả kiểu bản B, và trang không đè trang đã có.

use std::collections::{BTreeSet, HashMap, HashSet};

use indexmap::IndexMap;
use serde_json::json;

use super::*;
use crate::index::{self, Build, EdgeKeys};

fn clique(names: &[String], x: f64) -> Weights {
    let mut w = Weights::new();
    for (i, a) in names.iter().enumerate() {
        for b in &names[i + 1..] {
            w.insert(edge(a, b), x);
        }
    }
    w
}

fn names(prefix: &str, n: usize) -> Vec<String> {
    (0..n).map(|i| format!("{prefix}{i}.md")).collect()
}

fn set(v: &[String]) -> BTreeSet<String> {
    v.iter().cloned().collect()
}

#[test]
fn shuffle_matches_python_random_7() {
    // `r = random.Random(7); r.shuffle(list(range(20))); r.shuffle(list(range(5)))`.
    let mut r = PyRandom::new(7);
    let mut x: Vec<usize> = (0..20).collect();
    r.shuffle(&mut x);
    assert_eq!(
        x,
        [
            17, 15, 11, 18, 7, 6, 19, 3, 14, 0, 9, 5, 16, 8, 13, 2, 1, 12, 4, 10
        ]
    );
    let mut y: Vec<usize> = (0..5).collect();
    r.shuffle(&mut y);
    assert_eq!(y, [1, 2, 4, 3, 0]);
    let mut r = PyRandom::new(7);
    assert_eq!(
        [r.next_u32(), r.next_u32(), r.next_u32()],
        [1_390_851_128, 4_071_050_724, 647_892_279]
    );
}

#[test]
fn louvain_splits_two_cliques_joined_by_a_weak_edge_and_is_stable() {
    let (a, b) = (names("A/a", 6), names("B/b", 6));
    let mut w = clique(&a, 1.0);
    w.extend(clique(&b, 1.0));
    w.insert(edge(&a[0], &b[0]), 0.1);
    let nodes: BTreeSet<String> = a.iter().chain(&b).cloned().collect();
    let com = louvain(&nodes, &w, 1.0);
    assert_eq!(a.iter().map(|n| com[n]).collect::<HashSet<_>>().len(), 1);
    assert_eq!(b.iter().map(|n| com[n]).collect::<HashSet<_>>().len(), 1);
    assert_ne!(com[&a[0]], com[&b[0]]);
    assert_eq!(com, louvain(&nodes, &w, 1.0));
}

#[test]
fn big_communities_are_split_and_small_ones_dropped() {
    let (a, b) = (names("A/a", 6), names("B/b", 6));
    let c = names("C/c", 2);
    let mut w = clique(&a, 1.0);
    w.extend(clique(&b, 1.0));
    w.insert(edge(&a[0], &b[0]), 0.9);
    w.insert(edge(&a[1], &b[1]), 0.9);
    w.insert(edge(&c[0], &c[1]), 1.0);
    let nodes: BTreeSet<String> = a.iter().chain(&b).chain(&c).cloned().collect();
    let mut zs = zones_of(&nodes, &w, 8, 3);
    zs.sort();
    assert_eq!(zs, vec![set(&a), set(&b)]);
}

#[test]
fn score_label_colours_folders_and_posts() {
    let a = names("A/a", 6);
    let zone: BTreeSet<String> = a[..3]
        .iter()
        .cloned()
        .chain(["B/x.md".to_owned(), "Bài viết/p.md".to_owned()])
        .collect();
    let w = clique(&zone.iter().cloned().collect::<Vec<_>>(), 1.0);
    let mut info: HashMap<String, NoteInfo> = zone
        .iter()
        .map(|n| {
            let c = n.chars().rev().nth(4).unwrap();
            let i = NoteInfo {
                title: format!("Wardley map @tac_gia {c}"),
                created: "2020-01-01".into(),
                ..NoteInfo::default()
            };
            (n.clone(), i)
        })
        .collect();
    let colour = |c: Option<&str>| c.map(str::to_owned);
    info.get_mut(&a[0]).unwrap().hl = vec![
        ("đoạn một".into(), colour(Some("green"))),
        ("đoạn hai".into(), colour(Some("yellow"))),
    ];
    info.get_mut(&a[1]).unwrap().hl = vec![("đoạn ba".into(), None)];
    info.get_mut("Bài viết/p.md").unwrap().post = true;
    let df: HashMap<String, u64> = [("wardley", 5), ("map", 5), ("tac_gia", 5)]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
    let skip: HashSet<String> = ["tac_gia".to_owned()].into();
    let z = describe(&zone, &w, &info, &df, 100, &skip);
    assert_eq!(
        (
            &z["notes"],
            &z["with_highlights"],
            &z["highlights"],
            &z["highlight_weight"]
        ),
        (&json!(5), &json!(2), &json!(3), &json!(5))
    );
    assert_eq!(
        z["colours"],
        json!({"làm được": 1, "thú vị": 1, "chưa màu": 1})
    );
    assert_eq!(
        (&z["folders"], &z["posts"]),
        (&json!(3), &json!(["Bài viết/p.md"]))
    );
    assert_eq!(
        z["picks"][0],
        json!({"text": "đoạn một", "colour": "làm được"})
    );
    let label = z["label"].as_str().unwrap();
    assert!(!label.contains("tac_gia"), "{label}");
    assert!(label.contains("wardley"), "{label}");
    // Như Python: 2*2 + 5/10 + 0 + 3 + 5/5.
    assert_eq!(z["score"], json!(8.5));
    assert_eq!(z["years"], json!(["2020", "2020"]));
    assert_eq!(z["top_folders"], json!(["A", "B", "Bài viết"]));
}

#[test]
fn unknown_highlight_colour_keeps_its_name() {
    assert_eq!(meaning(Some("pink")), "pink");
    assert_eq!(weight(Some("pink")), 1);
    let caps: Vec<(String, Option<String>)> = EQ
        .captures_iter("a ==x== #hl/green b ===y=== ==z== #hl/pink")
        .map(|c| (c[1].to_owned(), c.get(2).map(|m| m.as_str().to_owned())))
        .collect();
    assert_eq!(
        caps,
        [
            ("x".to_owned(), Some("green".to_owned())),
            // Như Python: `===y===` khớp từ dấu `=` thứ hai.
            ("y".to_owned(), None),
            ("z".to_owned(), Some("pink".to_owned()))
        ]
    );
}

#[test]
fn words_drop_stopwords_numbers_single_letters_and_skipped() {
    let skip: HashSet<String> = ["an".to_owned()].into();
    assert_eq!(
        words("Cách làm Wardley Map 2024 x của An", &skip),
        ["cách", "wardley", "map"]
    );
}

/// Vault kiểu bản B: hai cụm sáu note (cụm A có đoạn tô, ý của mình), một trang chủ đề và trang hệ thống (không vào
/// vùng), link thân note là cạnh người viết.
fn e11_vault() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let v = tmp.path();
    let put = |p: &str, t: &str| crate::index::testutil::write(v, p, t);
    for i in 0..6 {
        put(
            &format!("Ý/Vườn {i}.md"),
            &format!(
                "---\ntype: idea\ncreated: 202{i}-01-01\n---\nChăm vườn ==tưới sớm {i}== #hl/green\n"
            ),
        );
        put(
            &format!("Bếp/Nấu {i}.md"),
            &format!(
                "---\ntype: clip\nauthor: Đầu Bếp\n---\nNấu ăn {i} [[Nấu {}]]\n",
                (i + 1) % 6
            ),
        );
    }
    put("Ý/Ý (chủ đề).md", "---\ntype: toc\n---\n");
    put("_Xưởng/Trang.md", "---\ntype: toc\n---\n");
    let db = v.join(".kg").join("graph.db");
    let adapter = crate::adapter::load("e11").unwrap().unwrap();
    index::build(
        v,
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
    (tmp, db)
}

fn knn_of(groups: &[Vec<String>]) -> IndexMap<String, Vec<(String, f64)>> {
    let mut knn = IndexMap::new();
    for g in groups {
        for a in g {
            let near: Vec<(String, f64)> = g
                .iter()
                .filter(|b| *b != a)
                .map(|b| (b.clone(), 0.9))
                .collect();
            knn.insert(a.clone(), near);
        }
    }
    knn
}

#[test]
fn zones_on_an_e11_vault_score_highlights_and_skip_generated_pages() {
    let (tmp, db) = e11_vault();
    let g = Graph::open(&db).unwrap();
    let vuon: Vec<String> = (0..6).map(|i| format!("Ý/Vườn {i}.md")).collect();
    let bep: Vec<String> = (0..6).map(|i| format!("Bếp/Nấu {i}.md")).collect();
    let mut groups = vec![vuon.clone(), bep.clone()];
    // Trang chủ đề, trang hệ thống gần nghĩa với cụm Ý vẫn bị loại.
    groups[0].push("Ý/Ý (chủ đề).md".into());
    groups[0].push("_Xưởng/Trang.md".into());
    let z = zones(tmp.path(), &g, &knn_of(&groups), &Options::default()).unwrap();
    assert_eq!(z.zones.len(), 2, "{:#?}", z.zones);
    let first = &z.zones[0];
    let mut all: Vec<String> = first["all"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect();
    all.sort();
    assert_eq!(all, vuon);
    assert_eq!(first["with_highlights"], 6);
    assert_eq!(first["own_ideas"], 6);
    assert_eq!(first["colours"], json!({"làm được": 6}));
    assert_eq!(first["years"], json!(["2020", "2025"]));
    assert_eq!(z.zones[1]["label"], "nấu");
    assert_eq!(z.stats["hand_edges"], 6);
    assert_eq!(z.stats["zones"], 2);
    assert_eq!(z.stats["covered"], 12);
    // Tên tác giả không thành tên vùng.
    assert!(!z.zones[1]["label"].as_str().unwrap().contains("bếp"));
}

#[test]
fn page_lists_zones_and_is_never_written_over_an_existing_page() {
    let z = json!({"label": "vườn · tưới", "score": 21.0, "notes": 6, "core": ["Ý/Vườn 0.md"], "all": ["Ý/Vườn 0.md"],
                   "with_highlights": 1, "highlights": 2, "highlight_weight": 4,
                   "colours": {"làm được": 1, "thú vị": 1}, "own_ideas": 1, "folders": 1, "top_folders": ["Ý"],
                   "posts": ["Bài viết/Cũ.md"], "years": ["2020", "2021"],
                   "picks": [{"text": "tưới sớm", "colour": "làm được"}, {"text": "đất tơi", "colour": "thú vị"}]});
    let text = page(&[z.clone(), z], "Vault B", "2026-11-04", 1);
    assert!(text.starts_with("---\ngenerated: 2026-11-04\n"), "{text}");
    assert!(text.contains("## 1. vườn · tưới\n\n6 note, 1 thư mục, 1 note có tô (2 đoạn: làm được 1, thú vị 1), 1 ý của mình, năm 2020–2021. Điểm 21.0. **Đã có bài:** [[Bài viết/Cũ|Cũ]]\n"), "{text}");
    assert!(text.contains("\n> tưới sớm *(làm được)*\n>\n> đất tơi *(thú vị)*\n\n[Viết bài từ vùng này](obsidian://new?vault=Vault%20B&file=B%C3%A0i%20vi%E1%BA%BFt%2Fv%C6%B0%E1%BB%9Dn%20t%C6%B0%E1%BB%9Bi&content=---%0Atype"), "{text}");
    assert!(!text.contains("## 2."));
    assert!(
        text.ends_with("Còn 1 vùng điểm thấp hơn trong `.e11/zones.json`.\n"),
        "{text}"
    );

    let tmp = tempfile::tempdir().unwrap();
    assert!(write_page_if_absent(tmp.path(), "mới").unwrap());
    let p = tmp.path().join(PAGE);
    std::fs::write(&p, "dòng chấm của người dùng").unwrap();
    assert!(!write_page_if_absent(tmp.path(), "đè").unwrap());
    assert_eq!(
        std::fs::read_to_string(&p).unwrap(),
        "dòng chấm của người dùng"
    );
}
