//! Tool `related` gieo bằng câu hỏi hay note, và tool `search`, với gói OHS thật. Bỏ qua mặc định (CI không có Node, gói hay model). Chạy:
//!
//! ```text
//! PKM_OHS_DIR=<thư mục gói> PKM_OHS_TEST_VAULT=<vault kg-solo> cargo test -p pkm-core --test related_ohs_real -- --ignored
//! ```
//!
//! Index của `pkm` và DB của OHS nằm trong thư mục tạm, không ghi gì vào vault. Lần đầu OHS có thể phải tải model.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use pkm_core::index::{self, Build};
use pkm_core::ohs::{Ohs, OhsConfig};
use pkm_core::tools::{self, Context, Tools};
use serde_json::json;

#[test]
#[ignore = "cần gói OHS thật (PKM_OHS_DIR) và vault (PKM_OHS_TEST_VAULT)"]
fn real_ohs_seeds_related_from_a_question() {
    let vault = PathBuf::from(std::env::var_os("PKM_OHS_TEST_VAULT").expect("PKM_OHS_TEST_VAULT"));
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("graph.db");
    let b = Build {
        packs: vec!["business".into()],
        ..Build::default()
    };
    index::build(&vault, &db, &b).expect("build");
    let ohs = Arc::new(Ohs::launch(
        None,
        OhsConfig {
            db: Some(tmp.path().join("ohs.db")),
            ..OhsConfig::new(&vault)
        },
    ));
    let state = ohs.wait_ready(Duration::from_secs(600));
    assert!(state.is_ready(), "{state:?}");

    let mut c = Context::new(Tools::open(&db).unwrap(), &vault, None).with_ohs(Arc::clone(&ohs));
    let r = tools::call(
        &mut c,
        "related",
        json!({"query": "khách hàng hợp đồng", "k": 10}),
    )
    .unwrap();
    assert_eq!(r["seed_source"], "query", "{r}");
    let seeds = r["seeds"].as_array().unwrap();
    assert!(!seeds.is_empty() && seeds.len() <= 10, "{r}");
    assert!(seeds.iter().all(|s| s["found_by"] == "ohs"));
    assert!(
        seeds
            .iter()
            .any(|s| s["snippet"].as_str().is_some_and(|x| !x.is_empty())),
        "{r}"
    );
    // Đường dẫn OHS trả (NFD) đã chuẩn hoá NFC nên tra được trong index; chỉ file không phải note mới vào `missing`.
    for m in r["missing"].as_array().unwrap() {
        assert!(!m.as_str().unwrap().ends_with(".md"), "{m}");
    }
    assert!(!r["related"].as_array().unwrap().is_empty(), "{r}");

    // `search` qua cùng OHS: kết quả gọn, id có trong index (type từ index), hai câu (song ngữ) gộp được.
    let r = tools::call(
        &mut c,
        "search",
        json!({"query": "hợp đồng", "queries": ["contract"], "limit": 5}),
    )
    .unwrap();
    let items = r["items"].as_array().unwrap();
    assert!(!items.is_empty() && items.len() <= 5, "{r}");
    assert_eq!(r["count"], items.len());
    for it in items {
        let keys: Vec<&str> = it.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, ["id", "title", "type", "score", "snippet"], "{it}");
        assert!(it["type"].is_string(), "{it}");
    }
    // `search(path)` và `related(node)`: note giống nghĩa với một note, không gồm chính note đó.
    let first = items[0]["id"].as_str().unwrap().to_owned();
    let r = tools::call(&mut c, "search", json!({"path": first, "limit": 5})).unwrap();
    let similar = r["items"].as_array().unwrap();
    assert!(!similar.is_empty(), "{r}");
    assert!(similar.iter().all(|x| x["id"] != first.as_str()), "{r}");
    let r = tools::call(&mut c, "related", json!({"node": first, "k": 5})).unwrap();
    assert_eq!(r["seed_source"], "node", "{r}");
    assert!(!r["seeds"].as_array().unwrap().is_empty(), "{r}");
}
