//! `vectors`, `neighbors` của OHS và phần tính trên vector (`pairs`) với gói OHS thật. Bỏ qua mặc định (CI không có
//! Node, gói hay model). Chạy:
//!
//! ```text
//! PKM_OHS_DIR=<thư mục gói> PKM_OHS_TEST_VAULT=<vault kg-solo> cargo test -p pkm-core --test ohs_vectors_real -- --ignored
//! ```
//!
//! DB của OHS nằm trong thư mục tạm (`OBSIDIAN_DB_PATH`), không ghi gì vào vault. Lần đầu OHS có thể phải tải model.

use std::path::PathBuf;
use std::time::Duration;

use pkm_core::ohs::{Ohs, OhsConfig};
use pkm_core::pairs::{self, NoteVectors, PairOptions};
use serde_json::{Value, json};
use unicode_normalization::is_nfc;

#[path = "support/process.rs"]
mod process;

#[test]
#[ignore = "cần gói OHS thật (PKM_OHS_DIR) và vault (PKM_OHS_TEST_VAULT)"]
fn real_ohs_exports_unit_vectors_and_neighbors_agree_with_them() {
    let vault = PathBuf::from(std::env::var_os("PKM_OHS_TEST_VAULT").expect("PKM_OHS_TEST_VAULT"));
    let tmp = tempfile::tempdir().unwrap();
    let ohs = Ohs::launch(
        None,
        OhsConfig {
            db: Some(tmp.path().join("ohs.db")),
            ..OhsConfig::new(&vault)
        },
    );
    let state = ohs.wait_ready(Duration::from_secs(900));
    assert!(state.is_ready(), "{state:?}");
    let pid = ohs.pid().unwrap();

    // Mọi note có vector: `meta` đủ khoá, vector đơn vị, đường dẫn NFC, cùng số chiều.
    let all = ohs.vectors(&[], Value::Null).unwrap();
    for key in ["model", "dim", "pooling", "note_count", "ohs_version"] {
        assert!(
            all.meta.contains_key(key),
            "meta thiếu {key}: {:?}",
            all.meta
        );
    }
    assert_eq!(all.meta["pooling"], "mean");
    assert_eq!(
        all.meta["note_count"].as_u64().unwrap() as usize,
        all.records.len()
    );
    let dim = all.meta["dim"].as_u64().unwrap() as usize;
    for r in &all.records {
        let p = r["path"].as_str().unwrap();
        assert!(is_nfc(p), "{p} chưa NFC");
        let v: Vec<f64> = r["vector"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap())
            .collect();
        assert_eq!(v.len(), dim, "{p}");
        let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6, "{p}: chuẩn {norm}");
    }
    let vectors = NoteVectors::from_reply(&all, |_| true).unwrap();
    assert_eq!(vectors.len(), all.records.len());

    // Hỏi đích danh: note có và note không có.
    let first = vectors.paths[0].clone();
    let missing = "Không hề có note này.md".to_owned();
    let some = ohs
        .vectors(
            &[first.clone(), missing.clone()],
            json!({"normalize": false}),
        )
        .unwrap();
    assert_eq!(some.records[0]["path"], first.as_str());
    assert_eq!(
        some.records[1],
        json!({"path": missing, "error": "note not found in the index"})
    );

    // `neighbors` của OHS khớp cosine tính lại trên vector đã xuất (thứ tự và điểm, sai số `f32`).
    let k = 5;
    let near = ohs
        .neighbors(std::slice::from_ref(&first), json!({"k": k}))
        .unwrap();
    let list = near.records[0]["neighbors"].as_array().unwrap();
    assert_eq!(list.len(), k.min(vectors.len() - 1));
    let knn = pairs::knn(&vectors, k);
    for ((p, s), n) in knn[&first].iter().zip(list) {
        assert!(is_nfc(n["path"].as_str().unwrap()));
        let got = n["score"].as_f64().unwrap();
        assert!((got - s).abs() < 2e-3, "{p}: OHS {got}, tính lại {s}");
    }

    // Cặp trên mọi note: điểm giảm dần, không cặp nào cùng nhóm ba đoạn đường dẫn.
    let p = pairs::similar_pairs(
        &vectors,
        &PairOptions {
            min: 0.9,
            max: 200,
            per_note: 3,
            min_words: 0,
        },
    );
    let scores: Vec<f64> = p
        .picked
        .iter()
        .map(|x| x["score"].as_f64().unwrap())
        .collect();
    assert!(scores.windows(2).all(|w| w[0] >= w[1]), "{scores:?}");
    let group = |s: &str| s.split('/').take(3).collect::<Vec<_>>().join("/");
    for x in &p.picked {
        assert_ne!(
            group(x["a"].as_str().unwrap()),
            group(x["b"].as_str().unwrap()),
            "{x}"
        );
    }

    ohs.shutdown();
    assert!(!process::alive(pid), "node {pid} còn chạy sau khi tắt");
}
