//! MCP stdio tự viết: mỗi dòng một bản tin JSON-RPC 2.0, đồng bộ, không async runtime.
//! Chép đúng `Server.handle` / `Server.serve` của `scripts/kg/mcp_server.py`.
//!
//!   s2-hand VAULT

use s2_mcp::{SERVER_NAME, SERVER_VERSION, call_tool, instructions, negotiate, tool_defs};
use serde_json::{Map, Value, json};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(vault) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: s2-hand VAULT");
        return ExitCode::from(2);
    };
    if !vault.is_dir() {
        eprintln!("s2-hand: not a directory: {}", vault.display());
        return ExitCode::from(2);
    }
    match serve(&vault, io::stdin().lock(), io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        // Client đóng ống đọc trước: thoát lặng, không phải lỗi của server.
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("s2-hand: {e}");
            ExitCode::FAILURE
        }
    }
}

fn serve(vault: &Path, input: impl BufRead, mut out: impl Write) -> io::Result<()> {
    for line in input.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(line) {
            Err(_) => Some(
                json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "parse error"}}),
            ),
            Ok(Value::Array(batch)) => {
                let replies: Vec<Value> = batch
                    .iter()
                    .filter_map(Value::as_object)
                    .filter_map(|m| handle(vault, m))
                    .collect();
                (!replies.is_empty()).then_some(Value::Array(replies))
            }
            Ok(Value::Object(m)) => handle(vault, &m),
            Ok(_) => None,
        };
        if let Some(r) = reply {
            serde_json::to_writer(&mut out, &r)?;
            out.write_all(b"\n")?;
            out.flush()?;
        }
    }
    Ok(())
}

fn handle(vault: &Path, m: &Map<String, Value>) -> Option<Value> {
    // Không có id (hoặc id null): notification như `notifications/initialized`, không trả lời.
    let id = m.get("id").filter(|v| !v.is_null())?.clone();
    let method = m.get("method").and_then(Value::as_str).unwrap_or_default();
    let params = m.get("params").and_then(Value::as_object);
    let ok = |result: Value| Some(json!({"jsonrpc": "2.0", "id": id, "result": result}));
    match method {
        "initialize" => {
            let want = params
                .and_then(|p| p.get("protocolVersion"))
                .and_then(Value::as_str);
            ok(json!({
                "protocolVersion": negotiate(want),
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": SERVER_NAME, "version": SERVER_VERSION},
                "instructions": instructions(vault),
            }))
        }
        "ping" => ok(json!({})),
        "tools/list" => ok(json!({"tools": tool_defs()})),
        "resources/list" => ok(json!({"resources": []})),
        "prompts/list" => ok(json!({"prompts": []})),
        "resources/templates/list" => ok(json!({"resourceTemplates": []})),
        "tools/call" => {
            let name = params
                .and_then(|p| p.get("name"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            let empty = Map::new();
            let args = params
                .and_then(|p| p.get("arguments"))
                .and_then(Value::as_object)
                .unwrap_or(&empty);
            let (text, is_error) = call_tool(vault, name, args);
            ok(json!({"content": [{"type": "text", "text": text}], "isError": is_error}))
        }
        _ => Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": -32601, "message": format!("method not found: {method}")},
        })),
    }
}
