//! Server giả nói giao thức stdio của OHS, chỉ để test client (không cần Node, không cần model).
//!
//! - In `{"ready":true}` rồi đọc JSON Lines.
//! - `search`: trả sau `options.delayMs` ms (để phản hồi về khác thứ tự), một kết quả có
//!   `path` = `query` ở dạng NFD.
//! - `status`: trả payload cố định.
//! - Hết stdin: chờ mọi request xong rồi thoát (như OHS).
//!
//! `FAKE_OHS_MODE`: `die` (thoát trước ready), `silent` (không bao giờ ready),
//! `ignore-eof` (ready, nhưng không thoát khi stdin đóng).

use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use unicode_normalization::UnicodeNormalization;

fn main() {
    let mode = std::env::var("FAKE_OHS_MODE").unwrap_or_default();
    match mode.as_str() {
        "die" => {
            eprintln!("Error: model not found");
            std::process::exit(3);
        }
        "silent" => loop {
            thread::sleep(Duration::from_secs(60));
        },
        _ => {}
    }
    let out = Arc::new(Mutex::new(std::io::stdout()));
    let say = |out: &Mutex<std::io::Stdout>, v: Value| {
        let mut o = out
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = writeln!(o, "{v}");
        let _ = o.flush();
    };
    say(&out, json!({"event": "noise"}));
    say(&out, json!({"ready": true}));

    let mut workers = Vec::new();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(req) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let out = Arc::clone(&out);
        workers.push(thread::spawn(move || {
            let id = req["id"].clone();
            let reply = match req["action"].as_str() {
                Some("status") => json!({"id": id, "status": {
                    "total": 4, "indexed": 1, "pending": 3,
                    "model": "local:m", "active_model": "local:m"
                }}),
                None | Some("search") => {
                    let delay = req["options"]["delayMs"].as_u64().unwrap_or(0);
                    thread::sleep(Duration::from_millis(delay));
                    let q = req["query"].as_str().unwrap_or_default();
                    let nfd: String = q.nfd().collect();
                    json!({"id": id, "results": [{"path": nfd, "score": 1.0}]})
                }
                Some(other) => json!({"id": id, "error": format!("unknown action: {other}")}),
            };
            say(&out, reply);
        }));
    }
    for w in workers {
        let _ = w.join();
    }
    if mode == "ignore-eof" {
        loop {
            thread::sleep(Duration::from_secs(60));
        }
    }
}
