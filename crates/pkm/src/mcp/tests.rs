//! MCP server qua `Server::serve` trên vault tạm, với tầng tool giả (danh mục tool lấy từ golden của Python, kết
//! quả tool do test đặt) để kiểm phần của vỏ: giao thức, `instructions`, refresh, thu gọn, cắt, log.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value, json};

use super::tool_layer::{ToolContext, ToolLayer, ToolSession};
use super::*;

/// Vault nhỏ có cạnh nhân quả (như `NOTES` của `test_kg.py`, phần không lỗi).
const NOTES: [(&str, &str); 6] = [
    ("Yếu tố/Doanh thu.md", "type: factor"),
    (
        "Yếu tố/Số lead.md",
        "type: factor\nincreases: [\"[[Doanh thu]]\"]",
    ),
    (
        "Yếu tố/Mệt mỏi.md",
        "type: factor\ndecreases: [\"[[Số lead]]\"]",
    ),
    ("Khách hàng/Công ty ABC.md", "type: organization"),
    ("Họp/Họp 2026-01-05.md", "type: meeting"),
    (
        "Claim/Mệt giảm doanh thu.md",
        "type: claim\nrelation: decreases\ncause: \"[[Mệt mỏi]]\"\neffect: \"[[Doanh thu]]\"\nconfidence: plausible\nsource: [\"[[Họp 2026-01-05]]\"]\nstatus: proposed\nproposed_by: llm",
    ),
];

fn write_vault(dir: &Path) {
    for (path, fm) in NOTES {
        let p = dir.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        let title = path.rsplit('/').next().unwrap().trim_end_matches(".md");
        fs::write(&p, format!("---\n{fm}\n---\n# {title}\n")).unwrap();
    }
}

/// `tools/list` của Python ở sha ghim (golden của bộ so khớp).
fn golden_tools() -> Vec<Value> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let lock = fs::read_to_string(root.join("spec/spec.lock")).unwrap();
    let sha = lock
        .lines()
        .find_map(|l| l.trim().strip_prefix("sha = \""))
        .and_then(|r| r.split('"').next())
        .unwrap();
    let text = fs::read_to_string(
        root.join("parity/golden")
            .join(sha)
            .join("mcp/tools_list.json"),
    )
    .unwrap();
    let v: Value = serde_json::from_str(&text).unwrap();
    v["tools"].as_array().unwrap().clone()
}

/// Lời gọi tool mà phiên giả nhận: tên và tham số.
type Calls = Arc<Mutex<Vec<(String, Map<String, Value>)>>>;

/// Tầng tool giả: ghi lại lời gọi, trả kết quả đặt sẵn theo tên tool.
#[derive(Default)]
struct Fake {
    opened: Mutex<Vec<ToolContext>>,
    reopened: Arc<AtomicUsize>,
    calls: Calls,
    /// `open` panic (như một lỗi ngoài dự kiến trong lần dựng đầu).
    open_panics: bool,
    /// `open` chờ tới khi đầu gửi của kênh này gửi hay bị bỏ (lần dựng đầu chạy rất lâu).
    open_waits: Mutex<Option<std::sync::mpsc::Receiver<()>>>,
}

struct FakeSession {
    reopened: Arc<AtomicUsize>,
    calls: Calls,
}

impl ToolLayer for Fake {
    fn defs(&self, hidden: bool) -> Vec<Value> {
        let mut tools = golden_tools();
        if hidden {
            tools.push(json!({"name": "system-marker"}));
        }
        tools
    }

    fn open(&self, ctx: &ToolContext) -> Result<Box<dyn ToolSession>, String> {
        assert!(!self.open_panics, "tầng tool hỏng khi mở index");
        if let Some(rx) = self.open_waits.lock().unwrap().take() {
            let _ = rx.recv();
        }
        self.opened.lock().unwrap().push(ctx.clone());
        Ok(Box::new(FakeSession {
            reopened: Arc::clone(&self.reopened),
            calls: Arc::clone(&self.calls),
        }))
    }
}

impl ToolSession for FakeSession {
    fn call(&mut self, name: &str, args: Map<String, Value>) -> Result<Value, String> {
        self.calls.lock().unwrap().push((name.to_owned(), args));
        match name {
            "find" => Ok(
                json!({"count": 1, "items": [{"id": "Khách hàng/Công ty ABC.md", "title": "Công ty ABC", "type": "organization", "aliases": [], "note": null, "x": ""}]}),
            ),
            "big" => Ok(json!({"text": "đ".repeat(100)})),
            "notready" => Ok(
                json!({"error": "ohs_not_ready", "state": "starting", "progress": null, "eta_s": null, "action": "Chờ."}),
            ),
            "boom" => Err("ValueError('hỏng')".to_owned()),
            "panic" => panic!("tool hỏng giữa chừng"),
            "neighbors" => Ok(
                json!({"error": "no note with id 'Doanh thu'; ids are exact vault paths", "did_you_mean": ["Yếu tố/Doanh thu.md"]}),
            ),
            other => Ok(json!({"error": format!("unknown tool {other}")})),
        }
    }

    fn query(&mut self, _method: &str, _kwargs: Map<String, Value>) -> Result<Value, String> {
        Err("CLI không dùng trong test này".to_owned())
    }

    fn reopen(&mut self) -> Result<(), String> {
        self.reopened.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct Session {
    _dir: tempfile::TempDir,
    vault: PathBuf,
    server: Server,
    fake: Arc<Fake>,
}

fn start(extra: &[&str], layer: bool) -> Session {
    start_fake(extra, layer, Fake::default())
}

fn start_fake(extra: &[&str], layer: bool, fake: Fake) -> Session {
    let dir = tempfile::tempdir().unwrap();
    let vault = dir.path().join("Vault thử");
    write_vault(&vault);
    let db = dir.path().join("graph.db");
    let mut argv: Vec<String> = vec![
        "pkm".into(),
        vault.to_string_lossy().into_owned(),
        "mcp".into(),
        "--db".into(),
        db.to_string_lossy().into_owned(),
    ];
    argv.extend(extra.iter().map(|s| (*s).to_owned()));
    let a = McpArgs::try_parse_from(argv).unwrap();
    let fake = Arc::new(fake);
    let server = Server::start(a, layer.then(|| Arc::clone(&fake) as Arc<dyn ToolLayer>));
    Session {
        _dir: dir,
        vault,
        server,
        fake,
    }
}

/// Gửi các dòng, trả các dòng trả lời.
fn talk(s: &Server, lines: &[&str]) -> Vec<String> {
    let input = lines.join("\n") + "\n";
    let mut out = Vec::new();
    s.serve(input.as_bytes(), &mut out).unwrap();
    String::from_utf8(out)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn one(s: &Server, line: &str) -> Value {
    let r = talk(s, &[line]);
    assert_eq!(r.len(), 1, "{r:?}");
    serde_json::from_str(&r[0]).unwrap()
}

fn call(s: &Server, tool: &str, args: Value) -> (String, bool) {
    let msg = json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {"name": tool, "arguments": args}});
    let r = one(s, &msg.to_string());
    let res = &r["result"];
    (
        res["content"][0]["text"].as_str().unwrap().to_owned(),
        res["isError"].as_bool().unwrap(),
    )
}

const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#;

#[test]
fn negotiate_echoes_known_versions_and_falls_back_to_newest() {
    assert_eq!(negotiate(Some("2024-11-05")), "2024-11-05");
    assert_eq!(negotiate(Some("2025-03-26")), "2025-03-26");
    assert_eq!(negotiate(Some("2025-11-25")), "2025-06-18");
    assert_eq!(negotiate(None), "2025-06-18");
}

#[test]
fn compact_drops_null_and_empty_strings_but_keeps_empty_lists() {
    let v =
        compact(json!({"a": null, "b": "", "c": [], "d": {"e": null, "f": 1}, "g": [null, ""]}));
    assert_eq!(v.to_string(), r#"{"c":[],"d":{"f":1},"g":[null,""]}"#);
}

#[test]
fn compact_keeps_every_key_of_ohs_not_ready() {
    let e = json!({"error": "ohs_not_ready", "state": "broken", "progress": null, "eta_s": null, "action": "x"});
    assert_eq!(compact(e.clone()), e);
}

/// Lỗi `ohs_not_ready` tới agent đủ năm khoá, đúng thứ tự, kể cả `progress`, `eta_s` là `null` (D7), và là lỗi.
#[test]
fn ohs_not_ready_reaches_the_client_with_all_five_keys() {
    let s = start(&["--refresh-every", "-1"], true);
    let (text, err) = call(&s.server, "notready", json!({}));
    assert!(err);
    assert_eq!(
        text,
        r#"{"error":"ohs_not_ready","state":"starting","progress":null,"eta_s":null,"action":"Chờ."}"#
    );
}

#[test]
fn handshake() {
    let s = start(
        &[
            "--packs",
            "business",
            "--today",
            "2026-03-01",
            "--about",
            "Test vault.",
            "--refresh-every",
            "0",
        ],
        true,
    );
    let r = one(&s.server, INIT);
    let init = &r["result"];
    assert_eq!(init["protocolVersion"], "2025-06-18");
    assert_eq!(
        init["serverInfo"],
        json!({"name": "pkm", "version": env!("CARGO_PKG_VERSION")})
    );
    assert_eq!(
        init["capabilities"],
        json!({"tools": {"listChanged": false}})
    );
    let ins = init["instructions"].as_str().unwrap();
    for want in [
        "Test vault.",
        "Today is 2026-03-01",
        "never to your own tools",
        "proposed_by",
        "increases (inverse increased_by, polarity +)",
        "Use only this server (pkm)",
        "\"Vault thử\": 6 notes.",
        "obsidian://open?vault=Vault%20th%E1%BB%AD&file=PATH",
    ] {
        assert!(ins.contains(want), "thiếu {want:?} trong:\n{ins}");
    }
    let replies = talk(
        &s.server,
        &[
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#,
            r#"{"jsonrpc":"2.0","id":3,"method":"nope/x"}"#,
            r#"{"jsonrpc":"2.0","id":4,"method":"prompts/list"}"#,
            r#"{"jsonrpc":"2.0","id":5,"method":"resources/templates/list"}"#,
        ],
    );
    assert_eq!(
        replies,
        [
            r#"{"jsonrpc": "2.0", "id": 2, "result": {}}"#,
            r#"{"jsonrpc": "2.0", "id": 3, "error": {"code": -32601, "message": "method not found: nope/x"}}"#,
            r#"{"jsonrpc": "2.0", "id": 4, "result": {"prompts": []}}"#,
            r#"{"jsonrpc": "2.0", "id": 5, "result": {"resourceTemplates": []}}"#,
        ]
    );
}

#[test]
fn old_protocol_version_echoed() {
    let s = start(&["--refresh-every", "-1"], false);
    let r = one(
        &s.server,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#,
    );
    assert_eq!(r["result"]["protocolVersion"], "2024-11-05");
}

#[test]
fn protocol_edges_like_python() {
    let s = start(&["--refresh-every", "-1"], false);
    let replies = talk(
        &s.server,
        &[
            "not json",
            "",
            r#"[{"jsonrpc":"2.0","id":1,"method":"ping"},{"jsonrpc":"2.0","method":"x"},5,{"jsonrpc":"2.0","id":2,"method":"ping"}]"#,
            r#"[{"jsonrpc":"2.0","method":"only/notification"}]"#,
            r#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#,
            r#"{"jsonrpc":"2.0","id":"a","params":{}}"#,
            r#"7"#,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#,
        ],
    );
    assert_eq!(
        replies,
        [
            r#"{"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "parse error"}}"#,
            r#"[{"jsonrpc": "2.0", "id": 1, "result": {}}, {"jsonrpc": "2.0", "id": 2, "result": {}}]"#,
            r#"{"jsonrpc": "2.0", "id": "a", "error": {"code": -32601, "message": "method not found: None"}}"#,
            r#"{"jsonrpc": "2.0", "id": 3, "result": {"tools": []}}"#,
        ]
    );
}

#[test]
fn tools_list_comes_from_the_tool_layer() {
    let s = start(&["--refresh-every", "-1"], true);
    let r = one(
        &s.server,
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
    );
    let tools = r["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        [
            "find",
            "note",
            "neighbors",
            "reach",
            "paths",
            "loops",
            "provenance",
            "select",
            "valid",
            "related",
            "more"
        ]
    );
    assert_eq!(*tools, golden_tools(), "vault không có trang hệ thống");
    // Phiên tool mở với đúng vault, DB, tên vault cho link.
    let opened = s.fake.opened.lock().unwrap().clone();
    assert_eq!(opened.len(), 1);
    assert_eq!(opened[0].vault, s.vault);
    assert_eq!(opened[0].vault_name.as_deref(), Some("Vault thử"));
    assert_eq!(opened[0].limit, None);
}

#[test]
fn tool_results_are_compacted_and_errors_flagged() {
    let s = start(&["--refresh-every", "-1"], true);
    let (text, err) = call(&s.server, "find", json!({"name": "cong ty"}));
    assert!(!err);
    assert_eq!(
        text,
        r#"{"count":1,"items":[{"id":"Khách hàng/Công ty ABC.md","title":"Công ty ABC","type":"organization","aliases":[]}]}"#
    );
    let (text, err) = call(&s.server, "neighbors", json!({"node": "Doanh thu"}));
    assert!(err);
    let r: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(r["did_you_mean"][0], "Yếu tố/Doanh thu.md");
    let (text, err) = call(&s.server, "boom", json!({}));
    assert!(err);
    assert_eq!(text, "error: ValueError('hỏng')");
    // Tên và tham số tới tầng tool nguyên vẹn; thiếu tên thì là "None" như Python.
    let r = one(
        &s.server,
        r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{}}"#,
    );
    assert_eq!(
        r["result"]["content"][0]["text"],
        r#"{"error":"unknown tool None"}"#
    );
    assert_eq!(r["result"]["isError"], true);
    let calls = s.fake.calls.lock().unwrap().clone();
    assert_eq!(calls[0].0, "find");
    assert_eq!(
        Value::Object(calls[0].1.clone()),
        json!({"name": "cong ty"})
    );
    assert_eq!(calls.last().unwrap().0, "None");
}

#[test]
fn long_results_are_cut_at_max_chars_counting_characters() {
    let s = start(&["--refresh-every", "-1", "--max-chars", "20"], true);
    let (text, err) = call(&s.server, "big", json!({}));
    assert!(!err);
    let full = format!("{{\"text\":\"{}\"}}", "đ".repeat(100));
    assert_eq!(
        text,
        format!(
            "{}... [truncated at 20 of 111 chars; narrow the query or use limit/more]",
            full.chars().take(20).collect::<String>()
        )
    );
}

#[test]
fn without_tool_layer_calls_report_index_unavailable() {
    let s = start(&["--refresh-every", "-1"], false);
    let (text, err) = call(&s.server, "find", json!({"name": "x"}));
    assert!(err);
    assert_eq!(
        text,
        r#"{"error":"index unavailable: this pkm build has no tool layer"}"#
    );
}

#[test]
fn panic_in_first_build_still_answers_tool_calls() {
    let fake = Fake {
        open_panics: true,
        ..Fake::default()
    };
    let s = start_fake(&["--refresh-every", "-1"], true, fake);
    let (text, err) = call(&s.server, "find", json!({"name": "x"}));
    assert!(err);
    assert!(
        text.starts_with(r#"{"error":"index unavailable: panic: tầng tool hỏng khi mở index"#),
        "{text}"
    );
    // Server còn trả lời.
    let r = one(&s.server, r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#);
    assert_eq!(r["result"], json!({}));
}

#[test]
fn tool_call_waits_a_bounded_time_for_the_first_build() {
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let fake = Fake {
        open_waits: Mutex::new(Some(rx)),
        ..Fake::default()
    };
    let s = start_fake(&["--refresh-every", "-1"], true, fake);
    let t0 = std::time::Instant::now();
    let (text, err) = call(&s.server, "find", json!({"name": "x"}));
    assert!(err);
    assert!(
        text.starts_with(r#"{"error":"index unavailable: the index is still being built"#),
        "{text}"
    );
    assert!(t0.elapsed() < CALL_WAIT + std::time::Duration::from_secs(5));
    // Dựng xong thì lời gọi sau chạy bình thường.
    drop(tx);
    let (text, err) = call(&s.server, "find", json!({"name": "x"}));
    assert!(!err, "{text}");
}

#[test]
fn panic_in_a_tool_is_an_error_result_and_the_server_keeps_running() {
    let s = start(&["--refresh-every", "-1"], true);
    let (text, err) = call(&s.server, "panic", json!({}));
    assert!(err);
    assert_eq!(text, "error: internal error: tool hỏng giữa chừng");
    let (text, err) = call(&s.server, "find", json!({"name": "x"}));
    assert!(!err, "{text}");
}

#[test]
fn every_tool_call_is_logged_as_jsonl() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("calls.jsonl");
    let log_s = log.to_string_lossy().into_owned();
    let s = start(&["--refresh-every", "-1", "--log", &log_s], true);
    call(&s.server, "find", json!({"name": "Công ty"}));
    call(&s.server, "neighbors", json!({"node": "Doanh thu"}));
    let text = fs::read_to_string(&log).unwrap();
    let lines: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert!(text.contains("\"args\": {\"name\": \"Công ty\"}"), "{text}");
    let keys: Vec<&str> = lines[0]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        ["t", "tool", "args", "chars", "error", "refresh_ms", "ms"]
    );
    assert_eq!(lines[0]["tool"], "find");
    assert_eq!(lines[0]["error"], Value::Null);
    assert_eq!(lines[1]["error"], true);
    assert!(lines[0]["ms"].is_f64() && lines[0]["refresh_ms"].is_f64());
    assert_eq!(lines[0]["t"].as_str().unwrap().len(), 19);
}

#[test]
fn edit_visible_on_next_call() {
    let s = start(&["--packs", "business", "--refresh-every", "0"], true);
    one(&s.server, INIT);
    let db = s.server.s.db.clone();
    let status = || -> String {
        let c = pkm_core::index::connect(&db, true).unwrap();
        c.query_row(
            "SELECT status FROM edges WHERE claim = 'Claim/Mệt giảm doanh thu.md'",
            [],
            |r| r.get(0),
        )
        .unwrap()
    };
    call(&s.server, "find", json!({"name": "x"}));
    assert_eq!(status(), "proposed");
    let before = s.fake.reopened.load(Ordering::SeqCst);
    let p = s.vault.join("Claim/Mệt giảm doanh thu.md");
    let text = fs::read_to_string(&p).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(&p, text.replace("status: proposed", "status: accepted")).unwrap();
    call(&s.server, "find", json!({"name": "x"}));
    assert_eq!(status(), "accepted", "build tăng dần trước lời gọi");
    assert_eq!(
        s.fake.reopened.load(Ordering::SeqCst),
        before + 1,
        "phiên tool mở lại graph sau khi index đổi"
    );
    // Không đổi gì: không mở lại.
    call(&s.server, "find", json!({"name": "x"}));
    assert_eq!(s.fake.reopened.load(Ordering::SeqCst), before + 1);
}

#[test]
fn packs_come_from_the_previous_build() {
    let s = start(&["--packs", "business", "--refresh-every", "-1"], false);
    one(&s.server, INIT);
    let db = s.server.s.db.clone();
    let vault = s.vault.to_string_lossy().into_owned();
    let a = McpArgs::try_parse_from([
        "pkm",
        &vault,
        "mcp",
        "--db",
        &db.to_string_lossy(),
        "--refresh-every",
        "-1",
    ])
    .unwrap();
    let again = Server::start(a, None);
    assert_eq!(again.s.packs(), ["business"]);
    let ins = again.instructions();
    assert!(ins.contains("6 notes"), "{ins}");
}

#[test]
fn missing_vault_reports_the_build_error_in_instructions() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("không có");
    let db = dir.path().join("g.db");
    let a = McpArgs::try_parse_from([
        "pkm",
        &missing.to_string_lossy(),
        "mcp",
        "--db",
        &db.to_string_lossy(),
    ])
    .unwrap();
    let s = Server::start(a, None);
    let ins = s.instructions();
    assert!(
        ins.starts_with(
            "Typed knowledge graph (pkm) of an Obsidian vault; the index is still being built. "
        ),
        "{ins}"
    );
    assert!(ins.len() > 90, "kèm lỗi build: {ins}");
}

#[test]
fn obsidian_vault_flag_names_or_disables_links() {
    let s = start(&["--refresh-every", "-1", "--obsidian-vault", ""], false);
    assert!(!s.server.instructions().contains("obsidian://"));
    let s = start(&["--refresh-every", "-1", "--obsidian-vault", "Kho"], false);
    assert!(
        s.server
            .instructions()
            .contains("obsidian://open?vault=Kho&file=PATH")
    );
}

#[test]
fn windows_drive_only_under_mnt_letter() {
    assert!(windows_drive(Path::new("/mnt/c/Users/x/vault")) || cfg!(windows));
    assert!(windows_drive(Path::new("/mnt/d")) || cfg!(windows));
    assert!(!windows_drive(Path::new("/mnt/data/vault")));
    assert!(!windows_drive(Path::new("/home/x/vault")));
}
