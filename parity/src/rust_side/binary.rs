//! Tầng MCP và CLI của bản Rust: chạy chính binary `pkm` (như `dump-python.py` chạy `Server` và `kg.cli`), thay
//! đường dẫn tuyệt đối bằng `<VAULT>`, `<DB>`.
//!
//! Binary tìm theo thứ tự: biến `PKM_BIN`, rồi cạnh binary đang chạy (`target/<profile>/pkm`, cả khi đang chạy từ
//! `target/<profile>/deps/`). CI chạy `cargo test --workspace` trước `parity check` nên binary đã dựng. Không thấy
//! binary thì hai tầng này báo "chưa có".

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};

use super::index_tables::{Scrub, TempDir, read_run};
use super::{CliOutput, McpHandshake};

/// Mã thoát của `pkm` khi lệnh cần tầng tool mà binary chưa có (`EXIT_NO_TOOL_LAYER` của `pkm`).
const EXIT_NO_TOOL_LAYER: i32 = 3;

/// Phiên bản giao thức bộ so khớp xin trong `initialize` (như `MCP_PROTOCOL` của `dump-python.py`).
const MCP_PROTOCOL: &str = "2025-06-18";

/// Binary `pkm` của bản dựng hiện tại, nếu có.
pub fn locate() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("PKM_BIN").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    let exe = std::env::current_exe().ok()?;
    let name = format!("pkm{}", std::env::consts::EXE_SUFFIX);
    exe.ancestors()
        .skip(1)
        .take(2)
        .map(|d| d.join(&name))
        .find(|p| p.is_file())
}

/// Binary `pkm` và thư mục tạm cho các index nó dựng.
pub struct Pkm {
    exe: PathBuf,
    work: TempDir,
}

impl Pkm {
    pub fn new(exe: &Path) -> Result<Self> {
        if !exe.is_file() {
            bail!("không có binary pkm ở {}", exe.display());
        }
        Ok(Pkm {
            exe: exe.to_path_buf(),
            work: TempDir::new()?,
        })
    }

    fn db(&self, name: &str) -> PathBuf {
        self.work.0.join(name)
    }

    /// Cờ đọc vault của `run` cho `build` (như `build_flags` của `dump-python.py`).
    fn build_flags(run: &Value) -> Result<Vec<String>> {
        let (packs, adapter, opts) = read_run(run)?;
        let mut f = vec!["--packs".to_owned(), packs.join(",")];
        if !adapter.is_empty() {
            f.extend(["--adapter".to_owned(), adapter]);
        }
        if !opts.aliases {
            f.push("--no-aliases".to_owned());
        }
        f.extend(["--edge-keys".to_owned(), opts.edge_keys.as_str().to_owned()]);
        if opts.body_edges {
            f.push("--body-edges".to_owned());
        }
        Ok(f)
    }

    /// Bắt tay như `dump_mcp`: index dựng sẵn, server với `--refresh-every -1`, `initialize` rồi `tools/list`.
    pub fn mcp(&self, vault: &Path, run: &Value) -> Result<McpHandshake> {
        let db = self.db("graph.db");
        let built = Command::new(&self.exe)
            .arg(vault)
            .args(["build", "--full", "--db"])
            .arg(&db)
            .args(Self::build_flags(run)?)
            .output()
            .with_context(|| format!("không chạy được {}", self.exe.display()))?;
        if !built.status.success() {
            bail!(
                "pkm build lỗi ({}): {}",
                built.status,
                String::from_utf8_lossy(&built.stderr)
            );
        }

        let (packs, adapter, opts) = read_run(run)?;
        let s = |k: &str| run.get(k).and_then(Value::as_str).filter(|x| !x.is_empty());
        let mut cmd = Command::new(&self.exe);
        cmd.arg(vault).arg("mcp").arg("--db").arg(&db);
        cmd.args(["--refresh-every", "-1"]);
        if let Some(today) = s("today") {
            cmd.args(["--today", today]);
        }
        cmd.args(["--packs", &packs.join(",")]);
        cmd.args(["--edge-keys", opts.edge_keys.as_str()]);
        cmd.args([
            "--adapter",
            if adapter.is_empty() { "none" } else { &adapter },
        ]);
        if !opts.aliases {
            cmd.arg("--no-aliases");
        }
        if opts.body_edges {
            cmd.arg("--body-edges");
        }
        if let Some(about) = s("about") {
            cmd.args(["--about", about]);
        }
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("không chạy được {} mcp", self.exe.display()))?;
        {
            let mut stdin = child.stdin.take().context("stdin của pkm mcp")?;
            let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {"protocolVersion": MCP_PROTOCOL, "capabilities": {}, "clientInfo": {"name": "pkm-parity", "version": "1"}}});
            let list = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"});
            writeln!(stdin, "{init}\n{list}")?;
        }
        let out = child.wait_with_output()?;
        if !out.status.success() {
            bail!(
                "pkm mcp thoát lỗi ({}): {}",
                out.status,
                String::from_utf8_lossy(&out.stderr)
            );
        }
        let replies: Vec<Value> = String::from_utf8(out.stdout)
            .context("stdout của pkm mcp không phải UTF-8")?
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()
            .context("pkm mcp in dòng không phải JSON")?;
        let result = |id: i64| -> Result<Value> {
            replies
                .iter()
                .find(|r| r.get("id").and_then(Value::as_i64) == Some(id))
                .and_then(|r| r.get("result"))
                .cloned()
                .with_context(|| format!("pkm mcp không trả lời request {id}"))
        };
        let scrub = Scrub::new(vault, &db);
        let Value::Object(mut init) = result(1)? else {
            bail!("kết quả initialize không phải object");
        };
        let instructions = match init.remove("instructions") {
            Some(Value::String(s)) => scrub.text(&s) + "\n",
            _ => bail!("initialize không có instructions"),
        };
        let tools = scrub_value(&scrub, result(2)?);
        let has_tools = tools
            .get("tools")
            .and_then(Value::as_array)
            .is_some_and(|t| !t.is_empty());
        Ok(McpHandshake {
            initialize: sort_keys(scrub_value(&scrub, Value::Object(init))),
            tools_list: has_tools.then(|| sort_keys(tools)),
            instructions,
        })
    }

    /// Một lệnh `pkm VAULT ... --db DB` trên index riêng của tầng CLI (các lệnh chạy theo thứ tự trên cùng index).
    /// `None` khi lệnh cần tầng tool mà binary chưa có.
    pub fn cli(&self, vault: &Path, args: &[String]) -> Result<Option<CliOutput>> {
        let db = self.db("cli.db");
        let out = Command::new(&self.exe)
            .arg(vault)
            .args(args)
            .arg("--db")
            .arg(&db)
            .stdin(Stdio::null())
            .output()
            .with_context(|| format!("không chạy được {}", self.exe.display()))?;
        let exit = out.status.code().unwrap_or(-1);
        if exit == EXIT_NO_TOOL_LAYER {
            return Ok(None);
        }
        let scrub = Scrub::new(vault, &db);
        Ok(Some(CliOutput {
            exit,
            stdout: mask_volatile(&scrub.text(&String::from_utf8_lossy(&out.stdout))),
            stderr: scrub.text(&String::from_utf8_lossy(&out.stderr)),
        }))
    }
}

fn scrub_value(scrub: &Scrub, v: Value) -> Value {
    match v {
        Value::String(s) => Value::String(scrub.text(&s)),
        Value::Array(a) => Value::Array(a.into_iter().map(|x| scrub_value(scrub, x)).collect()),
        Value::Object(m) => Value::Object(
            m.into_iter()
                .map(|(k, x)| (scrub.text(&k), scrub_value(scrub, x)))
                .collect::<Map<String, Value>>(),
        ),
        x => x,
    }
}

/// Khoá object theo thứ tự chữ, như `write_json(..., sort_keys=True)` của `dump-python.py`.
fn sort_keys(v: Value) -> Value {
    match v {
        Value::Array(a) => Value::Array(a.into_iter().map(sort_keys).collect()),
        Value::Object(m) => {
            let mut pairs: Vec<(String, Value)> = m.into_iter().collect();
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(pairs.into_iter().map(|(k, x)| (k, sort_keys(x))).collect())
        }
        x => x,
    }
}

const VOLATILE: &str = "<volatile>";

/// Giá trị đổi theo lần chạy trong đầu ra CLI thành `<volatile>`, như `mask_cli` của `dump-python.py`: `; ms {...}`
/// của `build`, và chuỗi của khoá `"built_at"`, `"ms"` trong JSON. Bộ so khớp vẫn bỏ qua các giá trị này; che đi
/// để dump của Rust giống golden từng byte.
fn mask_volatile(text: &str) -> String {
    let mut text = match text.find("; ms {") {
        Some(i) => match text[i..].find('}') {
            Some(j) => format!("{}; ms {{{VOLATILE}}}{}", &text[..i], &text[i + j + 1..]),
            None => text.to_owned(),
        },
        None => text.to_owned(),
    };
    for key in [r#""built_at": ""#, r#""ms": ""#] {
        let mut out = String::with_capacity(text.len());
        let mut rest = text.as_str();
        while let Some(i) = rest.find(key) {
            let start = i + key.len();
            out.push_str(&rest[..start]);
            // Cuối chuỗi JSON: dấu `"` không bị thoát.
            let bytes = rest.as_bytes();
            let mut k = start;
            while k < bytes.len() && bytes[k] != b'"' {
                k += if bytes[k] == b'\\' { 2 } else { 1 };
            }
            out.push_str(VOLATILE);
            rest = &rest[k.min(rest.len())..];
        }
        out.push_str(rest);
        text = out;
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_volatile_like_dump_python() {
        assert_eq!(
            mask_volatile(
                "<DB> (full): 3 note, đọc 3, xoá 0; ms {'list': 1.5}\ncạnh theo lớp: {'accepted': 2}\n"
            ),
            "<DB> (full): 3 note, đọc 3, xoá 0; ms {<volatile>}\ncạnh theo lớp: {'accepted': 2}\n"
        );
        assert_eq!(
            mask_volatile(
                r#"{"built_at": "2026-11-02T15:15:39", "ms": "{\"list\": 4.4}", "items": "ms", "x": 1}"#
            ),
            r#"{"built_at": "<volatile>", "ms": "<volatile>", "items": "ms", "x": 1}"#
        );
    }

    #[test]
    fn sort_keys_recursively() {
        let v = sort_keys(json!({"b": 1, "a": {"d": [{"z": 1, "y": 2}], "c": 0}}));
        assert_eq!(v.to_string(), r#"{"a":{"c":0,"d":[{"y":2,"z":1}]},"b":1}"#);
    }
}
