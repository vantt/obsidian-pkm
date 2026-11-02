//! `s3-ohs [--ohs DIR] --vault DIR [--db FILE] [--limit N] [--wait-indexed SECS] QUERY...`
//!
//! Mở một tiến trình OHS stdio, chờ ready, (tuỳ chọn) chờ index xong và in tiến độ,
//! gửi mọi QUERY cùng lúc, in mỗi câu một dòng JSON `{"query","paths","scores"}` ra stdout
//! (đường dẫn NFC, thứ tự OHS). Nhật ký trạng thái ra stderr.

use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant};

use s3_ohs::{OhsClient, OhsConfig, OhsPackage, OhsState};
use serde_json::{Value, json};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

struct Args {
    ohs: Option<PathBuf>,
    vault: PathBuf,
    db: Option<PathBuf>,
    limit: u64,
    wait_indexed: Option<Duration>,
    queries: Vec<String>,
}

fn usage() -> String {
    "dùng: s3-ohs [--ohs DIR] --vault DIR [--db FILE] [--limit N] [--wait-indexed SECS] QUERY..."
        .to_owned()
}

fn parse_args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let (mut ohs, mut vault, mut db, mut limit, mut wait_indexed) = (None, None, None, 10, None);
    let mut queries = Vec::new();
    while let Some(a) = it.next() {
        let mut val = |name: &str| {
            it.next()
                .ok_or_else(|| format!("{name} cần giá trị\n{}", usage()))
        };
        match a.as_str() {
            "--ohs" => ohs = Some(PathBuf::from(val("--ohs")?)),
            "--vault" => vault = Some(PathBuf::from(val("--vault")?)),
            "--db" => db = Some(PathBuf::from(val("--db")?)),
            "--limit" => {
                limit = val("--limit")?
                    .parse()
                    .map_err(|e| format!("--limit: {e}"))?
            }
            "--wait-indexed" => {
                let s: u64 = val("--wait-indexed")?
                    .parse()
                    .map_err(|e| format!("--wait-indexed: {e}"))?;
                wait_indexed = Some(Duration::from_secs(s));
            }
            "-h" | "--help" => return Err(usage()),
            s if s.starts_with("--") => return Err(format!("cờ lạ: {s}\n{}", usage())),
            _ => queries.push(a),
        }
    }
    let vault = vault.ok_or_else(|| format!("thiếu --vault\n{}", usage()))?;
    Ok(Args {
        ohs,
        vault,
        db,
        limit,
        wait_indexed,
        queries,
    })
}

/// Hỏi `status` mỗi 0,5 s khi còn `starting` hoặc `indexing`, tới khi đổi trạng thái khác
/// hoặc hết giờ; in trạng thái và các trường thô của `status` mỗi lần chúng đổi.
/// Vault rỗng thì ở `starting` mãi: hết giờ là thoát.
fn wait_indexed(client: &OhsClient, limit: Duration) -> OhsState {
    const FIELDS: [&str; 6] = [
        "total",
        "indexed",
        "pending",
        "chunks",
        "model",
        "active_model",
    ];
    let start = Instant::now();
    let mut last = String::new();
    loop {
        let (state, raw) = match client.status(REQUEST_TIMEOUT) {
            Ok(s) => {
                let raw: serde_json::Map<String, Value> = FIELDS
                    .iter()
                    .map(|k| ((*k).to_owned(), s.get(*k).cloned().unwrap_or(Value::Null)))
                    .collect();
                (OhsState::from_status(&s), Value::Object(raw))
            }
            Err(e) => (
                OhsState::Broken {
                    detail: e.to_string(),
                },
                Value::Null,
            ),
        };
        let line = format!("{} {raw}", state.name());
        if line != last {
            eprintln!("[s3-ohs] {:>6.1}s {line}", start.elapsed().as_secs_f64());
            last = line;
        }
        let waiting = matches!(state, OhsState::Starting | OhsState::Indexing { .. });
        if !waiting || start.elapsed() >= limit {
            return state;
        }
        thread::sleep(Duration::from_millis(500));
    }
}

fn run(args: Args) -> Result<(), String> {
    let pkg = OhsPackage::locate(args.ohs.as_deref()).map_err(|e| e.to_string())?;
    let mut cfg = OhsConfig::new(&args.vault);
    cfg.db = args.db;
    let t0 = Instant::now();
    let client = OhsClient::start(&pkg, &cfg).map_err(|e| e.to_string())?;
    eprintln!(
        "[s3-ohs] ready sau {:.2}s, pid {}",
        t0.elapsed().as_secs_f64(),
        client.pid()
    );

    if let Some(limit) = args.wait_indexed {
        let state = wait_indexed(&client, limit);
        eprintln!("[s3-ohs] trạng thái sau khi chờ index: {}", state.name());
    }

    let t1 = Instant::now();
    let options = json!({ "limit": args.limit, "snippetLength": 0 });
    let outcomes: Vec<Result<Vec<Value>, String>> = thread::scope(|s| {
        let handles: Vec<_> = args
            .queries
            .iter()
            .map(|q| {
                let (client, options) = (&client, options.clone());
                s.spawn(move || {
                    client
                        .search(q, options, REQUEST_TIMEOUT)
                        .map_err(|e| e.to_string())
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err("luồng tìm bị panic".to_owned()))
            })
            .collect()
    });
    eprintln!(
        "[s3-ohs] {} câu tìm đồng thời xong sau {:.2}s",
        args.queries.len(),
        t1.elapsed().as_secs_f64()
    );

    let mut failed = false;
    for (q, outcome) in args.queries.iter().zip(outcomes) {
        let line = match outcome {
            Ok(results) => json!({
                "query": q,
                "paths": results.iter().map(|r| r["path"].clone()).collect::<Vec<_>>(),
                "scores": results.iter().map(|r| r["score"].clone()).collect::<Vec<_>>(),
            }),
            Err(e) => {
                failed = true;
                json!({ "query": q, "error": e })
            }
        };
        println!("{line}");
    }

    let pid = client.pid();
    let graceful = client.shutdown();
    eprintln!(
        "[s3-ohs] đã đóng OHS pid {pid} ({})",
        if graceful {
            "tự thoát"
        } else {
            "phải kill"
        }
    );
    if failed {
        Err("có câu tìm lỗi".to_owned())
    } else {
        Ok(())
    }
}

fn main() -> ExitCode {
    match parse_args().and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
