//! Client stdio với server giả (`fake-ohs`): ghép theo `id`, NFC, trạng thái, tắt có giới hạn.
//! Test với OHS thật nằm cuối file, chỉ chạy khi có `PKM_OHS_DIR` và `S3_OHS_VAULT`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use s3_ohs::{OhsClient, OhsConfig, OhsError, OhsPackage, OhsState};
use serde_json::json;

const T: Duration = Duration::from_secs(10);

fn fake(mode: &str) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_fake-ohs"));
    c.env("FAKE_OHS_MODE", mode)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    c
}

fn alive(pid: u32) -> bool {
    // Linux: /proc/<pid> mất khi tiến trình đã thoát và đã được wait.
    Path::new(&format!("/proc/{pid}")).exists()
}

#[test]
fn concurrent_searches_are_matched_by_id_even_when_answers_arrive_out_of_order() {
    let client = OhsClient::spawn(fake(""), T, T).unwrap();
    let queries = [
        "Đà Nẵng",
        "khách hàng",
        "giữ chân",
        "hợp đồng",
        "báo giá",
        "nhật ký",
    ];
    let started = Instant::now();
    let answers: Vec<String> = thread::scope(|s| {
        let hs: Vec<_> = queries
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let client = &client;
                // Câu đầu chậm nhất: phản hồi về theo thứ tự ngược.
                let delay = 300 - 50 * i as u64;
                s.spawn(move || client.search(q, json!({ "delayMs": delay }), T).unwrap())
            })
            .collect();
        hs.into_iter()
            .map(|h| h.join().unwrap()[0]["path"].as_str().unwrap().to_owned())
            .collect()
    });
    // Song song: tổng thời gian gần câu chậm nhất, không phải tổng các câu.
    assert!(
        started.elapsed() < Duration::from_millis(1200),
        "{:?}",
        started.elapsed()
    );
    // Server trả NFD; client đưa về NFC, đúng câu của từng request.
    assert_eq!(answers, queries);
    assert!(client.shutdown());
}

#[test]
fn status_maps_to_indexing_state_with_progress() {
    let client = OhsClient::spawn(fake(""), T, T).unwrap();
    assert_eq!(
        client.state(T),
        OhsState::Indexing {
            progress: 0.25,
            indexed: 1,
            pending: 3
        }
    );
    assert!(client.shutdown());
}

#[test]
fn remote_error_is_reported_with_its_message() {
    let client = OhsClient::spawn(fake(""), T, T).unwrap();
    match client.call(json!({ "action": "nope" }), T) {
        Err(OhsError::Remote { message, .. }) => assert!(message.contains("unknown action")),
        other => panic!("{other:?}"),
    }
    assert!(client.shutdown());
}

#[test]
fn exit_before_ready_returns_stderr() {
    match OhsClient::spawn(fake("die"), T, T) {
        Err(OhsError::ExitedBeforeReady { stderr, .. }) => {
            assert!(stderr.contains("model not found"))
        }
        Err(e) => panic!("{e}"),
        Ok(_) => panic!("không được ready"),
    }
}

#[test]
fn ready_timeout_kills_the_child() {
    let started = Instant::now();
    let err = OhsClient::spawn(
        fake("silent"),
        Duration::from_millis(300),
        Duration::from_millis(300),
    );
    assert!(matches!(err, Err(OhsError::ReadyTimeout { .. })));
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn closing_stdin_lets_the_server_exit_on_its_own() {
    let client = OhsClient::spawn(fake(""), T, T).unwrap();
    let pid = client.pid();
    assert!(client.shutdown(), "server phải tự thoát khi stdin đóng");
    assert!(!alive(pid));
}

#[test]
fn server_ignoring_eof_is_killed_after_the_bound() {
    let client = OhsClient::spawn(fake("ignore-eof"), T, Duration::from_millis(300)).unwrap();
    let pid = client.pid();
    let started = Instant::now();
    assert!(!client.shutdown(), "phải kill");
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(!alive(pid));
}

#[test]
fn dropping_the_client_also_stops_the_server() {
    let client = OhsClient::spawn(fake("ignore-eof"), T, Duration::from_millis(300)).unwrap();
    let pid = client.pid();
    drop(client);
    assert!(!alive(pid));
}

/// OHS thật: `PKM_OHS_DIR=<gói> S3_OHS_VAULT=<vault> [S3_OHS_DB=<file>] cargo test -- --ignored`.
#[test]
#[ignore = "cần gói OHS dựng sẵn, vault và model nhúng trong cache"]
fn real_ohs_answers_concurrent_searches_and_exits_cleanly() {
    let pkg = OhsPackage::locate(None).expect("PKM_OHS_DIR");
    let vault = PathBuf::from(std::env::var_os("S3_OHS_VAULT").expect("S3_OHS_VAULT"));
    let mut cfg = OhsConfig::new(vault);
    cfg.db = std::env::var_os("S3_OHS_DB").map(PathBuf::from);
    let client = OhsClient::start(&pkg, &cfg).expect("ready");
    let pid = client.pid();
    let status = client.status(T).expect("status");
    assert!(status.get("total").is_some(), "{status}");
    let queries = ["khách hàng", "hợp đồng", "báo giá", "quy trình", "Đà Nẵng"];
    let results: Vec<_> = thread::scope(|s| {
        let hs: Vec<_> = queries
            .iter()
            .map(|q| {
                let client = &client;
                s.spawn(move || client.search(q, json!({ "limit": 5 }), Duration::from_secs(60)))
            })
            .collect();
        hs.into_iter()
            .map(|h| h.join().unwrap().expect("search"))
            .collect()
    });
    for r in &results {
        for hit in r {
            let p = hit["path"].as_str().unwrap();
            assert_eq!(p, s3_ohs::nfc(p));
        }
    }
    assert!(client.shutdown(), "OHS phải tự thoát khi stdin đóng");
    assert!(!alive(pid));
}
