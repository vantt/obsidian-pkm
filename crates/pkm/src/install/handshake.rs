//! Bắt tay với một MCP server qua stdio như Claude Desktop: chạy `command` với `args` và `env` của mục cấu hình,
//! gửi `initialize`, chờ trả lời, gửi `notifications/initialized` rồi `tools/list` (JSON-RPC, mỗi dòng một message).
//! Đặc tả: `check` của `desktop_config.py`; khác Python là chờ trả lời `initialize` trước khi gửi tiếp (đúng thứ tự
//! của giao thức) và có hạn giờ cho cả lần bắt tay.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::config::ServerEntry;

/// Phiên bản giao thức MCP gửi trong `initialize` (như Python).
const PROTOCOL_VERSION: &str = "2025-06-18";

/// Số dòng stderr cuối của server đưa vào thông báo lỗi.
const STDERR_LINES: usize = 20;

/// Chờ server tự thoát sau khi đóng stdin trước khi kill.
const EXIT_GRACE: Duration = Duration::from_secs(3);

/// Kết quả bắt tay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handshake {
    pub server_name: String,
    pub server_version: String,
    pub tools: Vec<String>,
}

/// Chạy server của `entry`, bắt tay và lấy danh sách tool trong `timeout`. Lỗi nêu lý do và stderr cuối của server.
pub fn handshake(entry: &ServerEntry, timeout: Duration) -> Result<Handshake, String> {
    let deadline = Instant::now() + timeout;
    let mut child = Command::new(&entry.command)
        .args(&entry.args)
        .envs(entry.env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("không chạy được {}: {e}", entry.command))?;
    let mut server = Server::attach(&mut child);
    let result = server.talk(deadline);
    let stderr = server.finish(&mut child);
    result.map_err(|e| {
        if stderr.is_empty() {
            e
        } else {
            format!("{e}; stderr cuối của server:\n{stderr}")
        }
    })
}

/// Một server đang chạy: stdin để gửi, kênh nhận từng dòng stdout, phần cuối stderr.
struct Server {
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    stderr: Arc<Mutex<VecDeque<String>>>,
    readers: Vec<thread::JoinHandle<()>>,
}

impl Server {
    fn attach(child: &mut Child) -> Self {
        let (tx, lines) = mpsc::channel();
        let stdout = child.stdout.take().expect("stdout là pipe");
        let stderr_pipe = child.stderr.take().expect("stderr là pipe");
        let stderr = Arc::new(Mutex::new(VecDeque::new()));
        let tail = Arc::clone(&stderr);
        let readers = vec![
            thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            }),
            thread::spawn(move || keep_tail(stderr_pipe, &tail)),
        ];
        Self {
            stdin: child.stdin.take(),
            lines,
            stderr,
            readers,
        }
    }

    fn send(&mut self, msg: &Value) -> Result<(), String> {
        let stdin = self.stdin.as_mut().ok_or("stdin đã đóng")?;
        let mut line = msg.to_string();
        line.push('\n');
        stdin
            .write_all(line.as_bytes())
            .and_then(|()| stdin.flush())
            .map_err(|e| format!("server đóng stdin ({e})"))
    }

    /// Chờ trả lời cho `id`; bỏ qua notification, request của server và dòng không phải JSON.
    fn reply(&self, id: u64, method: &str, deadline: Instant) -> Result<Value, String> {
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let line = match self.lines.recv_timeout(left) {
                Ok(line) => line,
                Err(RecvTimeoutError::Timeout) => {
                    return Err(format!("server không trả lời `{method}` trong hạn giờ"));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(format!("server thoát trước khi trả lời `{method}`"));
                }
            };
            let Ok(msg) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if msg.get("id").and_then(Value::as_u64) != Some(id) || msg.get("method").is_some() {
                continue;
            }
            if let Some(err) = msg.get("error") {
                return Err(format!("server trả lỗi cho `{method}`: {err}"));
            }
            return msg
                .get("result")
                .cloned()
                .ok_or_else(|| format!("trả lời `{method}` không có `result`"));
        }
    }

    fn talk(&mut self, deadline: Instant) -> Result<Handshake, String> {
        self.send(&json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "pkm-mcp-install", "version": env!("CARGO_PKG_VERSION")},
            },
        }))?;
        let init = self.reply(1, "initialize", deadline)?;
        self.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))?;
        self.send(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}))?;
        let list = self.reply(2, "tools/list", deadline)?;
        let info = init.get("serverInfo");
        let text = |key: &str| {
            info.and_then(|i| i.get(key))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let tools = list
            .get("tools")
            .and_then(Value::as_array)
            .ok_or("trả lời `tools/list` không có danh sách `tools`")?
            .iter()
            .map(|t| {
                t.get("name")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or("một tool không có `name`".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Handshake {
            server_name: text("name"),
            server_version: text("version"),
            tools,
        })
    }

    /// Đóng stdin, chờ server tự thoát có giới hạn rồi kill; trả phần cuối stderr.
    fn finish(mut self, child: &mut Child) -> String {
        drop(self.stdin.take());
        let deadline = Instant::now() + EXIT_GRACE;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }
            }
        }
        // Tiến trình con của server (ví dụ OHS) có thể còn giữ pipe một lúc: chờ luồng đọc có giới hạn rồi bỏ mặc,
        // không chặn mãi.
        let deadline = Instant::now() + Duration::from_millis(500);
        while self.readers.iter().any(|r| !r.is_finished()) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        let tail = self.stderr.lock().map(|t| t.clone()).unwrap_or_default();
        Vec::from(tail).join("\n")
    }
}

/// Đọc hết `pipe`, giữ [`STDERR_LINES`] dòng cuối.
fn keep_tail(pipe: impl Read, tail: &Mutex<VecDeque<String>>) {
    for line in BufReader::new(pipe).lines() {
        let Ok(line) = line else { break };
        if let Ok(mut t) = tail.lock() {
            if t.len() == STDERR_LINES {
                t.pop_front();
            }
            t.push_back(line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;

    /// Biến môi trường làm test [`fake_mcp_server`] chạy như một MCP server (khi được chính test binary này gọi lại).
    const FAKE_ENV: &str = "PKM_TEST_FAKE_MCP";

    /// Không phải test thật: khi có [`FAKE_ENV`], đọc stdin và trả lời như một MCP server tối giản. Kiểu hành xử chọn
    /// bằng giá trị của biến: `ok`, `silent` (không trả lời), `error` (trả lỗi cho `initialize`), `exit` (thoát ngay).
    #[test]
    fn fake_mcp_server() {
        let Ok(mode) = std::env::var(FAKE_ENV) else {
            return;
        };
        eprintln!("fake server: mode {mode}");
        if mode == "exit" {
            std::process::exit(3);
        }
        let stdin = std::io::stdin();
        let mut out = std::io::stdout();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            let msg: Value = serde_json::from_str(&line).unwrap();
            let id = msg.get("id").cloned();
            let reply = match (msg["method"].as_str(), mode.as_str()) {
                (_, "silent") => None,
                (Some("initialize"), "error") => Some(json!({"jsonrpc": "2.0", "id": id,
                    "error": {"code": -32603, "message": "boom"}})),
                (Some("initialize"), _) => Some(json!({"jsonrpc": "2.0", "id": id,
                    "result": {"protocolVersion": PROTOCOL_VERSION, "capabilities": {"tools": {}},
                               "serverInfo": {"name": "fake-kg", "version": "9.9"}}})),
                (Some("tools/list"), _) => Some(json!({"jsonrpc": "2.0", "id": id,
                    "result": {"tools": [{"name": "kg_find"}, {"name": "kg_note"}]}})),
                _ => None,
            };
            if let Some(reply) = reply {
                // Một dòng log rác trước câu trả lời: client phải bỏ qua.
                writeln!(out, "log: not json").unwrap();
                writeln!(out, "{reply}").unwrap();
                out.flush().unwrap();
            }
        }
    }

    fn fake(mode: &str) -> ServerEntry {
        ServerEntry {
            command: std::env::current_exe()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            args: [
                "install::handshake::tests::fake_mcp_server",
                "--exact",
                "--nocapture",
                "--test-threads=1",
            ]
            .map(str::to_owned)
            .to_vec(),
            env: vec![(FAKE_ENV.to_owned(), mode.to_owned())],
        }
    }

    #[test]
    fn handshake_lists_tools_and_server_info() {
        let got = handshake(&fake("ok"), Duration::from_secs(30)).unwrap();
        assert_eq!(
            got,
            Handshake {
                server_name: "fake-kg".into(),
                server_version: "9.9".into(),
                tools: vec!["kg_find".into(), "kg_note".into()],
            }
        );
    }

    #[test]
    fn handshake_times_out_on_silent_server() {
        let start = Instant::now();
        let err = handshake(&fake("silent"), Duration::from_millis(1500)).unwrap_err();
        assert!(err.contains("không trả lời `initialize`"), "{err}");
        assert!(start.elapsed() < Duration::from_secs(20));
    }

    #[test]
    fn handshake_reports_server_error_and_stderr() {
        let err = handshake(&fake("error"), Duration::from_secs(30)).unwrap_err();
        assert!(err.contains("boom"), "{err}");
        assert!(err.contains("fake server: mode error"), "{err}");
    }

    #[test]
    fn handshake_reports_early_exit() {
        let err = handshake(&fake("exit"), Duration::from_secs(30)).unwrap_err();
        assert!(err.contains("thoát trước khi trả lời"), "{err}");
    }

    #[test]
    fn handshake_reports_missing_command() {
        let entry = ServerEntry {
            command: "/không/có/pkm".into(),
            args: vec![],
            env: vec![],
        };
        let err = handshake(&entry, Duration::from_secs(1)).unwrap_err();
        assert!(err.starts_with("không chạy được"), "{err}");
    }
}
