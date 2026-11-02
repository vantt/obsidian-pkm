//! Module `ohs` với server giả `fake-ohs` (không cần Node hay model): ghép request theo `id`, NFC, trạng thái D7,
//! khởi động lại có giới hạn, và không để lại tiến trình sau khi tắt.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use pkm_core::ohs::{Ohs, OhsConfig, OhsError, OhsPackage, OhsState};
use serde_json::{Value, json};
use tempfile::TempDir;

#[path = "support/process.rs"]
mod process;

use process::{alive, kill_hard};

struct Fixture {
    tmp: TempDir,
}

impl Fixture {
    fn new(config: Value) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("vault")).unwrap();
        let f = Self { tmp };
        f.configure(&config);
        f
    }

    fn vault(&self) -> PathBuf {
        self.tmp.path().join("vault")
    }

    /// `fake-ohs` đọc lại file này ở mỗi request.
    fn configure(&self, config: &Value) {
        fs::write(self.vault().join("fake-ohs.json"), config.to_string()).unwrap();
    }

    /// Gói giả: `node` là `fake-ohs`, `cli.js` chỉ là tham số.
    fn package(&self) -> OhsPackage {
        OhsPackage {
            root: self.tmp.path().to_path_buf(),
            node: PathBuf::from(env!("CARGO_BIN_EXE_fake-ohs")),
            cli: self.tmp.path().join("app/dist/src/cli.js"),
            pack: None,
        }
    }

    fn config(&self) -> OhsConfig {
        OhsConfig {
            ready_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(10),
            status_timeout: Duration::from_secs(5),
            shutdown_timeout: Duration::from_secs(2),
            max_restarts: 2,
            restart_backoff: Duration::from_millis(20),
            stable_after: Duration::from_secs(60),
            poll_interval: Duration::from_millis(50),
            ..OhsConfig::new(self.vault())
        }
    }

    fn start(&self) -> Ohs {
        Ohs::start(self.package(), self.config())
    }

    fn starts(&self) -> usize {
        fs::read_to_string(self.vault().join("fake-ohs.starts"))
            .map(|s| s.lines().count())
            .unwrap_or(0)
    }

    fn seen_env(&self) -> Value {
        serde_json::from_str(&fs::read_to_string(self.vault().join("fake-ohs.env.json")).unwrap())
            .unwrap()
    }
}

const WAIT: Duration = Duration::from_secs(10);

fn ready(ohs: &Ohs) {
    let state = ohs.wait_ready(WAIT);
    assert!(state.is_ready(), "{state:?}");
}

/// Chờ tới khi `cond` đúng, tối đa [`WAIT`].
fn eventually(what: &str, mut cond: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !cond() {
        assert!(Instant::now() < deadline, "hết giờ chờ: {what}");
        thread::sleep(Duration::from_millis(20));
    }
}

fn nfd(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfd().collect()
}

#[test]
fn search_returns_nfc_paths_once_ready() {
    let f = Fixture::new(json!({}));
    let ohs = f.start();
    ready(&ohs);
    let results = ohs.search("Đà Nẵng", json!({"limit": 5})).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["path"], "Đà Nẵng");
    assert_ne!(results[0]["path"], nfd("Đà Nẵng"));
    assert_eq!(results[0]["score"], 1.0);
}

#[test]
fn read_sends_paths_and_options_and_returns_notes_in_order_with_nfc_paths() {
    let f = Fixture::new(json!({}));
    let ohs = f.start();
    ready(&ohs);
    let paths = vec!["Đà Nẵng.md".to_owned(), "thiếu.md".to_owned()];
    let notes = ohs
        .read(&paths, json!({"snippetLength": 50, "related": false}))
        .unwrap();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0]["path"], "Đà Nẵng.md");
    assert_eq!(notes[0]["found"], true);
    assert_eq!(notes[0]["links"], json!(["Liên kết.md"]));
    assert_eq!(
        notes[0]["request"],
        json!({"action": "read", "paths": ["Đà Nẵng.md", "thiếu.md"], "snippetLength": 50, "related": false})
    );
    assert_eq!(
        notes[1],
        json!({"path": "thiếu.md", "found": false, "suggestions": ["Gợi ý.md"]})
    );
    // Không có tuỳ chọn: request chỉ có `action` và `paths`, để OHS dùng mặc định của nó.
    let notes = ohs.read(&paths[..1], Value::Null).unwrap();
    assert_eq!(
        notes[0]["request"],
        json!({"action": "read", "paths": ["Đà Nẵng.md"]})
    );
}

#[test]
fn read_reports_not_ready_while_indexing() {
    let f = Fixture::new(json!({}));
    let ohs = f.start();
    ready(&ohs);
    f.configure(&json!({"status": {"total": 10, "indexed": 3, "pending": 7,
                                   "model": "local:m", "active_model": "local:m", "indexing_errors": []}}));
    eventually("indexing", || ohs.state().name() == "indexing");
    let err = ohs.read(&["a.md".to_owned()], Value::Null).unwrap_err();
    let e = err.not_ready_error().expect("ohs_not_ready");
    assert_eq!(e["state"], "indexing");
    assert_eq!(e.as_object().unwrap().len(), 5, "{e}");
}

#[test]
fn ohs_is_started_with_vault_db_and_clean_environment() {
    let f = Fixture::new(json!({}));
    let db = f.tmp.path().join("ohs.db");
    let ohs = Ohs::start(
        f.package(),
        OhsConfig {
            db: Some(db.clone()),
            ..f.config()
        },
    );
    ready(&ohs);
    let seen = f.seen_env();
    let cli = f.package().cli.to_string_lossy().into_owned();
    assert_eq!(seen["args"], json!([cli, "serve", "--stdio"]));
    assert_eq!(seen["vault"], f.vault().to_string_lossy().as_ref());
    assert_eq!(seen["db"], db.to_string_lossy().as_ref());
    assert_eq!(seen["update_check"], "1");
    assert_eq!(seen["node_options"], Value::Null);
    assert_eq!(seen["language_pack"], Value::Null);
    let cwd = PathBuf::from(seen["cwd"].as_str().unwrap());
    assert_eq!(
        cwd.canonicalize().unwrap(),
        f.tmp.path().canonicalize().unwrap()
    );
}

#[test]
fn ohs_runs_with_the_vietnamese_pack_of_its_package() {
    let f = Fixture::new(json!({}));
    let pack = f.tmp.path().join("lang/ohs-lang-vi");
    let ohs = Ohs::start(
        OhsPackage {
            pack: Some(pack.clone()),
            ..f.package()
        },
        f.config(),
    );
    ready(&ohs);
    assert_eq!(
        f.seen_env()["language_pack"],
        pack.to_string_lossy().as_ref()
    );
}

#[test]
fn relative_vault_is_made_absolute_before_ohs_changes_directory() {
    let f = Fixture::new(json!({}));
    let rel = pathdiff_from_cwd(&f.vault());
    let ohs = Ohs::start(
        f.package(),
        OhsConfig {
            vault: rel,
            ..f.config()
        },
    );
    ready(&ohs);
    let seen = PathBuf::from(f.seen_env()["vault"].as_str().unwrap());
    assert!(seen.is_absolute(), "{seen:?}");
    assert_eq!(
        seen.canonicalize().unwrap(),
        f.vault().canonicalize().unwrap()
    );
}

/// `p` viết tương đối so với thư mục hiện tại (`..` lặp lại tới gốc chung). Trên Windows khác ổ đĩa thì giữ nguyên.
fn pathdiff_from_cwd(p: &Path) -> PathBuf {
    let cwd = std::env::current_dir().unwrap();
    let common = cwd
        .ancestors()
        .find(|a| p.starts_with(a))
        .map(Path::to_path_buf);
    match common {
        Some(c) => {
            let ups = cwd.strip_prefix(&c).unwrap().components().count();
            let mut out: PathBuf = std::iter::repeat_n("..", ups).collect();
            out.push(p.strip_prefix(&c).unwrap());
            out
        }
        None => p.to_path_buf(),
    }
}

#[test]
fn concurrent_searches_are_matched_by_id_even_out_of_order() {
    let f = Fixture::new(json!({}));
    let ohs = f.start();
    ready(&ohs);
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
        let handles: Vec<_> = queries
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let ohs = &ohs;
                // Câu đầu chậm nhất: phản hồi về theo thứ tự ngược.
                let delay = 600 - 100 * i as u64;
                s.spawn(move || ohs.search(q, json!({ "delayMs": delay })).unwrap())
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap()[0]["path"].as_str().unwrap().to_owned())
            .collect()
    });
    assert_eq!(answers, queries);
    // Chạy song song: tổng thời gian gần câu chậm nhất (600 ms), xa tổng tuần tự (2100 ms).
    assert!(
        started.elapsed() < Duration::from_millis(1500),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn remote_errors_are_reported_as_such() {
    let f = Fixture::new(json!({}));
    let ohs = f.start();
    ready(&ohs);
    assert_eq!(
        ohs.search("fail", Value::Null),
        Err(OhsError::Remote("search failed".into()))
    );
    // Lỗi của một request không làm hỏng các request sau.
    assert!(ohs.search("tiếp", Value::Null).is_ok());
}

#[test]
fn indexing_returns_not_ready_with_progress_and_eta_from_rate() {
    let status = |indexed: u64, pending: u64| {
        json!({"status": {"total": indexed, "indexed": indexed, "pending": pending,
                          "model": "local:m", "active_model": "local:m", "indexing_errors": []}})
    };
    let f = Fixture::new(status(100, 900));
    let ohs = f.start();
    eventually("indexing", || ohs.state().name() == "indexing");
    let err = ohs.search("khách hàng", Value::Null).unwrap_err();
    let e = err.not_ready_error().unwrap();
    assert_eq!(e["state"], "indexing");
    assert_eq!(e["progress"], 0.1);
    assert!(e["action"].as_str().unwrap().contains("100/1000"), "{e}");

    // 400 note sau khoảng 1 s: tốc độ khoảng 400 note/s, còn 500 note là khoảng 2 s.
    thread::sleep(Duration::from_millis(1000));
    f.configure(&status(500, 500));
    let mut eta = None;
    eventually("eta_s", || {
        eta = match ohs.state() {
            OhsState::Indexing { eta_s, .. } => eta_s,
            _ => None,
        };
        eta.is_some()
    });
    let eta = eta.unwrap();
    assert!((1..=5).contains(&eta), "eta_s = {eta}");
    let e = ohs
        .search("x", Value::Null)
        .unwrap_err()
        .not_ready_error()
        .unwrap();
    assert_eq!((&e["progress"], e["eta_s"].is_u64()), (&json!(0.5), true));

    f.configure(&status(1000, 0));
    ready(&ohs);
    assert!(ohs.search("x", Value::Null).is_ok());
}

#[test]
fn not_ready_until_ohs_prints_ready() {
    let f = Fixture::new(json!({"silent": true}));
    let ohs = f.start();
    let e = ohs
        .search("x", Value::Null)
        .unwrap_err()
        .not_ready_error()
        .unwrap();
    assert_eq!(
        e,
        json!({"error": "ohs_not_ready", "state": "starting", "progress": null, "eta_s": null,
               "action": OhsState::Starting.action()})
    );
}

#[test]
fn empty_index_after_ready_is_still_starting() {
    // OHS in `ready` trước khi quét vault; tìm lúc này trả `[]` không báo lỗi, nên phải báo `starting`.
    let f = Fixture::new(
        json!({"status": {"total": 0, "indexed": 0, "pending": 0, "model": null,
                                           "active_model": "local:m", "indexing_errors": []}}),
    );
    let ohs = f.start();
    eventually("đã spawn", || f.starts() == 1);
    thread::sleep(Duration::from_millis(300));
    let err = ohs.search("x", Value::Null).unwrap_err();
    assert_eq!(err, OhsError::NotReady(OhsState::Starting));
}

#[test]
fn model_drift_blocks_search_as_model_missing() {
    let f = Fixture::new(json!({"status": {"total": 4, "indexed": 4, "pending": 0,
                                           "model": "local:a", "active_model": "local:b"}}));
    let ohs = f.start();
    let state = ohs.wait_ready(Duration::from_millis(500));
    assert_eq!(state.name(), "model_missing");
    let e = ohs
        .search("x", Value::Null)
        .unwrap_err()
        .not_ready_error()
        .unwrap();
    assert_eq!(e["state"], "model_missing");
}

#[test]
fn ohs_dying_before_ready_every_time_ends_broken_after_bounded_restarts() {
    let f = Fixture::new(json!({"die_before_ready": 1000}));
    let ohs = f.start();
    let state = ohs.wait_ready(WAIT);
    assert_eq!(state.name(), "broken", "{state:?}");
    // Lần đầu cộng `max_restarts` = 2 lần khởi động lại, rồi thôi.
    assert_eq!(f.starts(), 3);
    let action = state.action();
    assert!(action.contains("cannot open index"), "{action}");
    thread::sleep(Duration::from_millis(200));
    assert_eq!(f.starts(), 3, "không khởi động lại sau khi đã broken");
    let e = ohs
        .search("x", Value::Null)
        .unwrap_err()
        .not_ready_error()
        .unwrap();
    assert_eq!(e["state"], "broken");
}

#[test]
fn ohs_that_never_gets_ready_is_killed_and_ends_broken() {
    let f = Fixture::new(json!({"silent": true}));
    let ohs = Ohs::start(
        f.package(),
        OhsConfig {
            ready_timeout: Duration::from_millis(200),
            max_restarts: 1,
            ..f.config()
        },
    );
    let mut pids = Vec::new();
    let state = loop {
        if let Some(pid) = ohs.pid()
            && !pids.contains(&pid)
        {
            pids.push(pid);
        }
        let s = ohs.state();
        if s.is_terminal() {
            break s;
        }
        thread::sleep(Duration::from_millis(10));
    };
    assert!(state.action().contains("chưa sẵn sàng"), "{state:?}");
    assert_eq!(f.starts(), 2);
    for pid in pids {
        assert!(!alive(pid), "tiến trình {pid} còn sống");
    }
}

#[test]
fn ohs_crashing_after_ready_is_restarted() {
    let f = Fixture::new(json!({"exit_after_ready": 1, "exit_after_ready_ms": 100}));
    let ohs = f.start();
    eventually("khởi động lại", || f.starts() == 2);
    ready(&ohs);
    assert!(ohs.search("x", Value::Null).is_ok());
    thread::sleep(Duration::from_millis(300));
    assert_eq!(f.starts(), 2);
}

#[test]
fn killed_ohs_is_restarted_and_old_process_is_gone() {
    let f = Fixture::new(json!({}));
    let ohs = f.start();
    ready(&ohs);
    let first = ohs.pid().unwrap();
    kill_hard(first);
    eventually("tiến trình mới", || {
        ohs.pid().is_some_and(|p| p != first)
    });
    ready(&ohs);
    assert!(ohs.search("sau khi chết", Value::Null).is_ok());
    assert!(!alive(first));
    assert_eq!(f.starts(), 2);
}

#[test]
fn restart_budget_is_refilled_after_ohs_runs_stably() {
    // Mỗi lần sống lâu hơn `stable_after`: chết bao nhiêu lần cũng không thành broken.
    let f = Fixture::new(json!({"exit_after_ready": 4, "exit_after_ready_ms": 500}));
    let ohs = Ohs::start(
        f.package(),
        OhsConfig {
            max_restarts: 1,
            stable_after: Duration::from_millis(50),
            ..f.config()
        },
    );
    eventually("lần khởi động thứ 5", || f.starts() == 5);
    ready(&ohs);
}

#[test]
fn shutdown_closes_stdin_and_leaves_no_process() {
    let f = Fixture::new(json!({}));
    let ohs = f.start();
    ready(&ohs);
    let pid = ohs.pid().unwrap();
    let started = Instant::now();
    ohs.shutdown();
    // OHS tự thoát khi stdin đóng: không phải chờ hết `shutdown_timeout`.
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    assert!(!alive(pid));
}

#[test]
fn dropping_the_handle_leaves_no_process_even_while_starting() {
    let f = Fixture::new(json!({"silent": true}));
    let ohs = f.start();
    eventually("đã spawn", || ohs.pid().is_some());
    let pid = ohs.pid().unwrap();
    drop(ohs);
    assert!(!alive(pid));
}

#[test]
fn ohs_ignoring_eof_is_killed_after_shutdown_timeout() {
    let f = Fixture::new(json!({"ignore_eof": true}));
    let ohs = Ohs::start(
        f.package(),
        OhsConfig {
            shutdown_timeout: Duration::from_millis(300),
            ..f.config()
        },
    );
    ready(&ohs);
    let pid = ohs.pid().unwrap();
    let started = Instant::now();
    ohs.shutdown();
    assert!(started.elapsed() >= Duration::from_millis(300));
    assert!(!alive(pid));
}

#[test]
fn invalid_utf8_and_a_full_stderr_pipe_do_not_stall_or_restart_ohs() {
    // Byte không phải UTF-8 (cảnh báo theo code page Windows) rồi 256 KB stderr, lớn hơn pipe trên mọi nền: nếu
    // `pkm` ngừng đọc stderr, OHS chặn khi ghi và không bao giờ trả lời.
    let f = Fixture::new(json!({"noise_kb": 256}));
    let ohs = f.start();
    ready(&ohs);
    assert_eq!(
        ohs.search("sau nhiễu", Value::Null).unwrap()[0]["path"],
        "sau nhiễu"
    );
    // Dòng stdout không phải UTF-8 là nhiễu, không phải dấu hiệu OHS đã thoát.
    assert_eq!(f.starts(), 1);
}

#[test]
fn ohs_that_stops_reading_stdin_cannot_block_callers_or_shutdown() {
    let f = Fixture::new(json!({"stop_on": "stop"}));
    let ohs = Ohs::start(
        f.package(),
        OhsConfig {
            request_timeout: Duration::from_millis(300),
            status_timeout: Duration::from_millis(200),
            shutdown_timeout: Duration::from_millis(300),
            ..f.config()
        },
    );
    ready(&ohs);
    let pid = ohs.pid().unwrap();
    assert!(matches!(
        ohs.search("stop", Value::Null),
        Err(OhsError::Timeout(_))
    ));
    // Câu tìm lớn hơn pipe stdin: luồng ghi chặn, người gọi thì không.
    let big = "a".repeat(300 * 1024);
    for _ in 0..3 {
        let started = Instant::now();
        let got = ohs.search(&big, Value::Null);
        assert!(matches!(got, Err(OhsError::Timeout(_))), "{got:?}");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
    }
    // OHS bận (status hết hạn) không bị báo là đang khởi động.
    assert_ne!(ohs.state().name(), "starting");
    let started = Instant::now();
    ohs.shutdown();
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    assert!(!alive(pid));
}

fn holder_pid(f: &Fixture) -> u32 {
    let file = f.vault().join("fake-ohs.holder");
    eventually("tiến trình con đã chạy", || file.exists());
    let mut pid = None;
    eventually("đọc PID tiến trình con", || {
        pid = fs::read_to_string(&file)
            .ok()
            .and_then(|s| s.trim().parse().ok());
        pid.is_some()
    });
    pid.unwrap()
}

#[test]
fn ohs_death_is_noticed_even_when_a_child_keeps_stdout_open() {
    let f = Fixture::new(json!({
        "spawn_holder": true, "exit_after_ready": 1, "exit_after_ready_ms": 100
    }));
    let ohs = f.start();
    let first_holder = holder_pid(&f);
    eventually("khởi động lại dù stdout chưa đóng", || {
        f.starts() == 2
    });
    ready(&ohs);
    // Tiến trình con của lần chạy đầu bị giết cùng nhóm.
    eventually("tiến trình con của lần đầu đã chết", || {
        !alive(first_holder)
    });
}

#[test]
fn shutdown_kills_children_of_ohs_too() {
    let f = Fixture::new(json!({"spawn_holder": true}));
    let ohs = f.start();
    ready(&ohs);
    let holder = holder_pid(&f);
    let pid = ohs.pid().unwrap();
    ohs.shutdown();
    assert!(!alive(pid));
    eventually("tiến trình con đã chết", || !alive(holder));
}

#[test]
fn wait_ready_returns_on_time_while_status_is_slow() {
    let f = Fixture::new(json!({"status_delay_ms": 3000}));
    let ohs = Ohs::start(
        f.package(),
        OhsConfig {
            status_timeout: Duration::from_secs(1),
            ..f.config()
        },
    );
    let started = Instant::now();
    let state = ohs.wait_ready(Duration::from_millis(300));
    assert!(!state.is_ready());
    assert!(
        started.elapsed() < Duration::from_millis(700),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn a_rejected_status_is_not_broken_until_it_keeps_failing() {
    let f = Fixture::new(json!({}));
    let ohs = f.start();
    ready(&ohs);
    // `status` không phải object: OHS trả lời sai định dạng.
    f.configure(&json!({"status": "oops"}));
    let mut seen = Vec::new();
    eventually("broken sau nhiều lần lỗi liên tiếp", || {
        let s = ohs.state();
        seen.push(s.name());
        s.is_terminal()
    });
    // Trước khi đủ số lần lỗi, trạng thái cũ (`ready`) được giữ, không nhảy sang `starting`.
    assert!(!seen.contains(&"starting"), "{seen:?}");
    assert!(ohs.state().action().contains("giao thức"));
    // Trở lại bình thường thì luồng giám sát (vẫn hỏi `status` định kỳ) đưa trạng thái về `ready`.
    f.configure(&json!({}));
    eventually("hết broken", || ohs.state().is_ready());
}
