//! Chạy binary `pkm VAULT mcp` làm tiến trình con và nói JSON-RPC theo dòng trên stdin/stdout như Claude Desktop:
//! bắt tay, notification không được trả lời, phương thức lạ, dòng hỏng, và thoát sạch khi stdin đóng.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

fn write_vault(dir: &Path) {
    for (path, fm) in [
        ("Yếu tố/Doanh thu.md", "type: factor"),
        (
            "Yếu tố/Số lead.md",
            "type: factor\nincreases: [\"[[Doanh thu]]\"]",
        ),
        ("Khách hàng/Công ty ABC.md", "type: organization"),
    ] {
        let p = dir.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, format!("---\n{fm}\n---\n")).unwrap();
    }
}

#[test]
fn stdio_session_handshakes_and_exits_on_eof() {
    let tmp = tempfile::tempdir().unwrap();
    let vault = tmp.path().join("kg-test");
    write_vault(&vault);
    let db = tmp.path().join("idx").join("graph.db");
    let mut child = Command::new(env!("CARGO_BIN_EXE_pkm"))
        .arg(&vault)
        .arg("mcp")
        .arg("--db")
        .arg(&db)
        .args(["--today", "2026-03-01", "--about", "Test vault."])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("chạy được pkm");
    let mut stdin = child.stdin.take().unwrap();
    let input = [
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#,
        "{oops",
        r#"{"jsonrpc":"2.0","id":4,"method":"nope/x"}"#,
    ];
    for line in input {
        writeln!(stdin, "{line}").unwrap();
    }
    drop(stdin);

    let out = BufReader::new(child.stdout.take().unwrap());
    let lines: Vec<String> = out.lines().map(Result::unwrap).collect();
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        assert!(
            Instant::now() < deadline,
            "pkm không thoát sau khi stdin đóng"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(status.success(), "{status:?}");
    assert_eq!(lines.len(), 5, "{lines:#?}");

    // Bản tin in như `json.dumps(..., ensure_ascii=False)` của Python.
    assert!(lines[0].starts_with(r#"{"jsonrpc": "2.0", "id": 1, "result": {"protocolVersion": "2024-11-05", "capabilities": {"tools": {"listChanged": false}}, "serverInfo": {"name": "kg", "version": "0.1.0"}, "instructions": "Typed knowledge graph (kg) of the user's Obsidian vault \"kg-test\": 3 notes. What the vault describes: Test vault. Today is 2026-03-01; use it for \"now\", \"currently\", \"đang\", \"hiện tại\".\n"#), "{}", lines[0]);
    let init: Value = serde_json::from_str(&lines[0]).unwrap();
    let ins = init["result"]["instructions"].as_str().unwrap();
    assert!(
        ins.contains("increases (inverse increased_by, polarity +)"),
        "{ins}"
    );
    assert!(
        ins.contains("obsidian://open?vault=kg-test&file=PATH"),
        "{ins}"
    );
    assert_eq!(lines[1], r#"{"jsonrpc": "2.0", "id": 2, "result": {}}"#);
    let tools: Value = serde_json::from_str(&lines[2]).unwrap();
    assert!(tools["result"]["tools"].is_array(), "{}", lines[2]);
    assert_eq!(
        lines[3],
        r#"{"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "parse error"}}"#
    );
    assert_eq!(
        lines[4],
        r#"{"jsonrpc": "2.0", "id": 4, "error": {"code": -32601, "message": "method not found: nope/x"}}"#
    );
    assert!(db.is_file(), "index dựng ở nền vào --db");
}

#[test]
fn bad_flag_is_a_usage_error() {
    let out = Command::new(env!("CARGO_BIN_EXE_pkm"))
        .args(["v", "mcp", "--edge-keys", "nope"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty(), "stdout chỉ dành cho bản tin");
}
