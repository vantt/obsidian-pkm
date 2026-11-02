//! Tool của MCP server qua giao thức, bản chuyển của `scripts/kg/tests/test_mcp.py`: chạy binary `pkm VAULT mcp` làm
//! tiến trình con, nói JSON-RPC theo dòng trên stdin/stdout như Claude Desktop và Claude Code.
//!
//! Vault nhỏ có đáp án tự biết (`NOTES` của `test_kg.py`, bỏ các note `Lỗi/`): hai lớp cạnh, `accepted_only`, `as_of`,
//! `paths` top-k, `loops` bắt buộc `max_len`, `provenance`, `select`, `valid`, `more`, id sai; build tăng dần trước
//! lời gọi; bộ câu đáp án của kg-solo qua tool MCP (`PKM_KG_SOLO`). `related` chưa có trong bản Rust nên chưa chuyển
//! các test của nó.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

const NOTES: [(&str, &str); 20] = [
    ("Yếu tố/Doanh thu.md", "type: factor"),
    (
        "Yếu tố/Số lead.md",
        "type: factor\nincreases: [\"[[Doanh thu]]\"]",
    ),
    (
        "Yếu tố/Ngân sách.md",
        "type: factor\naliases: [Ngân sách quảng cáo]",
    ),
    (
        "Yếu tố/Mệt mỏi.md",
        "type: factor\ndecreases: [\"[[Số lead]]\"]",
    ),
    ("Họp/Họp 2026-01-05.md", "type: meeting\ndate: 2026-01-05"),
    ("Khách hàng/Công ty ABC.md", "type: organization"),
    ("Khách hàng/Công ty XYZ.md", "type: organization"),
    ("Người/An.md", "type: person"),
    ("Khái niệm/Marketing.md", "type: concept"),
    ("Khái niệm/SEO.md", "type: concept\nup: [\"[[Marketing]]\"]"),
    ("MOC/MOC Kinh doanh.md", "type: moc"),
    (
        "Dự án/Web ABC.md",
        "type: project\nclient: [\"[[Công ty ABC]]\"]",
    ),
    (
        "Hợp đồng/HĐ ABC 2025.md",
        "type: contract\nparty: [\"[[Công ty ABC]]\"]\ncovers: [\"[[Web ABC]]\"]\nvalid_from: 2025-01-01\nvalid_to: 2025-12-31\nsuperseded_by: [\"[[HĐ ABC 2026]]\"]",
    ),
    (
        "Hợp đồng/HĐ ABC 2026.md",
        "type: contract\nparty: [\"[[Công ty ABC]]\"]\ncovers: [\"[[Web ABC]]\"]\nvalid_from: 2026-01-01",
    ),
    (
        "Claim/Lead tăng doanh thu.md",
        "type: claim\nrelation: increases\ncause: \"[[Số lead]]\"\neffect: \"[[Doanh thu]]\"\nconfidence: established\nsource: [\"[[Họp 2026-01-05#^q1]]\"]\nstatus: accepted",
    ),
    (
        "Claim/Mệt giảm doanh thu.md",
        "type: claim\nrelation: decreases\ncause: \"[[Mệt mỏi]]\"\neffect: \"[[Doanh thu]]\"\nconfidence: plausible\nsource: [\"[[Họp 2026-01-05]]\"]\nstatus: proposed\nproposed_by: llm\nextracted_by: claude-sonnet@schema_v3\nscore: 0.62",
    ),
    (
        "Claim/Doanh thu tăng ngân sách.md",
        "type: claim\nrelation: increases\ncause: \"[[Doanh thu]]\"\neffect: \"[[Ngân sách quảng cáo]]\"\nconfidence: plausible\nsource: [\"[[Họp 2026-01-05]]\"]\nstatus: accepted",
    ),
    (
        "Claim/Ngân sách tăng lead.md",
        "type: claim\nrelation: increases\ncause: \"[[Ngân sách]]\"\neffect: \"[[Số lead]]\"\nconfidence: plausible\nsource: [\"[[Họp 2026-01-05]]\"]\nstatus: accepted",
    ),
    (
        "Claim/An làm ở ABC.md",
        "type: claim\nrelation: works_at\nsubject: \"[[An]]\"\nobject: \"[[Công ty ABC]]\"\nconfidence: established\nsource: [\"[[Họp 2026-01-05]]\"]\nstatus: superseded\nvalid_from: 2024-01-01\nvalid_to: 2025-06-30\nsuperseded_by: [\"[[An làm ở XYZ]]\"]",
    ),
    (
        "Claim/An làm ở XYZ.md",
        "type: claim\nrelation: works_at\nsubject: \"[[An]]\"\nobject: \"[[Công ty XYZ]]\"\nconfidence: established\nsource: [\"[[Họp 2026-01-05]]\"]\nstatus: accepted\nvalid_from: 2025-07-01",
    ),
];

const ACC: [&str; 2] = ["accepted", "superseded"];

/// Tên thư mục vault: `url` và `instructions` mang tên này.
const VAULT_NAME: &str = "Vault thử";

/// `test_kg.write_vault`: frontmatter, tiêu đề, thân riêng của note họp (block `^q1`).
fn write_vault(root: &Path) {
    for (p, fm) in NOTES {
        let f = root.join(p);
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        let title = p.rsplit('/').next().unwrap().trim_end_matches(".md");
        let body = if p == "Họp/Họp 2026-01-05.md" {
            "\n- Lead tăng thì doanh thu tăng. ^q1\n"
        } else {
            ""
        };
        fs::write(&f, format!("---\n{fm}\n---\n# {title}\n{body}")).unwrap();
    }
}

struct Client {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    n: u64,
}

/// `pkm` không thấy biến OHS của máy chạy test (gói OHS thật, cờ cũ của bản Python).
fn pkm_command() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_pkm"));
    for k in ["PKM_OHS_DIR", "PKM_NO_OHS", "KG_OHS_CLI", "KG_OHS_NODE"] {
        c.env_remove(k);
    }
    c
}

impl Client {
    /// Server không chạy OHS (`PKM_NO_OHS=1`, như `pkm mcp install --no-ohs`).
    fn new(vault: &Path, args: &[&str]) -> Client {
        let mut cmd = pkm_command();
        cmd.env("PKM_NO_OHS", "1");
        Client::spawn(cmd, vault, args)
    }

    fn spawn(mut cmd: Command, vault: &Path, args: &[&str]) -> Client {
        let mut child = cmd
            .arg(vault)
            .arg("mcp")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("chạy được pkm");
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Client {
            child,
            stdin,
            stdout,
            n: 0,
        }
    }

    fn write(&mut self, m: &Value) {
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{m}").unwrap();
        stdin.flush().unwrap();
    }

    fn send(&mut self, method: &str, params: Option<Value>) -> Value {
        self.n += 1;
        let mut m = json!({"jsonrpc": "2.0", "method": method, "id": self.n});
        if let Some(p) = params {
            m["params"] = p;
        }
        self.write(&m);
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        let r: Value =
            serde_json::from_str(line.trim_end()).unwrap_or_else(|e| panic!("{e}: {line:?}"));
        assert_eq!(r["id"], self.n, "{r}");
        r
    }

    fn init(&mut self) -> Value {
        let r = self.send(
            "initialize",
            Some(json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}})),
        );
        self.write(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        r["result"].clone()
    }

    /// Kết quả của tool (JSON khi văn bản là object, không thì chuỗi) và `isError`.
    fn call(&mut self, tool: &str, args: Value) -> (Value, bool) {
        let r = self.send("tools/call", Some(json!({"name": tool, "arguments": args})));
        let r = &r["result"];
        let txt = r["content"][0]["text"].as_str().unwrap();
        let v = if txt.starts_with('{') {
            serde_json::from_str(txt).unwrap()
        } else {
            Value::String(txt.to_owned())
        };
        (v, r["isError"].as_bool().unwrap())
    }
}

impl Client {
    /// Đóng stdin như client thoát; server phải tự thoát 0.
    fn close(mut self) {
        drop(self.stdin.take());
        let status = self.child.wait().unwrap();
        assert!(status.success(), "{status:?}");
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        drop(self.stdin.take());
        let _ = self.child.wait();
    }
}

/// `Protocol.setUpClass`: vault nhỏ, server với `--packs business --today --about --refresh-every 0`.
struct Protocol {
    _tmp: tempfile::TempDir,
    c: Client,
    init: Value,
}

fn protocol() -> Protocol {
    let tmp = tempfile::tempdir().unwrap();
    let vault = tmp.path().join(VAULT_NAME);
    write_vault(&vault);
    let db = tmp.path().join("graph.db");
    let db = db.to_string_lossy();
    let mut c = Client::new(
        &vault,
        &[
            "--db",
            &db,
            "--packs",
            "business",
            "--today",
            "2026-03-01",
            "--about",
            "Test vault.",
            "--refresh-every",
            "0",
        ],
    );
    let init = c.init();
    Protocol { _tmp: tmp, c, init }
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_owned())
        .collect()
}

fn items(r: &Value) -> &Vec<Value> {
    r["items"].as_array().unwrap()
}

#[test]
fn tools_list() {
    let mut p = protocol();
    let r = p.c.send("tools/list", None);
    let tools = r["result"]["tools"].as_array().unwrap();
    let mut names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    names.sort_unstable();
    let mut want = [
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
        "more",
    ];
    want.sort_unstable();
    assert_eq!(names, want);
    let by = |n: &str| tools.iter().find(|t| t["name"] == n).unwrap();
    assert_eq!(
        by("loops")["inputSchema"]["required"],
        json!(["node", "max_len"])
    );
    assert!(
        by("reach")["description"]
            .as_str()
            .unwrap()
            .contains("downstream")
    );
    for k in ["status", "proposed_by", "type"] {
        assert!(
            by("select")["inputSchema"]["properties"].get(k).is_some(),
            "{k}"
        );
    }
    for t in tools {
        assert!(
            t["description"].as_str().unwrap().chars().count() > 60,
            "{}",
            t["name"]
        );
    }
}

#[test]
fn find_and_note() {
    let mut p = protocol();
    let (r, _) = p.c.call("find", json!({"name": "cong ty"}));
    let got: Vec<(String, String)> = items(&r)
        .iter()
        .map(|x| {
            (
                x["id"].as_str().unwrap().into(),
                x["type"].as_str().unwrap().into(),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            ("Khách hàng/Công ty ABC.md".into(), "organization".into()),
            ("Khách hàng/Công ty XYZ.md".into(), "organization".into())
        ]
    );
    let (n, _) =
        p.c.call("note", json!({"node": "Claim/Mệt giảm doanh thu.md"}));
    assert_eq!(
        (
            &n["type"],
            &n["status"],
            &n["proposed_by"],
            &n["claim_edge"]["layer"]
        ),
        (
            &json!("claim"),
            &json!("proposed"),
            &json!("llm"),
            &json!("proposed")
        )
    );
    assert!(
        n["body"]
            .as_str()
            .unwrap()
            .starts_with("# Mệt giảm doanh thu"),
        "{n}"
    );
    // `urllib.parse.quote("Vault thử", safe='')`
    let v = "Vault%20th%E1%BB%AD";
    assert_eq!(
        n["url"],
        format!("obsidian://open?vault={v}&file=Claim%2FM%E1%BB%87t%20gi%E1%BA%A3m%20doanh%20thu")
    );
    let ins = p.init["instructions"].as_str().unwrap();
    assert!(
        ins.contains(&format!("obsidian://open?vault={v}&file=PATH")),
        "{ins}"
    );
    let (n, _) = p.c.call(
        "note",
        json!({"node": "Họp/Họp 2026-01-05.md", "body_chars": 0}),
    );
    assert!(n.get("body").is_none(), "{n}");
    assert_eq!(n["blocks"], json!(["q1"]));
}

#[test]
fn unknown_node() {
    let mut p = protocol();
    let (r, err) = p.c.call("neighbors", json!({"node": "Doanh thu"}));
    assert!(err);
    assert_eq!(r["did_you_mean"][0], "Yếu tố/Doanh thu.md");
}

#[test]
fn neighbors_layers_window() {
    let mut p = protocol();
    let (r, _) = p.c.call(
        "neighbors",
        json!({"node": "Yếu tố/Doanh thu.md", "dir": "in", "rels": ["increases", "decreases"]}),
    );
    let got: Vec<(&str, &str, &str)> = items(&r)
        .iter()
        .map(|x| {
            (
                x["node"].as_str().unwrap(),
                x["rel"].as_str().unwrap(),
                x["layer"].as_str().unwrap(),
            )
        })
        .collect();
    assert!(
        got.contains(&("Yếu tố/Mệt mỏi.md", "decreased_by", "proposed")),
        "{got:?}"
    );
    assert!(
        got.contains(&("Yếu tố/Số lead.md", "increased_by", "accepted")),
        "{got:?}"
    );
    let prop = items(&r).iter().find(|x| x["layer"] == "proposed").unwrap();
    assert_eq!(
        (&prop["claim"], &prop["proposed_by"], &prop["type"]),
        (
            &json!("Claim/Mệt giảm doanh thu.md"),
            &json!("llm"),
            &json!("factor")
        )
    );
    let (r, _) = p.c.call(
        "neighbors",
        json!({"node": "Yếu tố/Doanh thu.md", "dir": "in", "rels": ["increases", "decreases"], "accepted_only": true}),
    );
    assert!(items(&r).iter().all(|x| x["layer"] == "accepted"), "{r}");
    assert!(!items(&r).is_empty());
    let (r, _) = p.c.call(
        "neighbors",
        json!({"node": "Người/An.md", "dir": "out", "rels": ["works_at"], "as_of": "2025-01-01"}),
    );
    let got: Vec<Value> = items(&r)
        .iter()
        .map(|x| json!([x["node"], x["status"], x["valid_to"]]))
        .collect();
    assert_eq!(
        got,
        [json!([
            "Khách hàng/Công ty ABC.md",
            "superseded",
            "2025-06-30"
        ])]
    );
    let (r, _) = p.c.call(
        "neighbors",
        json!({"node": "Khách hàng/Công ty XYZ.md", "dir": "in", "rels": ["works_at"], "as_of": "2026-03-01"}),
    );
    let got: Vec<Value> = items(&r)
        .iter()
        .map(|x| json!([x["rel"], x["node"]]))
        .collect();
    assert_eq!(got, [json!(["employs", "Người/An.md"])]);
}

#[test]
fn reach_paths_loops() {
    let mut p = protocol();
    let (r, _) = p.c.call(
        "reach",
        json!({"node": "Yếu tố/Mệt mỏi.md", "rels": ["increases", "decreases"]}),
    );
    let mut ids: Vec<&str> = items(&r)
        .iter()
        .map(|x| x["id"].as_str().unwrap())
        .collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        [
            "Yếu tố/Doanh thu.md",
            "Yếu tố/Ngân sách.md",
            "Yếu tố/Số lead.md"
        ]
    );
    let (r, _) = p.c.call(
        "reach",
        json!({"node": "Yếu tố/Doanh thu.md", "dir": "in", "rels": ["increases"], "max_depth": 1, "accepted_only": true}),
    );
    assert_eq!(r["items"], json!([{"id": "Yếu tố/Số lead.md", "depth": 1}]));
    let (r, _) = p.c.call(
        "paths",
        json!({"from": "Yếu tố/Mệt mỏi.md", "to": "Yếu tố/Doanh thu.md", "rels": ["increases", "decreases"]}),
    );
    let got: Vec<Value> = items(&r)
        .iter()
        .map(|x| json!([x["rels"], x["sign"], x["layer"]]))
        .collect();
    assert_eq!(
        got,
        [
            json!([["decreases"], "-", "proposed"]),
            json!([["decreases", "increases"], "-", "accepted"])
        ]
    );
    let (r, _) = p.c.call(
        "paths",
        json!({"from": "Yếu tố/Mệt mỏi.md", "to": "Yếu tố/Doanh thu.md", "k": 1}),
    );
    assert_eq!(
        (&r["count"], items(&r).len(), &r["truncated"]),
        (&json!(2), 1, &json!(true))
    );
    let (_, err) = p.c.call("loops", json!({"node": "Yếu tố/Doanh thu.md"}));
    assert!(err);
    let (r, _) = p.c.call(
        "loops",
        json!({"node": "Yếu tố/Doanh thu.md", "max_len": 5, "rels": ["increases", "decreases"]}),
    );
    assert_eq!(r["kinds"], json!({"reinforcing": 1}));
    assert_eq!(
        r["items"][0]["nodes"],
        json!([
            "Yếu tố/Doanh thu.md",
            "Yếu tố/Ngân sách.md",
            "Yếu tố/Số lead.md"
        ])
    );
}

#[test]
fn provenance_select_valid() {
    let mut p = protocol();
    let (r, _) = p.c.call(
        "provenance",
        json!({"claim": "Claim/Lead tăng doanh thu.md"}),
    );
    assert_eq!(
        r["sources"],
        json!([{"note": "Họp/Họp 2026-01-05.md", "target": "Họp 2026-01-05", "block": "q1"}])
    );
    let (r, _) =
        p.c.call("provenance", json!({"claim": "Claim/An làm ở ABC.md"}));
    assert_eq!(r["superseded_by"], json!(["Claim/An làm ở XYZ.md"]));
    let (r, _) = p.c.call("select", json!({"status": "proposed"}));
    assert_eq!(r["items"], json!(["Claim/Mệt giảm doanh thu.md"]));
    let (r, _) =
        p.c.call("select", json!({"proposed_by": "llm", "type": "claim"}));
    assert_eq!(r["items"], json!(["Claim/Mệt giảm doanh thu.md"]));
    let (r, _) =
        p.c.call("select", json!({"where": {"confidence": "established"}}));
    assert_eq!(r["count"], 3);
    let (_, err) = p.c.call("select", json!({}));
    assert!(err);
    let (r, _) =
        p.c.call("valid", json!({"type": "contract", "as_of": "2025-06-01"}));
    assert_eq!(r["items"], json!(["Hợp đồng/HĐ ABC 2025.md"]));
}

#[test]
fn more() {
    let mut p = protocol();
    let (r, _) = p.c.call("select", json!({"type": "claim", "limit": 2}));
    assert_eq!((items(&r).len(), &r["truncated"]), (2, &json!(true)));
    let (rest, _) = p.c.call(
        "more",
        json!({"handle": r["handle"], "offset": 2, "limit": 50}),
    );
    assert_eq!(
        Some(items(&rest).len() as u64 + 2),
        r["count"].as_u64(),
        "{rest}"
    );
}

#[test]
fn edit_visible_on_next_call() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path().join(VAULT_NAME);
    write_vault(&d);
    let mut c = Client::new(&d, &["--packs", "business", "--refresh-every", "0"]);
    c.init();
    let (r, _) = c.call("select", json!({"status": "proposed"}));
    assert_eq!(r["items"], json!(["Claim/Mệt giảm doanh thu.md"]));
    let p = d.join("Claim").join("Mệt giảm doanh thu.md");
    std::thread::sleep(std::time::Duration::from_millis(10));
    let text = fs::read_to_string(&p).unwrap();
    fs::write(&p, text.replace("status: proposed", "status: accepted")).unwrap();
    let (r, _) = c.call("select", json!({"status": "proposed"}));
    assert_eq!(r["items"], json!([]));
    let (r, _) = c.call(
        "neighbors",
        json!({"node": "Yếu tố/Mệt mỏi.md", "dir": "out", "rels": ["decreases"]}),
    );
    assert!(!items(&r).is_empty());
    assert!(items(&r).iter().all(|x| x["layer"] == "accepted"), "{r}");
    drop(c);
    // Gói lấy từ lần build trước.
    let mut c = Client::new(&d, &["--refresh-every", "-1"]);
    c.init();
    let (r, _) = c.call("valid", json!({"type": "contract", "as_of": "2026-03-01"}));
    assert_eq!(r["items"], json!(["Hợp đồng/HĐ ABC 2026.md"]));
}

// ------------------------------------------------------------------------------------------------ kg-solo

/// `run_case`: một câu fixture thành lời gọi tool MCP, trả cùng dạng `fixture_check.run`.
fn run_case(c: &mut Client, q: &Value) -> Value {
    let op = q["op"].as_str().unwrap();
    let st = match &q["statuses"] {
        Value::Array(a) if !a.is_empty() => Value::Array(a.clone()),
        _ => json!(ACC),
    };
    let mut f = json!({"statuses": st, "limit": 100_000});
    if q["rels"].as_array().is_some_and(|a| !a.is_empty()) {
        f["rels"] = q["rels"].clone();
    }
    if q["asOf"].as_str().is_some_and(|s| !s.is_empty()) {
        f["as_of"] = q["asOf"].clone();
    }
    let with = |mut a: Value, extra: &Value| {
        for (k, v) in extra.as_object().unwrap() {
            a[k] = v.clone();
        }
        a
    };
    let or = |k: &str, d: Value| match q.get(k) {
        Some(v) if !v.is_null() => v.clone(),
        _ => d,
    };
    match op {
        "neighbors" => {
            let a = with(
                json!({"node": q["node"], "dir": or("dir", json!("both"))}),
                &f,
            );
            let (r, _) = c.call("neighbors", a);
            let set: std::collections::BTreeSet<(String, String)> = items(&r)
                .iter()
                .map(|x| {
                    (
                        x["rel"].as_str().unwrap().into(),
                        x["node"].as_str().unwrap().into(),
                    )
                })
                .collect();
            Value::Array(
                set.into_iter()
                    .map(|(a, b)| json!({"rel": a, "node": b}))
                    .collect(),
            )
        }
        "reach" => {
            let mut a = with(
                json!({"node": q["node"], "dir": or("dir", json!("out"))}),
                &f,
            );
            if q["maxDepth"].as_i64().is_some_and(|d| d != 0) {
                a["max_depth"] = q["maxDepth"].clone();
            }
            let (r, _) = c.call("reach", a);
            Value::Array(items(&r).iter().map(|x| x["id"].clone()).collect())
        }
        "paths" => {
            f.as_object_mut().unwrap().remove("limit");
            let a = with(
                json!({"from": q["from"], "to": q["to"], "max_len": or("maxLen", json!(6)), "k": 100_000}),
                &f,
            );
            let (r, _) = c.call("paths", a);
            Value::Array(
                items(&r)
                    .iter()
                    .map(|p| json!({"nodes": p["nodes"], "rels": p["rels"], "sign": p["sign"]}))
                    .collect(),
            )
        }
        "provenance" => {
            let (p, _) = c.call("provenance", json!({"claim": q["claim"]}));
            let list = |k: &str| {
                p.get(k)
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
            };
            json!({
                "sources": list("sources").iter().map(|s| json!({"note": s["note"], "block": s.get("block").cloned().unwrap_or(Value::Null)})).collect::<Vec<_>>(),
                "evidence": list("evidence").iter().map(|e| json!({"rel": e["rel"], "from": e["from"]})).collect::<Vec<_>>(),
            })
        }
        "find" => {
            let mut a = json!({"name": q["name"], "limit": 100_000});
            if q["type"].as_str().is_some_and(|s| !s.is_empty()) {
                a["type"] = q["type"].clone();
            }
            let (r, _) = c.call("find", a);
            Value::Array(items(&r).iter().map(|x| x["id"].clone()).collect())
        }
        "select" => {
            let (r, _) = c.call("select", json!({"where": q["where"], "limit": 100_000}));
            r["items"].clone()
        }
        "valid" => {
            let (r, _) = c.call(
                "valid",
                json!({"type": q["type"], "as_of": q["asOf"], "limit": 100_000}),
            );
            r["items"].clone()
        }
        "chain" => {
            let start = q["start"].as_str().unwrap().to_owned();
            let mut cur = vec![start.clone()];
            for s in q["steps"].as_array().unwrap() {
                let mut next = std::collections::BTreeSet::new();
                for n in &cur {
                    let mut a = json!({"node": n, "dir": s.get("dir").cloned().unwrap_or(json!("both")),
                                       "rels": [s["rel"]], "statuses": ACC, "limit": 100_000});
                    if s["asOf"].as_str().is_some_and(|x| !x.is_empty()) {
                        a["as_of"] = s["asOf"].clone();
                    }
                    let (r, _) = c.call("neighbors", a);
                    next.extend(
                        items(&r)
                            .iter()
                            .map(|x| x["node"].as_str().unwrap().to_owned()),
                    );
                }
                cur = next.into_iter().collect();
            }
            let exclude = q.get("exclude").map(strs).unwrap_or_default();
            Value::Array(
                cur.into_iter()
                    .filter(|n| *n != start && !exclude.contains(n))
                    .map(Value::String)
                    .collect(),
            )
        }
        other => panic!("phép lạ {other}"),
    }
}

/// `FixtureOverMcp.test_answer_key_over_protocol`: 81 câu (70 graph, 11 bảng) của kg-solo qua tool MCP khớp đáp
/// án; DB ngoài vault. Cần vault kg-solo dựng sẵn (`PKM_KG_SOLO=<thư mục>`; CI so cả bộ câu qua bộ so khớp); không có
/// thì bỏ qua như Python khi thiếu node.
#[test]
fn answer_key_over_protocol() {
    let Some(dir) = std::env::var_os("PKM_KG_SOLO") else {
        eprintln!("bỏ qua: chưa đặt PKM_KG_SOLO");
        return;
    };
    let vault = PathBuf::from(dir);
    let had_kg = vault.join(".kg").exists();
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("graph.db");
    let db = db.to_string_lossy();
    let mut c = Client::new(
        &vault,
        &[
            "--db",
            &db,
            "--packs",
            "business",
            "--today",
            "2026-10-15",
            "--max-chars",
            "100000000",
        ],
    );
    c.init();
    assert_eq!(
        vault.join(".kg").exists(),
        had_kg,
        "--db ngoài vault: không ghi vào vault"
    );
    let key: Value =
        serde_json::from_str(&fs::read_to_string(vault.join(".kg-answers.json")).unwrap()).unwrap();
    use pkm_core::query::answers::{GRAPH_OPS, TABLE_OPS, norm};
    let (mut graph, mut table, mut fails) = (0, 0, Vec::new());
    for case in key["cases"].as_array().unwrap() {
        let (qs, exp) = match case.get("queries").and_then(Value::as_array) {
            Some(qs) if !qs.is_empty() => (qs.clone(), case["expect"].as_array().unwrap().clone()),
            _ => (vec![case["query"].clone()], vec![case["expect"].clone()]),
        };
        for (q, x) in qs.iter().zip(&exp) {
            let op = q["op"].as_str().unwrap();
            let is_graph = GRAPH_OPS.contains(&op);
            if !is_graph && !TABLE_OPS.contains(&op) {
                continue;
            }
            let good = if op == "cycles" {
                // Mỗi vòng mong đợi phải có trong `loops` từ node đầu của nó.
                let cycles = x.as_array().unwrap();
                let mut good = !cycles.is_empty();
                for cyc in cycles {
                    let mut a = json!({"node": cyc["nodes"][0], "max_len": q.get("maxLen").cloned().unwrap_or(json!(10)),
                                       "statuses": q.get("statuses").filter(|s| !s.is_null()).cloned().unwrap_or(json!(ACC)),
                                       "limit": 100_000});
                    if q["rels"].as_array().is_some_and(|r| !r.is_empty()) {
                        a["rels"] = q["rels"].clone();
                    }
                    let (r, _) = c.call("loops", a);
                    good &= items(&r)
                        .iter()
                        .any(|l| l["nodes"] == cyc["nodes"] && l["kind"] == cyc["kind"]);
                }
                good
            } else {
                norm(&run_case(&mut c, q)) == norm(x)
            };
            match (good, is_graph) {
                (true, true) => graph += 1,
                (true, false) => table += 1,
                (false, _) => fails.push(format!("{}#{op}", case["id"])),
            }
        }
    }
    assert_eq!(fails, Vec::<String>::new());
    assert_eq!((graph, table), (70, 11));
}

// ------------------------------------------------------------------------------------------------ related

/// `Protocol.test_related`: hạt giống cho sẵn (`hits`), lọc `types`, `accepted_only`; câu hỏi không có OHS. Python
/// không có OHS thì trả `no OHS index reachable …`; bản Rust trả `ohs_not_ready` với `state: broken` (D7).
#[test]
fn related() {
    let mut p = protocol();
    let hits = json!([{"path": "Yếu tố/Số lead.md", "score": 0.9}, {"path": "Khách hàng/Không có.md", "score": 0.5}, "Người/An.md"]);
    let (r, _) = p.c.call("related", json!({"hits": hits, "k": 5}));
    assert_eq!(r["missing"], json!(["Khách hàng/Không có.md"]));
    let seeds: Vec<&Value> = r["seeds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| &s["id"])
        .collect();
    assert_eq!(seeds, [&json!("Yếu tố/Số lead.md"), &json!("Người/An.md")]);
    let ids: Vec<&str> = r["related"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["id"].as_str().unwrap())
        .collect();
    assert!(
        ids[..3.min(ids.len())].contains(&"Yếu tố/Doanh thu.md"),
        "{ids:?}"
    );
    let top = &r["related"][0];
    assert_eq!(top["found_by"], "graph");
    let from = &top["via"][0]["from"];
    assert!(
        from == "Yếu tố/Số lead.md" || from == "Người/An.md",
        "{top}"
    );
    assert_eq!(r["seed_source"], "hits");

    let (r2, _) = p.c.call(
        "related",
        json!({"hits": hits, "k": 5, "types": ["organization"]}),
    );
    let rel = r2["related"].as_array().unwrap();
    assert!(
        !rel.is_empty() && rel.iter().all(|x| x["type"] == "organization"),
        "{r2}"
    );

    let (r3, _) = p.c.call(
        "related",
        json!({"hits": ["Yếu tố/Mệt mỏi.md"], "k": 5, "accepted_only": true}),
    );
    let rel = r3["related"].as_array().unwrap();
    assert!(!rel.is_empty());
    assert!(rel.iter().all(|x| x["layer"] == "accepted"), "{r3}");
    assert!(
        rel.iter()
            .flat_map(|x| x["via"].as_array().unwrap())
            .all(|s| s["status"] != "proposed")
    );

    let (r, err) = p.c.call("related", json!({"query": "lead"}));
    assert!(err, "{r}");
    assert_eq!(r["error"], "ohs_not_ready");
    assert_eq!(r["state"], "broken");
    assert!(r["action"].as_str().unwrap().contains("OHS"), "{r}");
}

/// `FixtureOverMcp.test_related_on_fixture`: trên kg-solo, hit là một khách hàng; note chỉ graph tìm thấy có dự án,
/// hợp đồng hay người, mỗi note có `via`. Cần `PKM_KG_SOLO`.
#[test]
fn related_on_fixture() {
    let Some(dir) = std::env::var_os("PKM_KG_SOLO") else {
        eprintln!("bỏ qua: chưa đặt PKM_KG_SOLO");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("graph.db");
    let db = db.to_string_lossy();
    let mut c = Client::new(Path::new(&dir), &["--db", &db, "--packs", "business"]);
    c.init();
    let (r, _) = c.call(
        "related",
        json!({"hits": [{"path": "Khách hàng/Công ty ABC.md", "score": 1}], "k": 10}),
    );
    let rel = r["related"].as_array().unwrap();
    let types: Vec<&str> = rel.iter().filter_map(|x| x["type"].as_str()).collect();
    assert!(
        types
            .iter()
            .any(|t| ["project", "contract", "person"].contains(t)),
        "{types:?}"
    );
    assert!(rel.iter().all(|x| !x["via"].as_array().unwrap().is_empty()));
    c.close();
}

mod mcp_support;
use mcp_support::{fake_ohs_package, fake_pids, process};

/// Một vault nhỏ và gói OHS giả; server `pkm` gắn với gói đó qua `--ohs`.
struct WithFakeOhs {
    _tmp: tempfile::TempDir,
    vault: PathBuf,
    pkg: PathBuf,
}

fn with_fake_ohs(fake: Value) -> Option<WithFakeOhs> {
    let tmp = tempfile::tempdir().unwrap();
    let pkg = fake_ohs_package(tmp.path())?;
    let vault = tmp.path().join(VAULT_NAME);
    write_vault(&vault);
    fs::write(vault.join("fake-ohs.json"), fake.to_string()).unwrap();
    Some(WithFakeOhs {
        _tmp: tmp,
        vault,
        pkg,
    })
}

impl WithFakeOhs {
    fn client(&self) -> Client {
        let pkg = self.pkg.to_string_lossy();
        Client::spawn(
            pkm_command(),
            &self.vault,
            &["--packs", "business", "--refresh-every", "0", "--ohs", &pkg],
        )
    }

    fn assert_no_fake_left(&self) {
        let pids = fake_pids(&self.vault);
        assert!(!pids.is_empty(), "server không chạy OHS");
        for pid in pids {
            assert!(
                !process::alive(pid),
                "fake-ohs {pid} còn chạy sau khi server thoát"
            );
        }
    }
}

/// `related` chỉ có câu hỏi: server chạy OHS một lần ở nền, `initialize` trả lời ngay; hạt giống lấy từ OHS
/// (`seed_source` giữ `ohs-cli` như Python); index dựng lại giữa hai lời gọi vẫn giữ OHS; hết stdin thì tắt OHS,
/// không để lại tiến trình.
#[test]
fn related_query_seeds_come_from_ohs() {
    let Some(f) = with_fake_ohs(Value::Null) else {
        return;
    };
    let mut c = f.client();
    c.init();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let r = loop {
        let (r, _) = c.call("related", json!({"query": "Yếu tố/Số lead.md", "k": 5}));
        if r.get("error").is_none() || std::time::Instant::now() > deadline {
            break r;
        }
        assert_eq!(r["error"], "ohs_not_ready", "{r}");
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    assert_eq!(r["seed_source"], "ohs-cli", "{r}");
    assert_eq!(r["seeds"][0]["id"], "Yếu tố/Số lead.md");
    // Sửa một note: lời gọi sau dựng lại index và mở lại graph, OHS vẫn gắn với phiên.
    let p = f.vault.join("Yếu tố").join("Doanh thu.md");
    fs::write(&p, "---\ntype: factor\n---\n# Doanh thu\nsửa\n").unwrap();
    let (r, _) = c.call("related", json!({"query": "Yếu tố/Doanh thu.md"}));
    assert_eq!(r["seeds"][0]["id"], "Yếu tố/Doanh thu.md", "{r}");
    assert_eq!(fake_pids(&f.vault).len(), 1, "một OHS cho cả server");
    c.close();
    f.assert_no_fake_left();
}

/// OHS chưa sẵn sàng: server vẫn bắt tay ngay, `related` chỉ có câu hỏi trả `ohs_not_ready` (`starting`) với
/// `isError`, tool khác chạy bình thường; hết stdin thì OHS bị tắt dù chưa bao giờ sẵn sàng.
#[test]
fn related_query_while_ohs_starts_is_not_ready() {
    let Some(f) = with_fake_ohs(json!({"silent": true})) else {
        return;
    };
    let mut c = f.client();
    let t0 = std::time::Instant::now();
    c.init();
    assert!(
        t0.elapsed() < std::time::Duration::from_secs(10),
        "initialize không chờ OHS"
    );
    let (r, err) = c.call("related", json!({"query": "lead"}));
    assert!(err, "{r}");
    assert_eq!(r["error"], "ohs_not_ready");
    assert_eq!(r["state"], "starting");
    let (r, err) = c.call("find", json!({"name": "doanh thu"}));
    assert!(!err && r["count"] == 4, "{r}");
    c.close();
    f.assert_no_fake_left();
}
