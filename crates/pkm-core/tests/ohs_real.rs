//! Module `ohs` với gói OHS thật. Bỏ qua mặc định (CI không có Node, gói hay model). Chạy:
//!
//! ```text
//! PKM_OHS_DIR=<thư mục gói> PKM_OHS_TEST_VAULT=<vault kg-solo> cargo test -p pkm-core --test ohs_real -- --ignored
//! ```
//!
//! DB của OHS nằm trong thư mục tạm (`OBSIDIAN_DB_PATH`), không ghi gì vào vault. Lần đầu OHS có thể phải tải model.

use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use pkm_core::ohs::{Ohs, OhsConfig};
use serde_json::json;
use unicode_normalization::is_nfc;

#[path = "support/process.rs"]
mod process;

#[test]
#[ignore = "cần gói OHS thật (PKM_OHS_DIR) và vault (PKM_OHS_TEST_VAULT)"]
fn real_ohs_searches_concurrently_with_nfc_paths_and_exits_cleanly() {
    let vault = PathBuf::from(std::env::var_os("PKM_OHS_TEST_VAULT").expect("PKM_OHS_TEST_VAULT"));
    let tmp = tempfile::tempdir().unwrap();
    let ohs = Ohs::launch(
        None,
        OhsConfig {
            db: Some(tmp.path().join("ohs.db")),
            ..OhsConfig::new(&vault)
        },
    );
    let state = ohs.wait_ready(Duration::from_secs(600));
    assert!(state.is_ready(), "{state:?}");
    let pid = ohs.pid().unwrap();

    let queries = ["khách hàng", "hợp đồng", "báo giá", "quy trình", "Đà Nẵng"];
    let answers: Vec<Vec<String>> = thread::scope(|s| {
        let handles: Vec<_> = queries
            .iter()
            .map(|q| {
                let ohs = &ohs;
                s.spawn(move || {
                    ohs.search(q, json!({"limit": 10, "snippetLength": 0}))
                        .unwrap()
                        .iter()
                        .map(|r| r["path"].as_str().unwrap().to_owned())
                        .collect()
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for (q, paths) in queries.iter().zip(&answers) {
        assert!(!paths.is_empty(), "{q}: không có kết quả");
        for p in paths {
            assert!(is_nfc(p), "{q}: {p:?} chưa NFC");
            assert!(
                vault.join(p).is_file() || vault_has_nfd(&vault, p),
                "{q}: {p} không có trong vault"
            );
        }
    }

    // Note thử của bài thử OHS: tên trên đĩa dạng NFD, chứa từ khoá riêng. Có note đó thì nó phải đứng đầu, ở NFC.
    let nfd_note = "Ghi chú thử nghiệm Đà Nẵng NFD.md";
    if vault_has_nfd(&vault, nfd_note) {
        let top = ohs
            .search("quyxuyenzorblat", json!({"limit": 3, "snippetLength": 0}))
            .unwrap();
        assert_eq!(top[0]["path"], nfd_note);
    }

    ohs.shutdown();
    assert!(!process::alive(pid), "node {pid} còn chạy sau khi tắt");
}

/// Tên trên đĩa ở dạng NFD (macOS, hoặc file tạo bằng NFD): so sau khi chuẩn hoá.
fn vault_has_nfd(vault: &std::path::Path, rel: &str) -> bool {
    use unicode_normalization::UnicodeNormalization;
    let nfd: String = rel.nfd().collect();
    vault.join(nfd).is_file()
}
