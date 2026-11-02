//! Nói JSON-RPC theo dòng với binary thử như Claude Code: initialize, tools/list, tools/call.
//! `s2-rmcp` chỉ được kiểm khi bật feature `rmcp`.

use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Vault nhỏ trong thư mục tạm, xoá khi drop.
struct Vault(PathBuf);

impl Vault {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("s2-mcp-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("Khách hàng")).unwrap();
        fs::create_dir_all(dir.join("Người")).unwrap();
        fs::create_dir_all(dir.join(".obsidian")).unwrap();
        fs::write(
            dir.join("Khách hàng/Công ty ABC.md"),
            "---\ntype: client\n---\nKhách hàng lớn.\n",
        )
        .unwrap();
        fs::write(
            dir.join("Người/Nguyễn Văn Đức.md"),
            "---\ntype: person\n---\n",
        )
        .unwrap();
        fs::write(dir.join("Người/Đức Anh.md"), "không có frontmatter\n").unwrap();
        fs::write(dir.join(".obsidian/Công ty.md"), "ẩn\n").unwrap();
        Vault(dir)
    }
}

impl Drop for Vault {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Gửi cả phiên rồi đóng stdin; server phải thoát sạch và trả đúng một dòng cho mỗi request có id.
fn session(bin: &str, vault: &Path, lines: &[Value]) -> Vec<Value> {
    let mut child = Command::new(bin)
        .arg(vault)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn server");
    {
        let mut stdin = child.stdin.take().unwrap();
        for l in lines {
            writeln!(stdin, "{l}").unwrap();
        }
    }
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "server failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let mut replies: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    replies.sort_by_key(|r| r["id"].as_i64());
    replies
}

fn handshake(bin: &str) {
    let vault = Vault::new(bin.rsplit(['/', '\\']).next().unwrap());
    let replies = session(
        bin,
        &vault.0,
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "find", "arguments": {"name": "cong ty abc"}}}),
            json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "find", "arguments": {"name": "duc", "type": "person"}}}),
            json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "find", "arguments": {}}}),
        ],
    );
    assert_eq!(replies.len(), 5, "{replies:?}");

    let init = &replies[0]["result"];
    assert_eq!(init["protocolVersion"], "2025-06-18");
    assert_eq!(
        init["serverInfo"],
        json!({"name": "kg", "version": "0.1.0"})
    );
    assert_eq!(
        init["capabilities"],
        json!({"tools": {"listChanged": false}})
    );
    assert!(
        init["instructions"]
            .as_str()
            .unwrap()
            .contains("công cụ, projects / dự án")
    );

    // Mục `find` phải giống hệt bản Python, kể cả thứ tự khoá.
    let python = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/python-find-tool.json"
    ))
    .unwrap();
    let python: Value = serde_json::from_str(&python).unwrap();
    let tools = replies[1]["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].to_string(), python.to_string());

    let text = |i: usize| -> Value {
        let r = &replies[i]["result"];
        serde_json::from_str(r["content"][0]["text"].as_str().unwrap()).unwrap()
    };
    assert_eq!(replies[2]["result"]["isError"], false);
    assert_eq!(
        text(2),
        json!({"count": 1, "items": [{"id": "Khách hàng/Công ty ABC.md", "title": "Công ty ABC", "type": "client"}]})
    );
    // "duc" khớp cả hai người; lọc type bỏ note không có type.
    assert_eq!(
        text(3),
        json!({"count": 1, "items": [{"id": "Người/Nguyễn Văn Đức.md", "title": "Nguyễn Văn Đức", "type": "person"}]})
    );
    assert_eq!(replies[4]["result"]["isError"], true);
    assert_eq!(text(4), json!({"error": "find needs name (string)"}));
}

#[test]
fn hand_written_server_handshakes_and_calls_find() {
    handshake(env!("CARGO_BIN_EXE_s2-hand"));
}

#[cfg(feature = "rmcp")]
#[test]
fn rmcp_server_handshakes_and_calls_find() {
    handshake(env!("CARGO_BIN_EXE_s2-rmcp"));
}

#[test]
fn hand_written_server_keeps_python_protocol_edges() {
    let vault = Vault::new("edges");
    let bin = env!("CARGO_BIN_EXE_s2-hand");
    let mut child = Command::new(bin)
        .arg(&vault.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        let session = concat!(
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2024-11-05\"}}\n",
            "not json\n",
            "\n",
            "[{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"},{\"jsonrpc\":\"2.0\",\"method\":\"notifications/x\"}]\n",
            "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"nope/x\"}\n",
            "{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"resources/templates/list\"}\n",
        );
        stdin.write_all(session.as_bytes()).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let lines: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 5, "{lines:?}");
    assert_eq!(lines[0]["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(
        lines[1],
        json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "parse error"}})
    );
    assert_eq!(lines[2], json!([{"jsonrpc": "2.0", "id": 2, "result": {}}]));
    assert_eq!(
        lines[3]["error"],
        json!({"code": -32601, "message": "method not found: nope/x"})
    );
    assert_eq!(lines[4]["result"], json!({"resourceTemplates": []}));
}
