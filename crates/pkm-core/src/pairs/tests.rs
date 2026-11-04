//! Test của `pairs`: lọc note, chọn cặp và K láng giềng như `similar_pairs.mjs`, `note_knn.mjs`, trên vector giả.

use serde_json::{Value, json};

use super::*;

fn reply(records: Value) -> VectorReply {
    VectorReply {
        meta: serde_json::Map::new(),
        records: records.as_array().unwrap().clone(),
    }
}

/// Vector đơn vị từ góc (độ) trên mặt phẳng: cosine giữa hai note là cos của hiệu góc.
fn at(deg: f64) -> Value {
    let r = deg.to_radians();
    json!([r.cos(), r.sin()])
}

fn vectors(notes: &[(&str, f64)]) -> NoteVectors {
    let records: Vec<Value> = notes
        .iter()
        .map(|(p, d)| json!({"path": p, "dim": 2, "model": "m", "chunks": 1, "vector": at(*d)}))
        .collect();
    NoteVectors::from_reply(&reply(json!(records)), |_| true).unwrap()
}

#[test]
fn generated_pages_and_system_folders_are_skipped() {
    for p in [
        "_E11/x.md",
        "_Templates/Bài.md",
        "_Xưởng/Nguyên liệu/a.md",
        "_Đề xuất/p.md",
        "_ohs/lexicon.md",
        "Vườn/Vườn (chủ đề).md",
        "Trang chủ.md",
    ] {
        assert!(is_generated(p), "{p}");
    }
    for p in [
        "Vườn/Cây táo.md",
        "A/Trang chủ.md",
        "_E11x/a.md",
        "Ý/(chủ đề).md",
    ] {
        assert!(!is_generated(p), "{p}");
    }
}

#[test]
fn words_are_counted_in_the_body_only() {
    assert_eq!(
        body_words("---\ntype: note\ntags: [a, b]\n---\nmột hai  ba\nbốn\n"),
        4
    );
    assert_eq!(body_words("không có frontmatter"), 3);
    assert_eq!(body_words("---\nx: 1\n---\n"), 0);
}

#[test]
fn rounding_is_math_round_to_three_digits() {
    assert_eq!(js_round3(0.8765), 0.877);
    assert_eq!(js_round3(0.12345), 0.123);
    assert_eq!(js_round3(-0.0005), -0.0);
    assert_eq!(js_round3(1.0), 1.0);
}

#[test]
fn records_with_errors_and_filtered_notes_are_dropped_and_dims_must_agree() {
    let r = reply(json!([
        {"path": "a.md", "vector": [1.0, 0.0]},
        {"path": "b.md", "error": "note not found in the index"},
        {"path": "c.md", "vector": [0.0, 1.0]},
    ]));
    let v = NoteVectors::from_reply(&r, |p| p != "c.md").unwrap();
    assert_eq!(v.paths, ["a.md"]);
    assert_eq!(v.dim, 2);
    let bad =
        reply(json!([{"path": "a.md", "vector": [1.0]}, {"path": "c.md", "vector": [0.0, 1.0]}]));
    assert!(NoteVectors::from_reply(&bad, |_| true).is_err());
}

#[test]
fn pairs_cross_groups_best_first_with_per_note_and_total_limits() {
    let v = vectors(&[
        ("A/a.md", 0.0),
        ("B/b.md", 5.0),  // gần a: cos 5° ≈ 0.996
        ("C/c.md", 10.0), // gần b, kém hơn với a
        ("D/x/y/1.md", 20.0),
        ("D/x/y/2.md", 21.0), // cùng nhóm ba đoạn với 1: không bao giờ thành cặp
        ("E/e.md", 90.0),     // xa tất cả
    ]);
    let all = similar_pairs(
        &v,
        &PairOptions {
            min: 0.9,
            max: 60,
            per_note: 10,
            min_words: 0,
        },
    );
    let names: Vec<(String, String)> = all
        .picked
        .iter()
        .map(|p| {
            (
                p["a"].as_str().unwrap().into(),
                p["b"].as_str().unwrap().into(),
            )
        })
        .collect();
    assert!(!names.contains(&("D/x/y/1.md".into(), "D/x/y/2.md".into())));
    assert!(names.iter().all(|(a, b)| a != "E/e.md" && b != "E/e.md"));
    assert_eq!(names[0], ("A/a.md".into(), "B/b.md".into()));
    let scores: Vec<f64> = all
        .picked
        .iter()
        .map(|p| p["score"].as_f64().unwrap())
        .collect();
    assert!(scores.windows(2).all(|w| w[0] >= w[1]), "{scores:?}");
    assert_eq!(all.picked[0]["score"], json!(0.996));
    assert_eq!(all.notes, 6);
    assert_eq!(all.candidates, all.picked.len());

    let limited = similar_pairs(
        &v,
        &PairOptions {
            min: 0.9,
            max: 2,
            per_note: 1,
            min_words: 0,
        },
    );
    // a–b dùng hết lượt của a và b; cặp sau không chạm a, b.
    assert_eq!(limited.picked.len(), 2);
    assert_eq!(limited.picked[0]["a"], "A/a.md");
    let second = &limited.picked[1];
    for p in ["A/a.md", "B/b.md"] {
        assert_ne!(second["a"], p);
        assert_ne!(second["b"], p);
    }
}

#[test]
fn identical_notes_score_one_written_as_an_integer() {
    let v = vectors(&[("A/a.md", 30.0), ("B/a.md", 30.0)]);
    let p = similar_pairs(&v, &PairOptions::default());
    assert_eq!(
        p.picked,
        vec![json!({"a": "A/a.md", "b": "B/a.md", "score": 1})]
    );
    assert_eq!(serde_json::to_string(&p.picked[0]["score"]).unwrap(), "1");
}

#[test]
fn knn_keeps_k_best_and_ties_keep_input_order() {
    let v = vectors(&[
        ("a.md", 0.0),
        ("b.md", 10.0),
        ("c.md", -10.0), // hoà với b khi nhìn từ a
        ("d.md", 50.0),
    ]);
    let k = knn(&v, 2);
    assert_eq!(k.len(), 4);
    let a: Vec<&str> = k["a.md"].iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(a, ["b.md", "c.md"]);
    assert_eq!(k["a.md"][0].1, js_round3(10f64.to_radians().cos()));
    let d: Vec<&str> = k["d.md"].iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(d, ["b.md", "a.md"]);
    // K lớn hơn số note: mọi note khác, giảm dần.
    let all = knn(&v, 10);
    assert_eq!(all["c.md"].len(), 3);
    assert!(all["c.md"].windows(2).all(|w| w[0].1 >= w[1].1));
    assert!(knn(&v, 0).values().all(Vec::is_empty));
}
