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
//! Binary chỉ có tầng tool khi [`installed`] trả `Some`. Khi chưa có, MCP server vẫn bắt tay (`initialize`,
//! `instructions`) nhưng `tools/list` rỗng và `tools/call` báo lỗi; lệnh CLI cần tool thoát với mã
//! [`crate::cli::EXIT_NO_TOOL_LAYER`].

use std::path::PathBuf;

use serde_json::{Map, Value};

/// Những gì một phiên tool cần biết về vault và index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolContext {
    /// Index SQLite (đã dựng).
    pub db: PathBuf,
    /// Thư mục vault, tuyệt đối (`note` đọc thân note từ đây).
    pub vault: PathBuf,
    /// Tên vault trong Obsidian cho link `obsidian://open` (`url` của `note`); `None` là không thêm link.
    pub vault_name: Option<String>,
    /// Số mục trước khi rút gọn khi lời gọi không ghi `limit` (`Tools(limit=...)`; CLI: `--limit` hoặc 20).
    pub limit: Option<i64>,
    /// `--ohs-cli` của MCP server (gói OHS cho `related` khi không có `hits`).
    pub ohs_cli: Option<String>,
    /// `--ohs-node` của MCP server.
    pub ohs_node: Option<String>,
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

/// Tầng tool có trong binary này. Đây là điểm nối duy nhất: khi `pkm_core::tools` có điểm vào chung, hàm này trả
/// bản bọc nó. Hiện binary chưa có tầng tool.
pub fn installed() -> Option<std::sync::Arc<dyn ToolLayer>> {
    None
}
