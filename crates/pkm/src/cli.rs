//! Dòng lệnh `pkm`: chia lệnh và lệnh trên vault, chép `scripts/kg/cli.py` ở sha ghim (đổi `python3 scripts/kg VAULT`
//! thành `pkm VAULT`, docs/PRD.md mục 10).
//!
//! - `pkm VAULT mcp ...`: MCP server ([`crate::mcp`]); như Python, chữ thứ hai là `mcp` thì luôn là server.
//! - `pkm mcp install ...`: cài cho Claude Desktop ([`crate::install`]).
//! - `pkm doctor ...`: chẩn đoán máy ([`crate::doctor`]).
//! - `pkm VAULT <lệnh> ...`: các lệnh của `cli.py`.
//! - `pkm --version`, `pkm --help`.

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use pkm_core::adapter;
use pkm_core::index::{self, Build, EdgeKeys, Options};
use pkm_core::validate;
use rusqlite::Connection;
use rusqlite::types::ValueRef;
use serde_json::{Map, Value, json};

use crate::doctor::{self, DoctorArgs};
use crate::install::{self, InstallArgs, SystemHooks};
use crate::mcp::{self, pyjson, tool_layer};

/// Mã thoát của lệnh cần tầng tool khi binary chưa có tầng tool (bộ so khớp coi là "chưa có"). Python không dùng mã
/// này (`argparse` thoát 2, lỗi khác 1).
pub const EXIT_NO_TOOL_LAYER: u8 = 3;

/// Chuỗi `pkm --version` in ra: phiên bản crate và sha đầy đủ của bản Python đặc tả mà binary này khớp
/// (`PKM_SPEC_SHA`, build.rs đọc từ `spec/spec.lock`).
const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (spec ",
    env!("PKM_SPEC_SHA"),
    ")"
);

const USAGE: &str = "Cách gọi:
  pkm VAULT build [--packs business] [--full] [--json]   dựng hoặc cập nhật tăng dần (mặc định VAULT/.kg/graph.db)
  pkm VAULT stats | validate | find | note | read | neighbors | reach | paths | loops | provenance | select | valid | related ...
  pkm VAULT mcp [--packs business] [--db PATH] ...       MCP server stdio (pkm VAULT mcp --help)
  pkm mcp install VAULT [--dry-run] | --check | --remove  cài MCP `pkm` cho Claude Desktop (pkm mcp install --help)
  pkm doctor [--json] [--deep]                            chẩn đoán (chỉ đọc) cấu hình, index, gói OHS
Chi tiết lệnh trên vault: pkm VAULT stats --help.";

#[derive(Parser)]
#[command(
    name = "pkm",
    version = VERSION,
    about = "Index graph của vault Obsidian, bộ tool hỏi graph và MCP server",
    after_help = USAGE,
    arg_required_else_help = true
)]
struct Top {}

const VAULT_ABOUT: &str = "Dòng lệnh của index graph. Kết quả là JSON (cho agent và để kiểm), trừ `build`, `validate` \
không --json.

  pkm VAULT build [--packs business] [--full]      dựng hoặc cập nhật tăng dần (mặc định VAULT/.kg/graph.db)
  pkm VAULT build --adapter e11 --packs capture --no-aliases --edge-keys links --body-edges   (bản B)
  pkm VAULT stats
  pkm VAULT validate [--rule R4] [--limit 5] [--json]
  pkm VAULT find \"cong ty abc\" [--type organization]
  pkm VAULT note \"Claim/X.md\"
  pkm VAULT read \"Claim/X.md\" \"Người/An.md\" [--max-chars 4000] [--offset 0]   thân note, đọc thẳng từ vault
  pkm VAULT neighbors NODE [--dir in] [--rels a,b] [--as-of 2026-01-01] [--statuses proposed] [--accepted]
  pkm VAULT reach NODE [--dir in] [--max-depth 3] ...
  pkm VAULT paths FROM TO [--max-len 6] ...
  pkm VAULT loops NODE --max-len 7 [--rels increases,decreases,prevents]
  pkm VAULT provenance CLAIM
  pkm VAULT select status=proposed proposed_by=llm
  pkm VAULT valid decision --as-of 2026-10-15
  pkm VAULT related \"câu hỏi\" [--k 15] [--types person,project] [--ohs THƯ_MỤC_GÓI]   hạt giống từ OHS
  pkm VAULT related --hit \"Claim/X.md\" --hit \"Người/An.md\" ...                     hạt giống cho sẵn, không cần OHS
  pkm VAULT mcp [--packs business] [--db PATH] ...  MCP server stdio (pkm VAULT mcp --help)
Tuỳ chọn chung: --db PATH, --refresh (build tăng dần trước khi hỏi), --limit N (độ dài trước khi rút gọn), --adapter,
--system (tính cả note hệ thống adapter ẩn). Gói, adapter và cách đọc (--no-aliases, --edge-keys, --body-edges) nhớ
trong index sau lần build đầu.
Đầu ra nêu tên note thật của vault: chỉ in ra terminal, không dán vào repo nếu là vault thật.";

const OPS: [&str; 14] = [
    "build",
    "stats",
    "validate",
    "find",
    "note",
    "read",
    "neighbors",
    "reach",
    "paths",
    "loops",
    "provenance",
    "select",
    "valid",
    "related",
];

/// `related` chỉ có câu hỏi trên CLI: chờ OHS sẵn sàng tối đa chừng này giây (mặc định), như hạn 60 giây của lần gọi
/// OHS một lần trong bản Python.
const OHS_WAIT_SECS: u64 = 60;

/// Cờ của `pkm VAULT <lệnh>`, như `argparse` của `cli.py` (cờ viết tắt được, như `argparse`).
#[derive(Parser, Debug)]
#[command(name = "pkm", about = VAULT_ABOUT, infer_long_args = true)]
struct VaultArgs {
    vault: PathBuf,
    #[arg(value_parser = OPS)]
    cmd: String,
    args: Vec<String>,
    #[arg(long)]
    db: Option<PathBuf>,
    /// Gói schema, cách nhau bằng dấu phẩy (mặc định: gói của lần build trước).
    #[arg(long)]
    packs: Option<String>,
    #[arg(long)]
    full: bool,
    #[arg(long)]
    refresh: bool,
    /// Không giải link theo alias (như Obsidian).
    #[arg(long)]
    no_aliases: bool,
    /// Mặc định schema, hoặc cách đọc của lần build trước khi không ghi cờ đọc nào.
    #[arg(long, value_parser = ["schema", "links"])]
    edge_keys: Option<String>,
    #[arg(long)]
    body_edges: bool,
    /// Cạnh riêng của vault: tên ngắn (e11 = bản B); mặc định adapter của lần build trước, none để bỏ.
    #[arg(long)]
    adapter: Option<String>,
    /// Đi qua và trả cả note hệ thống mà adapter ẩn.
    #[arg(long)]
    system: bool,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    rule: Option<String>,
    #[arg(long, allow_negative_numbers = true)]
    limit: Option<i64>,
    #[arg(long = "type")]
    note_type: Option<String>,
    #[arg(long)]
    dir: Option<String>,
    #[arg(long)]
    rels: Option<String>,
    #[arg(long)]
    as_of: Option<String>,
    #[arg(long)]
    statuses: Option<String>,
    /// Chỉ lớp accepted (và superseded).
    #[arg(long)]
    accepted: bool,
    #[arg(long, allow_negative_numbers = true)]
    max_depth: Option<i64>,
    #[arg(long, allow_negative_numbers = true)]
    max_len: Option<i64>,
    /// `related`: một hit cho sẵn (đường dẫn note), lặp lại được; có hit thì không cần OHS.
    #[arg(long = "hit")]
    hits: Vec<String>,
    /// `related`: số note chỉ graph tìm thấy (mặc định 15).
    #[arg(long, allow_negative_numbers = true)]
    k: Option<i64>,
    /// `related`: chỉ trả note thuộc các type này, cách nhau bằng dấu phẩy.
    #[arg(long)]
    types: Option<String>,
    /// `related`: thư mục gói OHS (mặc định biến PKM_OHS_DIR, rồi gói mà `pkm mcp install` cài).
    #[arg(long)]
    ohs: Option<PathBuf>,
    /// `related`: chờ OHS sẵn sàng tối đa chừng này giây.
    #[arg(long, default_value_t = OHS_WAIT_SECS)]
    ohs_wait: u64,
    /// `read`: số ký tự mỗi note (mặc định 4000, 0 là cả note).
    #[arg(long, allow_negative_numbers = true)]
    max_chars: Option<i64>,
    /// `read`: bỏ qua chừng này ký tự đầu của mỗi thân note.
    #[arg(long, allow_negative_numbers = true)]
    offset: Option<i64>,
}

#[derive(Parser)]
#[command(
    name = "pkm mcp install",
    about = "Cài MCP `pkm` trỏ vào binary pkm cho Claude Desktop"
)]
struct InstallCmd {
    #[command(flatten)]
    args: InstallArgs,
}

#[derive(Parser)]
#[command(
    name = "pkm doctor",
    about = "Chẩn đoán cấu hình Claude Desktop, index, gói OHS, đường dẫn dài trên Windows"
)]
struct DoctorCmd {
    #[command(flatten)]
    args: DoctorArgs,
}

/// Chạy `pkm` với `argv` (không gồm tên chương trình).
pub fn run(argv: Vec<OsString>) -> ExitCode {
    let word = |i: usize| argv.get(i).and_then(|a| a.to_str());
    let flag = |i: usize| word(i).is_some_and(|w| w.starts_with('-'));
    let with_name = |name: &str, rest: &[OsString]| {
        std::iter::once(OsString::from(name))
            .chain(rest.iter().cloned())
            .collect::<Vec<_>>()
    };
    if word(1) == Some("mcp") {
        return mcp::main(argv);
    }
    if word(0) == Some("mcp") && word(1) == Some("install") {
        let cmd = InstallCmd::parse_from(with_name("pkm mcp install", &argv[2..]));
        let mut out = io::stdout().lock();
        return match install::run(&cmd.args, &SystemHooks, &mut out) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => fail(&format!("mcp install: {e}")),
        };
    }
    if word(0) == Some("doctor") && (argv.len() == 1 || flag(1)) {
        let cmd = DoctorCmd::parse_from(with_name("pkm doctor", &argv[1..]));
        let mut out = io::stdout().lock();
        return match doctor::run(&cmd.args, &SystemHooks, &mut out) {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::FAILURE,
            Err(e) => fail(&format!("doctor: {e}")),
        };
    }
    if argv.is_empty() || flag(0) {
        Top::parse_from(with_name("pkm", &argv));
        return ExitCode::SUCCESS;
    }
    if word(1).is_some_and(|w| WRITING_OPS.contains(&w)) {
        return writing::run(with_name("pkm", &argv));
    }
    let a = VaultArgs::parse_from(with_name("pkm", &argv));
    match vault_command(&a) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Message(m)) => fail(&m),
        Err(Failure::Exit(code, m)) => {
            eprintln!("{m}");
            ExitCode::from(code)
        }
    }
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("pkm: {msg}");
    ExitCode::FAILURE
}

/// Lỗi của một lệnh trên vault.
enum Failure {
    /// In `pkm: <câu>` ra stderr, thoát 1 (Python: ngoại lệ, thoát 1).
    Message(String),
    /// In nguyên câu ra stderr, thoát với mã này (`sys.exit('...')` của Python là mã 1).
    Exit(u8, String),
}

impl<E: std::fmt::Display> From<E> for Failure {
    fn from(e: E) -> Self {
        Failure::Message(e.to_string())
    }
}

fn print(text: &str) -> Result<(), Failure> {
    let mut out = io::stdout().lock();
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => Ok(()),
        // Người đọc đóng ống (`| head`): không phải lỗi của lệnh.
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// `json.dump(x, ensure_ascii=False, indent=1)` rồi `print()`.
fn print_json(v: &Value) -> Result<(), Failure> {
    print(&(pyjson::dumps(v, pyjson::INDENT1) + "\n"))
}

fn vault_command(a: &VaultArgs) -> Result<(), Failure> {
    let db =
        a.db.clone()
            .unwrap_or_else(|| a.vault.join(".kg").join("graph.db"));
    let packs = a
        .packs
        .clone()
        .unwrap_or_else(|| index::previous(&db, "packs", ""));
    let packs: Vec<String> = packs
        .split(',')
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect();
    let opts = Options::pick(
        a.no_aliases,
        a.edge_keys.as_deref().and_then(EdgeKeys::parse),
        a.body_edges,
        Some(&db),
    );
    if a.cmd == "build" || a.refresh || !db.exists() {
        let spec = a
            .adapter
            .clone()
            .unwrap_or_else(|| index::previous(&db, "adapter_spec", ""));
        let adapter = adapter::load(&spec)?;
        let info = index::build(
            &a.vault,
            &db,
            &Build {
                packs,
                opts,
                full: a.full,
                adapter: adapter.as_deref(),
                ..Build::default()
            },
        )?;
        let quiet = a.json || a.cmd != "build";
        if !quiet {
            print(&info.summary(&db.to_string_lossy()))?;
        }
        if a.cmd == "build" {
            if a.json {
                let obj: Map<String, Value> = info
                    .entries()
                    .iter()
                    .map(|(k, v)| ((*k).to_owned(), Value::String(v.clone())))
                    .collect();
                print(&pyjson::dumps(&Value::Object(obj), pyjson::INDENT1))?;
            }
            return Ok(());
        }
    }
    match a.cmd.as_str() {
        "stats" => print_json(&stats(&index::connect(&db, true)?)?),
        "validate" => {
            let conn = index::connect(&db, true)?;
            let report = validate::report(&conn, a.rule.as_deref(), a.limit)?;
            if a.json {
                print_json(&report.to_json())
            } else {
                print(&report.to_text())
            }
        }
        _ => tool_command(a, &db),
    }
}

/// Một ô SQLite thành JSON như `sqlite3` của Python rồi `json.dump`.
fn cell(v: ValueRef<'_>) -> Value {
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => Value::from(i),
        ValueRef::Real(f) => Value::from(f),
        ValueRef::Text(t) | ValueRef::Blob(t) => {
            Value::String(String::from_utf8_lossy(t).into_owned())
        }
    }
}

/// Khoá dict Python thành khoá JSON (`json.dump` đổi `None` thành `"null"`, số thành chữ).
fn key(v: ValueRef<'_>) -> String {
    match cell(v) {
        Value::String(s) => s,
        Value::Null => "null".to_owned(),
        Value::Number(n) => pyjson::dumps(&Value::Number(n), pyjson::LINE),
        other => other.to_string(),
    }
}

fn rows(db: &Connection, sql: &str) -> rusqlite::Result<Vec<Vec<Value>>> {
    let mut st = db.prepare(sql)?;
    let n = st.column_count();
    st.query_map([], |r| (0..n).map(|i| r.get_ref(i).map(cell)).collect())?
        .collect()
}

/// `dict(rows)` của hai cột.
fn pairs(db: &Connection, sql: &str) -> rusqlite::Result<Value> {
    let mut st = db.prepare(sql)?;
    let mut out = Map::new();
    let mut q = st.query([])?;
    while let Some(r) = q.next()? {
        out.insert(key(r.get_ref(0)?), cell(r.get_ref(1)?));
    }
    Ok(Value::Object(out))
}

/// Lệnh `stats`.
fn stats(db: &Connection) -> rusqlite::Result<Value> {
    let nodes: i64 = db.query_row("SELECT count(*) FROM nodes", [], |r| r.get(0))?;
    let validator: Map<String, Value> = validate::counts(db)?
        .into_iter()
        .map(|(r, c)| (r, json!({"deviations": c.deviations, "notes": c.notes})))
        .collect();
    let lists = |sql: &str| -> rusqlite::Result<Value> {
        Ok(Value::Array(
            rows(db, sql)?.into_iter().map(Value::Array).collect(),
        ))
    };
    Ok(json!({
        "meta": pairs(db, "SELECT key, value FROM meta")?,
        "nodes": nodes,
        "nodes_by_type": pairs(db, "SELECT coalesce(type, '(không type)'), count(*) FROM nodes GROUP BY 1 ORDER BY 2 DESC")?,
        "edges_by_status": pairs(db, "SELECT status, count(*) FROM edges GROUP BY 1")?,
        "edges_by_origin": lists("SELECT status, origin, proposed_by, count(*) FROM edges GROUP BY 1, 2, 3 ORDER BY 1, 4 DESC")?,
        "edges_by_rel": lists("SELECT status, rel, count(*), sum(dst IS NULL) FROM edges GROUP BY 1, 2 ORDER BY 1, 3 DESC")?,
        "validator": validator,
    }))
}

/// Tham số vị trí thứ `i` (Python: `x[i]`, thiếu thì `IndexError`).
fn positional(a: &VaultArgs, i: usize) -> Result<&str, Failure> {
    a.args.get(i).map(String::as_str).ok_or_else(|| {
        Failure::Message(format!(
            "{} thiếu tham số thứ {} (xem pkm VAULT {} --help)",
            a.cmd,
            i + 1,
            a.cmd
        ))
    })
}

fn split(s: Option<&str>) -> Value {
    s.filter(|s| !s.is_empty())
        .map_or(Value::Null, |s| json!(s.split(',').collect::<Vec<_>>()))
}

/// Tham số gọi `Tools.<method>` của một lệnh hỏi graph, như `cli.py`.
fn tool_kwargs(a: &VaultArgs) -> Result<(&'static str, Map<String, Value>), Failure> {
    let filters = || -> Map<String, Value> {
        let mut f = Map::new();
        f.insert("rels".into(), split(a.rels.as_deref()));
        f.insert("as_of".into(), json!(a.as_of));
        f.insert("statuses".into(), split(a.statuses.as_deref()));
        f.insert("accepted_only".into(), json!(a.accepted));
        f.insert("system".into(), json!(a.system));
        f
    };
    let with = |mut m: Map<String, Value>, extra: Value| {
        if let Value::Object(e) = extra {
            for (k, v) in e {
                m.insert(k, v);
            }
        }
        m
    };
    let dir = |default: &str| {
        json!(
            a.dir
                .as_deref()
                .filter(|d| !d.is_empty())
                .unwrap_or(default)
        )
    };
    Ok(match a.cmd.as_str() {
        "find" => (
            "find",
            with(
                Map::new(),
                json!({"name": a.args.join(" "), "type": a.note_type}),
            ),
        ),
        "note" => ("note", with(Map::new(), json!({"node": positional(a, 0)?}))),
        "read" => {
            positional(a, 0)?;
            (
                "read",
                json_map(json!({"nodes": a.args, "max_chars": a.max_chars, "offset": a.offset})),
            )
        }
        "neighbors" => (
            "neighbors",
            with(
                json_map(json!({"node": positional(a, 0)?, "dir": dir("both")})),
                Value::Object(filters()),
            ),
        ),
        "reach" => (
            "reach",
            with(
                json_map(
                    json!({"node": positional(a, 0)?, "dir": dir("out"), "max_depth": a.max_depth}),
                ),
                Value::Object(filters()),
            ),
        ),
        "paths" => (
            "paths",
            with(
                json_map(json!({
                    "frm": positional(a, 0)?,
                    "to": positional(a, 1)?,
                    "max_len": a.max_len.filter(|n| *n != 0).unwrap_or(6),
                })),
                Value::Object(filters()),
            ),
        ),
        "loops" => {
            let Some(max_len) = a.max_len.filter(|n| *n != 0) else {
                return Err(Failure::Exit(1, "loops cần --max-len".to_owned()));
            };
            (
                "loops",
                with(
                    json_map(json!({"node": positional(a, 0)?, "max_len": max_len})),
                    Value::Object(filters()),
                ),
            )
        }
        "provenance" => ("provenance", json_map(json!({"claim": positional(a, 0)?}))),
        "select" => {
            let mut wh = Map::new();
            for kv in &a.args {
                let (k, v) = kv.split_once('=').ok_or_else(|| {
                    Failure::Message(format!("select: {kv:?} không có dạng KHOÁ=GIÁ_TRỊ"))
                })?;
                wh.insert(k.to_owned(), json!(v));
            }
            ("select", json_map(json!({"where": wh, "system": a.system})))
        }
        "related" => {
            let query = a.args.join(" ");
            let mut m = json_map(json!({
                "hits": if a.hits.is_empty() { Value::Null } else { json!(a.hits) },
                "query": if query.is_empty() { Value::Null } else { json!(query) },
                "k": a.k,
                "types": split(a.types.as_deref()),
            }));
            m.extend(filters());
            ("related", m)
        }
        "valid" => (
            "valid",
            json_map(json!({"type": positional(a, 0)?, "as_of": a.as_of, "system": a.system})),
        ),
        other => return Err(Failure::Message(format!("lệnh lạ {other}"))),
    })
}

fn json_map(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

/// Lệnh hỏi graph: gọi tầng tool như `Tools(dbpath, limit=a.limit or 20).<method>(...)`.
fn tool_command(a: &VaultArgs, db: &Path) -> Result<(), Failure> {
    let (method, kwargs) = tool_kwargs(a)?;
    let Some(layer) = tool_layer::installed() else {
        return Err(Failure::Exit(
            EXIT_NO_TOOL_LAYER,
            format!("pkm: lệnh `{}` cần tầng tool, bản pkm này chưa có", a.cmd),
        ));
    };
    // `related` chỉ có câu hỏi: chạy OHS cho lần gọi này, chờ có hạn; chưa sẵn sàng thì tool trả `ohs_not_ready`.
    let ohs = (method == "related" && kwargs["hits"].is_null() && !kwargs["query"].is_null())
        .then(|| mcp::launch_ohs(&a.vault, a.ohs.as_deref(), None))
        .flatten();
    if let Some(o) = &ohs {
        o.wait_ready(std::time::Duration::from_secs(a.ohs_wait));
    }
    let ctx = tool_layer::ToolContext {
        db: db.to_path_buf(),
        vault: mcp::abspath(&a.vault),
        vault_name: None,
        limit: Some(a.limit.filter(|n| *n != 0).unwrap_or(20)),
        ohs,
        max_chars: None,
    };
    let result = {
        let mut session = layer.open(&ctx).map_err(Failure::Message)?;
        session.query(method, kwargs).map_err(Failure::Message)?
    };
    // Tắt OHS (nếu có) trước khi in: phiên đã bỏ, đây là chỗ giữ cuối.
    drop(ctx);
    print_json(&result)?;
    // `related`, `read` báo lỗi (gồm `ohs_not_ready`): JSON ra stdout cho agent đọc, thoát 1 như lỗi của `cli.py`.
    // Lỗi của từng note trong `read` nằm trong mục của nó, không phải lỗi của lệnh.
    match result.get("error") {
        Some(e) if method == "related" || method == "read" => Err(Failure::Exit(
            1,
            format!("{method}: {}", mcp::py_str(Some(e))),
        )),
        _ => Ok(()),
    }
}

/// Lệnh tính cho việc viết bài ([`writing`]); đi riêng khỏi [`VaultArgs`].
const WRITING_OPS: [&str; 3] = ["material", "zones", "pairs"];

/// `pkm VAULT material|zones|pairs ...`: tính bằng `pkm_core` (OHS qua một tiến trình `serve --stdio`), in JSON ra
/// stdout. Phần giao diện (trang nguyên liệu, ô đã tích, ghi `uses`, màn hình vùng) thuộc plugin.
mod writing {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::process::ExitCode;
    use std::sync::Arc;
    use std::time::Duration;

    use clap::Parser;
    use pkm_core::adapter;
    use pkm_core::index::{self, Build, Options};
    use pkm_core::material;
    use pkm_core::ohs::{Ohs, OhsError, OhsState};
    use pkm_core::pairs::{self, PairOptions};
    use pkm_core::query::Graph;
    use pkm_core::zones;
    use serde_json::{Value, json};

    use super::{Failure, OHS_WAIT_SECS, WRITING_OPS, fail, print_json};
    use crate::mcp::{self, pyjson};

    const ABOUT: &str = "Tính cho việc viết bài, in JSON ra stdout (số đếm ra stderr). Cần OHS (gói qua --ohs, \
PKM_OHS_DIR hoặc thư mục cài); OHS chưa sẵn sàng thì in lỗi ohs_not_ready và thoát 1, không có đường lùi.

  pkm VAULT material \"Bài viết/X\" [--k 12] [--per-query 10] [--hits FILE]
      nguyên liệu cho bản nháp: mỗi đoạn một câu tìm OHS, rồi đi graph (trừ cạnh thư mục)
  pkm VAULT pairs [--min 0.85] [--max 60] [--per-note 2] [--min-words 40] [--write | --out FILE]
      cặp note gần nghĩa ở hai nhóm thư mục khác nhau; --write ghi VAULT/_E11/similar-pairs.json
  pkm VAULT zones [--k 15] [--min-words 8] [--max-size 60] [--min-size 5] [--write] [--apply [--top 20]]
      vùng tri thức để gợi ý bài viết; --write ghi VAULT/.e11/zones.json, --apply ghi thêm trang
      `_Xưởng/Gợi ý bài viết.md` khi trang chưa có (trang đã có giữ nguyên: nó giữ dòng chấm của người dùng)
Tuỳ chọn chung: --db PATH (index graph, mặc định VAULT/.kg/graph.db, dựng bằng pkm VAULT build), --no-refresh
(không build tăng dần trước khi tính), --ohs DIR, --ohs-wait GIÂY.
Đầu ra nêu tên note thật của vault: chỉ in ra terminal, không dán vào repo nếu là vault thật.";

    /// Cờ của `pkm VAULT material|zones|pairs`.
    #[derive(Parser, Debug)]
    #[command(name = "pkm", about = ABOUT, infer_long_args = true)]
    struct Args {
        vault: PathBuf,
        #[arg(value_parser = WRITING_OPS)]
        cmd: String,
        /// `material`: bản nháp (đường dẫn trong vault, `.md` có thể bỏ).
        args: Vec<String>,
        /// Index graph (mặc định VAULT/.kg/graph.db).
        #[arg(long)]
        db: Option<PathBuf>,
        /// Không build tăng dần index graph trước khi tính.
        #[arg(long)]
        no_refresh: bool,
        /// Thư mục gói OHS (mặc định biến PKM_OHS_DIR, rồi gói mà `pkm mcp install` cài).
        #[arg(long)]
        ohs: Option<PathBuf>,
        /// Chờ OHS sẵn sàng tối đa chừng này giây.
        #[arg(long, default_value_t = OHS_WAIT_SECS)]
        ohs_wait: u64,
        /// `material`: số note mỗi danh sách (12); `zones`: số láng giềng của mỗi note (15).
        #[arg(long)]
        k: Option<usize>,
        /// `material`: số hit mỗi câu tìm.
        #[arg(long, default_value_t = 10)]
        per_query: usize,
        /// `material`: file JSON hit OHS có sẵn (`[{path, score}]` hoặc `{results: [...]}`), thêm làm hạt giống.
        #[arg(long)]
        hits: Option<PathBuf>,
        /// `pairs`, `zones`: số từ tối thiểu của thân note (40 với `pairs`, 8 với `zones`).
        #[arg(long)]
        min_words: Option<usize>,
        /// `pairs`: cosine tối thiểu.
        #[arg(long, default_value_t = 0.85, allow_negative_numbers = true)]
        min: f64,
        /// `pairs`: số cặp tối đa.
        #[arg(long, default_value_t = 60)]
        max: usize,
        /// `pairs`: số cặp tối đa của mỗi note.
        #[arg(long, default_value_t = 2)]
        per_note: usize,
        /// `pairs`: ghi kết quả vào file này.
        #[arg(long)]
        out: Option<PathBuf>,
        /// `pairs`: ghi VAULT/_E11/similar-pairs.json; `zones`: ghi VAULT/.e11/zones.json.
        #[arg(long)]
        write: bool,
        /// `zones`: như --write, và ghi trang gợi ý khi trang chưa có.
        #[arg(long)]
        apply: bool,
        /// `zones`: số vùng trên trang.
        #[arg(long, default_value_t = 20)]
        top: usize,
        /// `zones`: vùng lớn hơn chừng này note thì chia tiếp.
        #[arg(long, default_value_t = 60)]
        max_size: usize,
        /// `zones`: bỏ vùng nhỏ hơn chừng này note.
        #[arg(long, default_value_t = 5)]
        min_size: usize,
    }

    pub(super) fn run(argv: Vec<OsString>) -> ExitCode {
        let a = Args::parse_from(argv);
        let result = match a.cmd.as_str() {
            "material" => material_cmd(&a),
            "zones" => zones_cmd(&a),
            _ => pairs_cmd(&a),
        };
        match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(Failure::Message(m)) => fail(&format!("{}: {m}", a.cmd)),
            Err(Failure::Exit(code, m)) => {
                eprintln!("{m}");
                ExitCode::from(code)
            }
        }
    }

    /// OHS cho lần chạy này, đã chờ sẵn sàng tối đa `--ohs-wait` giây. `PKM_NO_OHS=1` thì OHS ở trạng thái `broken`.
    fn launch(a: &Args) -> Arc<Ohs> {
        let ohs = mcp::launch_ohs(&a.vault, a.ohs.as_deref(), None).unwrap_or_else(|| {
            Arc::new(Ohs::unavailable(OhsState::Broken {
                action: format!(
                    "{}=1: pkm không chạy OHS. Bỏ biến này để dùng lệnh cần OHS.",
                    mcp::NO_OHS_ENV
                ),
            }))
        });
        ohs.wait_ready(Duration::from_secs(a.ohs_wait));
        ohs
    }

    /// Lỗi OHS thành JSON ra stdout (như kết quả tool: `ohs_not_ready` đủ năm khoá, hoặc `ohs_error`) và thoát 1.
    fn ohs_failure(cmd: &str, e: &OhsError) -> Failure {
        let v = e
            .not_ready_error()
            .unwrap_or_else(|| json!({"error": "ohs_error", "detail": e.to_string()}));
        if let Err(f) = print_json(&v) {
            return f;
        }
        Failure::Exit(1, format!("{cmd}: {}", mcp::py_str(v.get("error"))))
    }

    /// Index graph sẵn để đọc: build tăng dần với gói, adapter, cách đọc của lần build trước (trừ `--no-refresh`).
    fn graph(a: &Args) -> Result<Graph, Failure> {
        let db =
            a.db.clone()
                .unwrap_or_else(|| a.vault.join(".kg").join("graph.db"));
        if !db.exists() {
            return Err(Failure::Message(format!(
                "chưa có index graph ở {}: dựng trước bằng pkm VAULT build (bản B: --adapter e11 --packs capture \
                 --no-aliases --edge-keys links --body-edges)",
                db.display()
            )));
        }
        if !a.no_refresh {
            let spec = index::previous(&db, "adapter_spec", "");
            let adapter = adapter::load(&spec)?;
            let packs = index::previous(&db, "packs", "");
            index::build(
                &a.vault,
                &db,
                &Build {
                    packs: packs
                        .split(',')
                        .filter(|p| !p.is_empty())
                        .map(str::to_owned)
                        .collect(),
                    opts: Options::pick(false, None, false, Some(&db)),
                    adapter: adapter.as_deref(),
                    ..Build::default()
                },
            )?;
        }
        Ok(Graph::open(&db)?)
    }

    fn material_cmd(a: &Args) -> Result<(), Failure> {
        let [raw] = &a.args[..] else {
            return Err(Failure::Message(
                "cần đúng một bản nháp (pkm VAULT material \"Bài viết/X\")".into(),
            ));
        };
        let draft =
            material::resolve_draft(&a.vault, raw).map_err(|e| Failure::Exit(1, e.to_string()))?;
        let extra = match &a.hits {
            Some(f) => {
                let text = std::fs::read_to_string(f).map_err(|e| {
                    Failure::Message(format!("không đọc được {}: {e}", f.display()))
                })?;
                let v: Value = serde_json::from_str(&text).map_err(|e| {
                    Failure::Message(format!("{} không phải JSON: {e}", f.display()))
                })?;
                material::hits_from_value(&v).map_err(Failure::Message)?
            }
            None => Vec::new(),
        };
        let g = graph(a)?;
        let o = material::Options {
            k: a.k.unwrap_or(material::Options::default().k),
            per_query: a.per_query,
        };
        // OHS chỉ chạy khi bản nháp có chữ để tìm.
        let needs_ohs = !material::draft_queries(&a.vault, &draft)
            .map_err(|e| Failure::Message(e.to_string()))?
            .is_empty();
        let ohs = needs_ohs.then(|| launch(a));
        let search = |q: &str, limit: usize| match &ohs {
            Some(o) => material::ohs_search(o, q, limit),
            None => Ok(Vec::new()),
        };
        let r = material::gather(&g, &a.vault, &draft, &o, &extra, &search);
        drop(ohs);
        match r {
            Ok(v) => print_json(&v),
            Err(material::Error::Ohs(e)) => Err(ohs_failure("material", &e)),
            Err(e) => Err(Failure::Message(e.to_string())),
        }
    }

    /// Vector note từ OHS, đã lọc; OHS tắt ngay sau khi lấy xong.
    fn note_vectors(a: &Args, cmd: &str, min_words: usize) -> Result<pairs::NoteVectors, Failure> {
        let ohs = launch(a);
        let r = pairs::fetch(&ohs, &a.vault, min_words);
        drop(ohs);
        match r {
            Ok(v) => Ok(v),
            Err(pairs::FetchError::Ohs(e)) => Err(ohs_failure(cmd, &e)),
            Err(pairs::FetchError::Other(m)) => Err(Failure::Message(m)),
        }
    }

    /// Ghi `text` vào `path`, tạo thư mục cha.
    fn write_file(path: &Path, text: &str) -> Result<(), Failure> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, text)
            .map_err(|e| Failure::Message(format!("không ghi được {}: {e}", path.display())))
    }

    fn pairs_cmd(a: &Args) -> Result<(), Failure> {
        let o = PairOptions {
            min: a.min,
            max: a.max,
            per_note: a.per_note,
            min_words: a.min_words.unwrap_or(PairOptions::default().min_words),
        };
        let v = note_vectors(a, "pairs", o.min_words)?;
        let p = pairs::similar_pairs(&v, &o);
        let picked = p.picked.len();
        let list = Value::Array(p.picked);
        let out = a.out.clone().or_else(|| {
            a.write
                .then(|| a.vault.join("_E11").join("similar-pairs.json"))
        });
        if let Some(path) = &out {
            write_file(path, &pyjson::dumps(&list, pyjson::INDENT1))?;
        }
        eprintln!(
            "notes {}, pairs >= {}: {}, picked {picked}{}",
            p.notes,
            o.min,
            p.candidates,
            out.map(|p| format!(" -> {}", p.display()))
                .unwrap_or_default()
        );
        print_json(&list)
    }

    fn zones_cmd(a: &Args) -> Result<(), Failure> {
        let g = graph(a)?;
        let v = note_vectors(a, "zones", a.min_words.unwrap_or(8))?;
        let knn = pairs::knn(&v, a.k.unwrap_or(15));
        let o = zones::Options {
            max_size: a.max_size,
            min_size: a.min_size,
        };
        let z = zones::zones(&a.vault, &g, &knn, &o)?;
        let stamp = chrono::Local::now().date_naive().to_string();
        let file = json!({"generated": stamp, "zones": z.zones});
        if a.write || a.apply {
            write_file(
                &a.vault.join(zones::OUT),
                &pyjson::dumps(&file, pyjson::INDENT1),
            )?;
        }
        let s = &z.stats;
        eprintln!(
            "notes in graph {} (knn {}, hand edges {}), zones {}, covered {}, size min/median/max {}/{}/{}, \
             with posts {}, single-folder {}",
            s["notes"],
            s["knn"],
            s["hand_edges"],
            s["zones"],
            s["covered"],
            s["size_min"],
            s["size_median"],
            s["size_max"],
            s["with_posts"],
            s["single_folder"]
        );
        if a.apply {
            let abs = mcp::abspath(&a.vault);
            let name = abs
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let text = zones::page(&z.zones, &name, &stamp, a.top);
            if zones::write_page_if_absent(&a.vault, &text)? {
                eprintln!("wrote {}", zones::PAGE);
            } else {
                eprintln!(
                    "giữ nguyên {} (đã có; xoá trang nếu muốn dựng lại)",
                    zones::PAGE
                );
            }
        }
        let mut out = file;
        out["stats"] = z.stats;
        print_json(&out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> VaultArgs {
        VaultArgs::try_parse_from(std::iter::once("pkm").chain(args.iter().copied())).unwrap()
    }

    #[test]
    fn options_mix_with_positionals_and_abbreviate_like_argparse() {
        let a = parse(&[
            "v", "find", "Chất", "--type", "claim", "lượng", "--lim", "3",
        ]);
        assert_eq!(a.args, ["Chất", "lượng"]);
        assert_eq!(a.note_type.as_deref(), Some("claim"));
        assert_eq!(a.limit, Some(3));
        let (m, kw) = tool_kwargs(&a).unwrap_or_else(|_| panic!("find"));
        assert_eq!(m, "find");
        assert_eq!(
            Value::Object(kw),
            json!({"name": "Chất lượng", "type": "claim"})
        );
    }

    #[test]
    fn graph_commands_map_to_python_tool_arguments() {
        let a = parse(&[
            "v",
            "neighbors",
            "A.md",
            "--accepted",
            "--as-of",
            "2026-10-15",
            "--rels",
            "a,b",
        ]);
        let (_, kw) = tool_kwargs(&a).unwrap_or_else(|_| panic!("neighbors"));
        assert_eq!(
            Value::Object(kw),
            json!({"node": "A.md", "dir": "both", "rels": ["a", "b"], "as_of": "2026-10-15",
                   "statuses": null, "accepted_only": true, "system": false})
        );
        let a = parse(&[
            "v",
            "paths",
            "A.md",
            "B.md",
            "--statuses",
            "accepted,proposed",
        ]);
        let (_, kw) = tool_kwargs(&a).unwrap_or_else(|_| panic!("paths"));
        assert_eq!(kw["frm"], "A.md");
        assert_eq!(kw["max_len"], 6);
        assert_eq!(kw["statuses"], json!(["accepted", "proposed"]));
        let a = parse(&["v", "select", "status=proposed", "x=a=b", "--system"]);
        let (_, kw) = tool_kwargs(&a).unwrap_or_else(|_| panic!("select"));
        assert_eq!(
            Value::Object(kw),
            json!({"where": {"status": "proposed", "x": "a=b"}, "system": true})
        );
    }

    #[test]
    fn loops_without_max_len_exits_like_python() {
        let a = parse(&["v", "loops", "A.md"]);
        match tool_kwargs(&a) {
            Err(Failure::Exit(1, m)) => assert_eq!(m, "loops cần --max-len"),
            _ => panic!("loops không có --max-len phải thoát 1"),
        }
    }

    #[test]
    fn unknown_command_is_rejected() {
        assert!(VaultArgs::try_parse_from(["pkm", "v", "nope"]).is_err());
    }
}
