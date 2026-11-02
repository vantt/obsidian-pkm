//! Server giả nói giao thức stdio của OHS, để test module `ohs` của `pkm-core` không cần Node hay model.
//!
//! Chạy như Node của gói OHS: nhận `<cli.js> serve --stdio`, đọc vault từ `OBSIDIAN_VAULT_PATH`. Hành vi lấy từ
//! `<vault>/fake-ohs.json` (thiếu file thì dùng mặc định), đọc lại ở mỗi request nên test đổi được giữa chừng:
//! - `die_before_ready`: số lần khởi động đầu tiên thoát (mã 3) trước khi in `ready`;
//! - `exit_after_ready_ms` + `exit_after_ready`: số lần khởi động đầu tiên tự thoát (mã 4) sau `ready` chừng ấy ms;
//! - `silent`: không bao giờ in `ready`;
//! - `ignore_eof`: không thoát khi stdin đóng;
//! - `status`: giá trị trả trong `status`, kể cả không phải object (mặc định: index xong);
//! - `status_delay_ms`: trả `status` chậm chừng ấy ms;
//! - `noise_kb`: sau `ready`, in ra stderr và stdout byte không phải UTF-8 rồi chừng ấy KB rác vào stderr (lớn hơn
//!   pipe), trước khi đọc request;
//! - `spawn_holder`: sinh một tiến trình con giữ stdout, stderr mở và ngủ mãi (PID ghi vào `<vault>/fake-ohs.holder`);
//! - `stop_on`: gặp câu tìm này thì ngừng đọc stdin hẳn (và không thoát).
//!
//! Mỗi lần khởi động ghi một dòng vào `<vault>/fake-ohs.starts` và ghi môi trường nhận được vào
//! `<vault>/fake-ohs.env.json`. `search` trả sau `options.delayMs` ms một kết quả có `path` là câu tìm ở dạng NFD;
//! câu `fail` thì trả `{"error"}`. `read` trả mỗi đường dẫn một note (dạng NFD): đường dẫn bắt đầu bằng `thiếu` là
//! `{found: false, suggestions}`, còn lại là `{found: true, …, request}` với `request` là request nhận được (bỏ `id`);
//! `paths` rỗng thì trả `{"error"}` như OHS.

use std::fs::{self, OpenOptions};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use unicode_normalization::UnicodeNormalization;

fn config(vault: &Path) -> Value {
    fs::read_to_string(vault.join("fake-ohs.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null)
}

fn say(out: &Mutex<std::io::Stdout>, v: &Value) {
    let mut o = out.lock().unwrap_or_else(PoisonError::into_inner);
    let _ = writeln!(o, "{v}");
    let _ = o.flush();
}

fn sleep_forever() -> ! {
    loop {
        thread::sleep(Duration::from_secs(60));
    }
}

fn main() {
    let vault =
        PathBuf::from(std::env::var_os("OBSIDIAN_VAULT_PATH").expect("OBSIDIAN_VAULT_PATH"));
    if std::env::var_os("FAKE_OHS_HOLDER").is_some() {
        fs::write(
            vault.join("fake-ohs.holder"),
            std::process::id().to_string(),
        )
        .expect("ghi fake-ohs.holder");
        sleep_forever();
    }
    let starts_file = vault.join("fake-ohs.starts");
    let start = fs::read_to_string(&starts_file)
        .map(|s| s.lines().count() as u64)
        .unwrap_or(0);
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&starts_file)
        .expect("ghi fake-ohs.starts");
    writeln!(log, "{}", std::process::id()).expect("ghi fake-ohs.starts");
    let env = |k: &str| std::env::var(k).ok();
    let seen = json!({
        "args": std::env::args().skip(1).collect::<Vec<_>>(),
        "vault": env("OBSIDIAN_VAULT_PATH"),
        "db": env("OBSIDIAN_DB_PATH"),
        "update_check": env("OHS_DISABLE_UPDATE_CHECK"),
        "node_options": env("NODE_OPTIONS"),
        "language_pack": env("OHS_LANGUAGE_PACK"),
        "cwd": std::env::current_dir().ok(),
    });
    fs::write(vault.join("fake-ohs.env.json"), seen.to_string()).expect("ghi fake-ohs.env.json");

    let cfg = config(&vault);
    let count = |k: &str| cfg[k].as_u64().unwrap_or(0);
    if start < count("die_before_ready") {
        eprintln!("Error: cannot open index (lần {start})");
        std::process::exit(3);
    }
    if cfg["silent"].as_bool().unwrap_or(false) {
        sleep_forever();
    }
    if cfg["spawn_holder"].as_bool().unwrap_or(false) {
        // stdin không thừa kế: chỉ stdout, stderr bị giữ mở. Không chờ: tiến trình con phải sống lâu hơn server giả;
        // test kiểm `pkm` giết nó cùng nhóm.
        #[allow(clippy::zombie_processes)]
        std::process::Command::new(std::env::current_exe().expect("current_exe"))
            .env("FAKE_OHS_HOLDER", "1")
            .stdin(std::process::Stdio::null())
            .spawn()
            .expect("sinh tiến trình giữ pipe");
    }
    let out = Arc::new(Mutex::new(std::io::stdout()));
    say(&out, &json!({"event": "noise"}));
    say(&out, &json!({"ready": true}));
    let noise_kb = count("noise_kb");
    if noise_kb > 0 {
        let mut err = std::io::stderr().lock();
        err.write_all(b"\xff\xfe c\xe1nh b\xe1o CP1258\n")
            .expect("ghi stderr");
        let line = [b'x'; 1023];
        for _ in 0..noise_kb {
            err.write_all(&line).expect("ghi stderr");
            err.write_all(b"\n").expect("ghi stderr");
        }
        err.flush().expect("ghi stderr");
        let mut o = out.lock().unwrap_or_else(PoisonError::into_inner);
        o.write_all(b"\xff{\"id\":\"r0\"}\nnot json\n")
            .expect("ghi stdout");
        o.flush().expect("ghi stdout");
    }
    let stop_on = cfg["stop_on"].as_str().map(str::to_owned);
    if start < count("exit_after_ready") {
        let ms = count("exit_after_ready_ms");
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(ms));
            eprintln!("fatal: crashed");
            std::process::exit(4);
        });
    }

    let mut workers = Vec::new();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(req) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if stop_on.is_some() && req["query"].as_str() == stop_on.as_deref() {
            sleep_forever();
        }
        let out = Arc::clone(&out);
        let vault = vault.clone();
        workers.push(thread::spawn(move || {
            let id = req["id"].clone();
            let reply = match req["action"].as_str() {
                Some("status") => {
                    let delay = config(&vault)["status_delay_ms"].as_u64().unwrap_or(0);
                    thread::sleep(Duration::from_millis(delay));
                    let status = match config(&vault).get("status") {
                        Some(s) => s.clone(),
                        _ => json!({"total": 4, "indexed": 4, "pending": 0,
                                    "model": "local:m", "active_model": "local:m", "indexing_errors": []}),
                    };
                    json!({"id": id, "status": status})
                }
                None | Some("search") => {
                    let delay = req["options"]["delayMs"].as_u64().unwrap_or(0);
                    thread::sleep(Duration::from_millis(delay));
                    let q = req["query"].as_str().unwrap_or_default();
                    if q == "fail" {
                        json!({"id": id, "error": "search failed"})
                    } else {
                        let nfd: String = q.nfd().collect();
                        json!({"id": id, "results": [{"path": nfd, "score": 1.0}]})
                    }
                }
                Some("read") => {
                    let paths: Vec<String> = req["paths"]
                        .as_array()
                        .map(|a| a.iter().filter_map(|p| p.as_str().map(str::to_owned)).collect())
                        .unwrap_or_default();
                    if paths.is_empty() {
                        json!({"id": id, "error": "Invalid stdio request: paths must not be empty"})
                    } else {
                        let mut echo = req.clone();
                        echo.as_object_mut().map(|m| m.remove("id"));
                        let notes: Vec<Value> = paths
                            .iter()
                            .map(|p| {
                                let nfd: String = p.nfd().collect();
                                if p.starts_with("thiếu") {
                                    let hint: String = "Gợi ý.md".nfd().collect();
                                    json!({"path": nfd, "found": false, "suggestions": [hint]})
                                } else {
                                    let link: String = "Liên kết.md".nfd().collect();
                                    json!({"path": nfd, "found": true, "title": p, "content": "nội dung",
                                           "links": [link], "backlinks": [], "request": echo})
                                }
                            })
                            .collect();
                        json!({"id": id, "notes": notes})
                    }
                }
                Some(other) => json!({"id": id, "error": format!("unknown action: {other}")}),
            };
            say(&out, &reply);
        }));
    }
    for w in workers {
        let _ = w.join();
    }
    if config(&vault)["ignore_eof"].as_bool().unwrap_or(false) {
        sleep_forever();
    }
}
