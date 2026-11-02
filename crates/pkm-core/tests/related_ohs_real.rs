//! Tool `related` chỉ có câu hỏi với gói OHS thật. Bỏ qua mặc định (CI không có Node, gói hay model). Chạy:
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
    assert_eq!(r["seed_source"], "ohs-cli", "{r}");
    let seeds = r["seeds"].as_array().unwrap();
    assert!(!seeds.is_empty() && seeds.len() <= 10, "{r}");
    assert!(seeds.iter().all(|s| s["found_by"] == "ohs"));
    // Đường dẫn OHS trả (NFD) đã chuẩn hoá NFC nên tra được trong index; chỉ file không phải note mới vào `missing`.
    for m in r["missing"].as_array().unwrap() {
        assert!(!m.as_str().unwrap().ends_with(".md"), "{m}");
    }
    assert!(!r["related"].as_array().unwrap().is_empty(), "{r}");
}
