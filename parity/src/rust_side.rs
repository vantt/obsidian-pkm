//! Phần cắm Rust của bộ so khớp: chạy `pkm-core` (và sau này binary `pkm`) trên vault, ghi dump
//! cùng định dạng với `parity/dump-python.py`, chạy lại đúng bộ câu của dump mẫu.
//!
//! Mỗi phase lõi nối tầng của mình vào một phương thức của [`RustSide`]: trả `Some` khi Rust đã
//! làm được, `None` khi chưa. Mục trả `None` được khai trong `not_available` của manifest và báo
//! "chưa có"; bộ so khớp không bao giờ tính chúng là khớp.

mod binary;
mod index_tables;
mod note_tables;

use std::cell::OnceCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};

use crate::dump::{CliCommand, Dump, Manifest, VaultInfo, Writer};

/// Kết quả một câu đáp án hay một lời gọi tool: giá trị, hoặc tên lớp ngoại lệ Python khi bản Rust báo lỗi ở chỗ
/// Python ném (ghi vào `raised` như `dump-python.py`).
pub enum Outcome {
    Result(Value),
    Raised(String),
}

/// Kết quả bắt tay MCP: `initialize` (bỏ `instructions`), `tools/list`, khối `instructions`.
/// `tools_list` là `None` khi binary chưa có tầng tool (danh sách rỗng).
pub struct McpHandshake {
    pub initialize: Value,
    pub tools_list: Option<Value>,
    pub instructions: String,
}

/// Đầu ra một lệnh CLI.
pub struct CliOutput {
    pub exit: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Bản Rust trên một vault, với cách đọc vault ghi ở `run` của dump mẫu.
pub struct RustSide {
    vault: PathBuf,
    run: Value,
    /// Bảng của `vault` và `schema`, dựng một lần khi được hỏi lần đầu.
    note_tables: OnceCell<HashMap<&'static str, note_tables::Rows>>,
    /// Mọi bảng của index SQLite (build đủ rồi tăng dần), dựng một lần khi được hỏi lần đầu.
    index_tables: OnceCell<index_tables::Built>,
    /// Binary `pkm` cho tầng MCP và CLI; `None` khi không có binary.
    pkm: Option<binary::Pkm>,
    /// Index mở để chạy câu đáp án.
    graph: OnceCell<pkm_core::query::Graph>,
    /// Phiên tool (một phiên cho cả bộ câu, để `more` thấy handle của câu trước).
    tools: Option<pkm_core::tools::Context>,
}

impl RustSide {
    /// `pkm`: binary cho tầng MCP và CLI (`None`: hai tầng đó báo "chưa có").
    pub fn new(vault: &Path, run: &Value, pkm: Option<&Path>) -> Result<Self> {
        Ok(Self {
            vault: vault.to_path_buf(),
            run: run.clone(),
            note_tables: OnceCell::new(),
            index_tables: OnceCell::new(),
            pkm: pkm.map(binary::Pkm::new).transpose()?,
            graph: OnceCell::new(),
            tools: None,
        })
    }

    pub fn vault(&self) -> &Path {
        &self.vault
    }

    /// Cách đọc vault của dump mẫu (`packs`, `adapter`, `today`, ...).
    pub fn run(&self) -> &Value {
        &self.run
    }

    /// Các dòng của một bảng index (hoặc view `v3_deviations`) sau khi build đủ rồi build tăng dần
    /// không đổi gì, mỗi dòng một object theo thứ tự cột của `columns`.
    ///
    /// Dòng lấy từ index SQLite mà `pkm-core::index` dựng. Bảng mà `vault` và `schema` quyết định còn được dựng thẳng
    /// từ vault (`note_tables`) và phải giống hệt bảng trong DB.
    pub fn table(&self, name: &str, columns: &[String]) -> Result<Option<Vec<Map<String, Value>>>> {
        let tables = match self.note_tables.get() {
            Some(t) => t,
            None => {
                let built = note_tables::build(&self.vault, &self.run)?;
                self.note_tables.get_or_init(|| built)
            }
        };
        let direct = note_tables::rows(tables, name, columns)?;
        let Some(rows) = self.index_tables()?.tables.get(name) else {
            return Ok(direct);
        };
        let rows: Vec<Map<String, Value>> = rows
            .iter()
            .map(|r| {
                columns
                    .iter()
                    .map(|c| {
                        let v = r
                            .get(c)
                            .cloned()
                            .with_context(|| format!("bảng {name}: index không có cột {c}"))?;
                        Ok((c.clone(), v))
                    })
                    .collect::<Result<Map<String, Value>>>()
            })
            .collect::<Result<_>>()?;
        if let Some(direct) = direct {
            let key = |rs: &[Map<String, Value>]| {
                let mut v: Vec<String> = rs
                    .iter()
                    .map(|r| Value::Object(r.clone()).to_string())
                    .collect();
                v.sort();
                v
            };
            if key(&direct) != key(&rows) {
                bail!("bảng {name}: index SQLite khác bảng dựng thẳng từ vault");
            }
        }
        Ok(Some(rows))
    }

    fn index_tables(&self) -> Result<&index_tables::Built> {
        Ok(match self.index_tables.get() {
            Some(t) => t,
            None => {
                let built = index_tables::build(&self.vault, &self.run)?;
                self.index_tables.get_or_init(|| built)
            }
        })
    }

    /// Một câu của `.kg-answers.json`, chạy như `fixture_check.run` của repo đặc tả (`pkm_core::query::answers`)
    /// trên index của tầng bảng.
    pub fn answer(&self, query: &Value) -> Result<Option<Outcome>> {
        let built = self.index_tables()?;
        let g = match self.graph.get() {
            Some(g) => g,
            None => {
                let g =
                    pkm_core::query::Graph::open(&built.db).context("mở index cho câu đáp án")?;
                self.graph.get_or_init(|| g)
            }
        };
        Ok(Some(match pkm_core::query::answers::run(g, query) {
            Ok(v) => Outcome::Result(built.scrub.value(v)),
            Err(e) => Outcome::Raised(e.kind.to_owned()),
        }))
    }

    /// Một lời gọi tool qua điểm vào chung của tool (`pkm_core::tools::call`, như `Server.call` của Python, trước
    /// khi MCP thu gọn JSON). Các lời gọi chạy theo thứ tự của dump mẫu trên cùng một phiên (`more`). Tên vault cho
    /// link `obsidian://` là tên thư mục vault, như MCP server mặc định.
    pub fn tool(&mut self, name: &str, args: &Value) -> Result<Option<Outcome>> {
        let db = self.index_tables()?.db.clone();
        if self.tools.is_none() {
            let t = pkm_core::tools::Tools::open(&db).context("mở index cho tool")?;
            let name = pkm_core::tools::default_vault_name(&self.vault);
            self.tools = Some(pkm_core::tools::Context::new(t, &self.vault, name));
        }
        let ctx = self.tools.as_mut().context("phiên tool")?;
        let out = pkm_core::tools::call(ctx, name, args.clone());
        let scrub = &self.index_tables()?.scrub;
        Ok(Some(match out {
            Ok(v) => Outcome::Result(scrub.value(v)),
            Err(e) => Outcome::Raised(e.kind.to_owned()),
        }))
    }

    /// Bắt tay với `pkm VAULT mcp` trên index dựng sẵn (như `dump_mcp` của Python).
    pub fn mcp(&mut self) -> Result<Option<McpHandshake>> {
        self.pkm
            .as_ref()
            .map(|p| p.mcp(&self.vault, &self.run))
            .transpose()
    }

    /// Một lệnh `pkm VAULT ... --db DB`; `args` không gồm VAULT và `--db`. Các lệnh chạy theo thứ tự trên cùng
    /// một index. `None` khi không có binary hoặc lệnh cần tầng tool mà binary chưa có.
    pub fn cli(&mut self, args: &[String]) -> Result<Option<CliOutput>> {
        match &self.pkm {
            Some(p) => p.cli(&self.vault, args),
            None => Ok(None),
        }
    }

    /// Số đo hiệu năng cùng khoá với `perf.json` của Python.
    /// Hiện có `build_full_ms`, `build_noop_ms`.
    pub fn perf(&mut self) -> Result<Option<Value>> {
        Ok(Some(self.index_tables()?.perf.clone()))
    }
}

/// Kiểm vault thật đúng là vault của dump mẫu: cùng tên thư mục (tên vault nằm trong
/// `instructions` và link `obsidian://`) và, với vault fixture, cùng tree hash.
pub fn check_vault(vault: &Path, like: &Manifest) -> Result<()> {
    if !vault.is_dir() {
        bail!("{} không phải thư mục vault", vault.display());
    }
    let name = vault
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if name != like.vault.name {
        bail!(
            "tên thư mục vault {name:?} khác tên trong dump mẫu {:?} (tên vault nằm trong kết quả MCP); \
             dựng vault vào thư mục cùng tên",
            like.vault.name
        );
    }
    if let Some(fx) = &like.vault.fixture {
        let path = vault.join(".fixture-manifest.json");
        let text = std::fs::read_to_string(&path).map_err(|e| {
            anyhow::anyhow!(
                "dump mẫu dựng từ fixture nhưng vault không có {}: {e}",
                path.display()
            )
        })?;
        let m: Value = serde_json::from_str(&text)?;
        let got = m.get("treeHash").and_then(Value::as_str);
        if got != fx.tree_hash.as_deref() {
            bail!(
                "vault fixture khác vault của dump mẫu (treeHash {:?}, mẫu {:?}): sai tag, preset hoặc seed của fixture",
                got,
                fx.tree_hash
            );
        }
    }
    Ok(())
}

/// Dòng của dump mẫu với kết quả của bản Rust thay vào (`result` hoặc `raised`). Khoá sắp như dòng JSONL của
/// `dump-python.py` (`sort_keys=True`): lõi giữ thứ tự khoá của Python (`preserve_order`), mà danh sách không có
/// thứ tự được so theo chuỗi JSON của từng mục.
fn record(like: &Value, out: Outcome) -> Value {
    let mut rec = like.clone();
    if let Value::Object(m) = &mut rec {
        m.remove("result");
        m.remove("raised");
        match out {
            Outcome::Result(v) => m.insert("result".into(), v),
            Outcome::Raised(k) => m.insert("raised".into(), Value::String(k)),
        };
    }
    sort_keys(rec)
}

fn sort_keys(v: Value) -> Value {
    match v {
        Value::Array(a) => Value::Array(a.into_iter().map(sort_keys).collect()),
        Value::Object(m) => {
            let mut pairs: Vec<(String, Value)> = m.into_iter().collect();
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(pairs.into_iter().map(|(k, x)| (k, sort_keys(x))).collect())
        }
        v => v,
    }
}

/// Chạy bản Rust trên `vault` theo bộ câu của dump mẫu `like`, ghi dump vào `out`. Tầng MCP và CLI chạy binary
/// `pkm` của bản dựng hiện tại (`PKM_BIN`, hoặc cạnh binary đang chạy).
pub fn dump(vault: &Path, like: &Dump, out: &Path) -> Result<Manifest> {
    let pkm = binary::locate();
    if pkm.is_none() {
        eprintln!(
            "không thấy binary pkm (dựng bằng `cargo build -p pkm` hoặc đặt PKM_BIN): tầng MCP và CLI báo \"chưa có\""
        );
    }
    dump_with(vault, like, out, pkm.as_deref())
}

/// [`dump`] với binary `pkm` chỉ định (`None`: tầng MCP và CLI báo "chưa có").
pub fn dump_with(vault: &Path, like: &Dump, out: &Path, pkm: Option<&Path>) -> Result<Manifest> {
    check_vault(vault, &like.manifest)?;
    let w = Writer::create(out)?;
    let mut side = RustSide::new(vault, &like.manifest.run, pkm)?;
    let mut missing: Vec<String> = Vec::new();
    let mut tables = std::collections::BTreeMap::new();

    for (name, info) in &like.manifest.tables {
        match side.table(name, &info.columns)? {
            Some(rows) => {
                let values: Vec<Value> = rows.iter().cloned().map(Value::Object).collect();
                w.jsonl(&format!("tables/{name}.jsonl"), &values)?;
                let mut info = info.clone();
                info.rows = rows.len();
                tables.insert(name.clone(), info);
            }
            None => missing.push(format!("index:{name}")),
        }
    }

    if like.exists("answers/answers.jsonl") {
        let mut lines = Vec::new();
        let mut absent = std::collections::BTreeSet::new();
        for l in like.jsonl("answers/answers.jsonl")? {
            let op = l
                .value
                .get("op")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let query = l.value.get("query").cloned().unwrap_or(Value::Null);
            match side.answer(&query)? {
                Some(out) => lines.push(record(&l.value, out)),
                None => {
                    absent.insert(op);
                }
            }
        }
        missing.extend(absent.iter().map(|op| format!("answers:{op}")));
        lines.retain(|r| !absent.contains(r.get("op").and_then(Value::as_str).unwrap_or_default()));
        if !lines.is_empty() {
            w.jsonl("answers/answers.jsonl", &lines)?;
        }
    }

    if like.exists("tools/calls.jsonl") {
        let mut lines = Vec::new();
        let mut absent = std::collections::BTreeSet::new();
        for l in like.jsonl("tools/calls.jsonl")? {
            let tool = l
                .value
                .get("tool")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let args = l.value.get("args").cloned().unwrap_or(Value::Null);
            match side.tool(&tool, &args)? {
                Some(out) => lines.push(record(&l.value, out)),
                None => {
                    absent.insert(tool);
                }
            }
        }
        missing.extend(absent.iter().map(|t| format!("tools:{t}")));
        lines.retain(|r| {
            !absent.contains(r.get("tool").and_then(Value::as_str).unwrap_or_default())
        });
        if !lines.is_empty() {
            w.jsonl("tools/calls.jsonl", &lines)?;
        }
    }

    match side.mcp()? {
        Some(h) => {
            w.json("mcp/initialize.json", &h.initialize)?;
            match &h.tools_list {
                Some(t) => w.json("mcp/tools_list.json", t)?,
                None => missing.push("mcp:tools_list".into()),
            }
            w.text("mcp/instructions.txt", &h.instructions)?;
        }
        None => missing.push("mcp:*".into()),
    }

    if like.exists("cli/commands.json") {
        let mut done = Vec::new();
        for c in like.cli_commands()? {
            match side.cli(&c.args)? {
                Some(o) => {
                    let stdout = format!("{}.out", c.id);
                    w.text(&format!("cli/{stdout}"), &o.stdout)?;
                    let stderr = if o.exit != 0 {
                        let f = format!("{}.err", c.id);
                        w.text(&format!("cli/{f}"), &o.stderr)?;
                        Some(f)
                    } else {
                        None
                    };
                    let kind = if serde_json::from_str::<Value>(&o.stdout).is_ok()
                        && !o.stdout.trim().is_empty()
                    {
                        "json"
                    } else {
                        "text"
                    };
                    done.push(CliCommand {
                        id: c.id,
                        args: c.args,
                        exit: o.exit,
                        stdout,
                        kind: kind.into(),
                        stderr,
                    });
                }
                None => missing.push(format!("cli:{}", c.id)),
            }
        }
        if !done.is_empty() {
            w.json("cli/commands.json", &serde_json::to_value(&done)?)?;
        }
    }

    match side.perf()? {
        Some(p) => w.json("perf.json", &p)?,
        None => missing.push("perf:*".into()),
    }

    let manifest = Manifest {
        format: crate::dump::FORMAT,
        implementation: "rust".into(),
        spec_sha: crate::pinned_spec_sha().ok(),
        vault: VaultInfo {
            name: like.manifest.vault.name.clone(),
            fixture: like.manifest.vault.fixture.clone(),
        },
        run: side.run().clone(),
        tables,
        answers: None,
        calls: None,
        cli: None,
        not_available: missing,
    };
    w.manifest(&manifest)?;
    Ok(manifest)
}
