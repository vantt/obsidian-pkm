//! Chỗ nối giữa vỏ `pkm` (MCP server, CLI) và tầng tool của `pkm-core` (`query`, `tools`, `related`).
//!
//! Vỏ lo giao thức, dựng index, refresh, thu gọn và cắt kết quả, log; tầng tool lo mọi thứ bên trong một lời gọi.
//! Ranh giới chép đúng ranh giới của bản Python:
//!
//! - [`ToolLayer::defs`] là `tool_defs(hidden)` của `scripts/kg/mcp_server.py` (tên, mô tả, `inputSchema`).
//! - [`ToolSession::call`] là `Server.call(name, args)` (tham số theo tên MCP, gồm kiểm id note, `url`, `body`,
//!   hạt OHS của `related`), trả kết quả **trước** khi MCP thu gọn (`compact`) và cắt (`--max-chars`).
//! - [`ToolSession::query`] là `Tools.<method>(**kwargs)` mà `scripts/kg/cli.py` gọi (tham số theo tên Python).
//!
//! [`installed`] trả bản bọc `pkm_core::tools`. Vỏ vẫn chạy được khi không có tầng tool (`None`): MCP server bắt
//! tay (`initialize`, `instructions`) nhưng `tools/list` rỗng và `tools/call` báo lỗi; lệnh CLI cần tool thoát với
//! mã [`crate::cli::EXIT_NO_TOOL_LAYER`]. Test của vỏ dùng đường này với tầng tool giả.

use std::path::PathBuf;
use std::sync::Arc;

use pkm_core::ohs::Ohs;
use pkm_core::query::Graph;
use pkm_core::tools;
use serde_json::{Map, Value};

/// Những gì một phiên tool cần biết về vault và index.
#[derive(Clone)]
pub struct ToolContext {
    /// Index SQLite (đã dựng).
    pub db: PathBuf,
    /// Thư mục vault, tuyệt đối (`note` đọc thân note từ đây).
    pub vault: PathBuf,
    /// Tên vault trong Obsidian cho link `obsidian://open` (`url` của `note`); `None` là không thêm link.
    pub vault_name: Option<String>,
    /// Số mục trước khi rút gọn khi lời gọi không ghi `limit` (`Tools(limit=...)`; CLI: `--limit` hoặc 20).
    pub limit: Option<i64>,
    /// OHS cho `search` và `related` gieo bằng câu hỏi hay note (một tiến trình cho cả server); `None`: hai tool đó
    /// trả `ohs_not_ready`.
    pub ohs: Option<Arc<Ohs>>,
    /// Bộ cắt `--max-chars` của MCP: `read` tự cắt thân note cho vừa (`None`: CLI, không cắt).
    pub max_chars: Option<usize>,
}

/// Tầng tool: danh mục tool và cách mở một phiên trên một index.
pub trait ToolLayer: Send + Sync {
    /// Các mục của `tools/list` theo thứ tự, mỗi mục `{"name", "description", "inputSchema"}` đúng từng chữ như
    /// Python. `hidden`: vault có trang hệ thống (adapter ẩn), thêm tham số `system`.
    fn defs(&self, hidden: bool) -> Vec<Value>;

    /// Mở một phiên trên `ctx.db` (như `Tools(Graph(db))`).
    fn open(&self, ctx: &ToolContext) -> Result<Box<dyn ToolSession>, String>;
}

/// Một phiên tool. Giữ trạng thái giữa các lời gọi (handle của `more`).
pub trait ToolSession: Send {
    /// `Server.call`: `name` như client gửi (`"None"` khi thiếu), `args` là `arguments` của `tools/call`. Kết quả
    /// lỗi nghiệp vụ là object có khoá `error`; `Err` là lỗi ngoài dự kiến (Python: ngoại lệ), MCP trả
    /// `error: <chuỗi>` với `isError`.
    fn call(&mut self, name: &str, args: Map<String, Value>) -> Result<Value, String>;

    /// `Tools.<method>(**kwargs)` cho CLI: `method` là `find`, `note`, `neighbors`, `reach`, `paths`, `loops`,
    /// `provenance`, `select`, `valid`; `kwargs` theo tên tham số Python (`frm`, `max_len`, `where`, ...), `null`
    /// là `None`.
    fn query(&mut self, method: &str, kwargs: Map<String, Value>) -> Result<Value, String>;

    /// Index vừa dựng lại có thay đổi: mở lại graph, giữ handle của `more` (Python thay `tools.g`).
    fn reopen(&mut self) -> Result<(), String>;
}

/// Tầng tool có trong binary này: bản bọc `pkm_core::tools` (danh mục `tools::definitions`, điểm vào chung
/// `tools::call`, các phương thức có kiểu của `tools::Tools` cho CLI).
pub fn installed() -> Option<std::sync::Arc<dyn ToolLayer>> {
    Some(std::sync::Arc::new(CoreLayer))
}

/// Tầng tool của `pkm-core`.
struct CoreLayer;

/// Một phiên trên index: `tools::Context` (bộ tool, thư mục vault, tên vault cho link) và đường dẫn index để mở lại.
struct CoreSession {
    ctx: tools::Context,
    db: PathBuf,
}

impl ToolLayer for CoreLayer {
    fn defs(&self, hidden: bool) -> Vec<Value> {
        tools::definitions(hidden)
            .iter()
            .map(tools::ToolDef::to_json)
            .collect()
    }

    fn open(&self, ctx: &ToolContext) -> Result<Box<dyn ToolSession>, String> {
        let g = Graph::open(&ctx.db).map_err(|e| e.to_string())?;
        // `Tools(g)` của MCP (mặc định 20), `Tools(db, limit=a.limit or 20)` của CLI; 0 là mặc định.
        let t = tools::Tools::with_limit(g, ctx.limit.unwrap_or(0));
        Ok(Box::new(CoreSession {
            ctx: {
                let mut c = tools::Context::new(t, &ctx.vault, ctx.vault_name.clone());
                c.set_ohs(ctx.ohs.clone());
                c.set_reply_budget(ctx.max_chars);
                c
            },
            db: ctx.db.clone(),
        }))
    }
}

impl ToolSession for CoreSession {
    fn call(&mut self, name: &str, args: Map<String, Value>) -> Result<Value, String> {
        // MCP ghi `error: {e!r}` khi `Server.call` ném ngoại lệ.
        tools::call(&mut self.ctx, name, Value::Object(args)).map_err(|e| e.repr())
    }

    fn query(&mut self, method: &str, kwargs: Map<String, Value>) -> Result<Value, String> {
        let kw = Kwargs(kwargs);
        let t = self.ctx.tools_mut();
        let out = match method {
            "find" => t.find(&kw.str("name")?, kw.opt_str("type")?.as_deref(), None),
            "note" => t.note(&kw.str("node")?),
            "neighbors" => t.neighbors(&kw.str("node")?, &kw.str("dir")?, &kw.edge()?, None),
            "reach" => t.reach(
                &kw.str("node")?,
                &kw.str("dir")?,
                kw.opt_int("max_depth")?,
                &kw.edge()?,
                None,
            ),
            "paths" => t.paths(
                &kw.str("frm")?,
                &kw.str("to")?,
                kw.opt_int("max_len")?.unwrap_or(6),
                &kw.edge()?,
                None,
            ),
            "loops" => {
                let max_len = kw
                    .opt_int("max_len")?
                    .ok_or_else(|| "loops cần max_len".to_owned())?;
                t.loops(&kw.str("node")?, max_len, &kw.edge()?, None)
            }
            "provenance" => t.provenance(&kw.str("claim")?),
            "select" => {
                let w = match kw.0.get("where") {
                    Some(Value::Object(m)) => m.clone(),
                    _ => return Err("select: where phải là object".to_owned()),
                };
                t.select(&w, None, kw.flag("system"))
            }
            // `as_of` có thể vắng (`None` của Python: SQL so với NULL, không note nào khớp); điểm vào chung xử lý
            // đúng như vậy và `valid` không có tham số note nên đi qua đó cho kết quả như `Tools.valid`.
            "valid" => {
                let mut args = Map::new();
                for k in ["type", "as_of", "system"] {
                    args.insert(k.into(), kw.0.get(k).cloned().unwrap_or(Value::Null));
                }
                return tools::call(&mut self.ctx, "valid", Value::Object(args))
                    .map_err(|e| e.to_string());
            }
            // Lệnh `related`, `read` của CLI (không có trong `cli.py`): tham số theo tên MCP, qua điểm vào chung.
            "related" | "read" => {
                let args: Map<String, Value> =
                    kw.0.into_iter().filter(|(_, v)| !v.is_null()).collect();
                return tools::call(&mut self.ctx, method, Value::Object(args))
                    .map_err(|e| e.to_string());
            }
            other => return Err(format!("phương thức tool lạ {other}")),
        };
        out.map(Value::Object).map_err(|e| e.to_string())
    }

    fn reopen(&mut self) -> Result<(), String> {
        let g = Graph::open(&self.db).map_err(|e| e.to_string())?;
        self.ctx.tools_mut().replace_graph(g);
        Ok(())
    }
}

/// Tham số theo tên Python mà CLI dựng (`cli::tool_kwargs`); `null` là `None`.
struct Kwargs(Map<String, Value>);

impl Kwargs {
    fn get(&self, k: &str) -> &Value {
        self.0.get(k).unwrap_or(&Value::Null)
    }

    fn opt_str(&self, k: &str) -> Result<Option<String>, String> {
        match self.get(k) {
            Value::Null => Ok(None),
            Value::String(s) => Ok(Some(s.clone())),
            v => Err(format!("{k} phải là chuỗi, có {v}")),
        }
    }

    fn str(&self, k: &str) -> Result<String, String> {
        self.opt_str(k)?.ok_or_else(|| format!("thiếu {k}"))
    }

    fn opt_int(&self, k: &str) -> Result<Option<i64>, String> {
        match self.get(k) {
            Value::Null => Ok(None),
            v => v
                .as_i64()
                .map(Some)
                .ok_or_else(|| format!("{k} phải là số nguyên, có {v}")),
        }
    }

    fn flag(&self, k: &str) -> bool {
        self.get(k).as_bool().unwrap_or(false)
    }

    fn list(&self, k: &str) -> Result<Option<Vec<String>>, String> {
        match self.get(k) {
            Value::Null => Ok(None),
            Value::Array(a) => a
                .iter()
                .map(|x| {
                    x.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| format!("{k} phải là danh sách chuỗi"))
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Some),
            v => Err(format!("{k} phải là danh sách, có {v}")),
        }
    }

    fn edge(&self) -> Result<tools::EdgeArgs, String> {
        Ok(tools::EdgeArgs {
            rels: self.list("rels")?,
            as_of: self.opt_str("as_of")?,
            statuses: self.list("statuses")?,
            accepted_only: self.flag("accepted_only"),
            system: self.flag("system"),
        })
    }
}
