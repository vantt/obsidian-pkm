# Phase 12 (phần không cần tầng tool): MCP server và CLI của `pkm`

Ngày: 2026-11-02. Phase: [phase-12-mcp-server-and-cli.md](../261102-0535-rust-core-port-master/phase-12-mcp-server-and-cli.md) (giữ `in-progress`). Repo đặc tả ở sha ghim `5a3eb2f`.

## 1. Kết quả

- MCP server `pkm VAULT mcp` chép `scripts/kg/mcp_server.py`: giao thức JSON-RPC tự viết (Q4), `instructions` ghép đủ, dựng index ở luồng nền, refresh trước `tools/call`, thu gọn, cắt `--max-chars`, log `--log`, thoát 0 khi hết stdin.
- CLI `pkm VAULT <lệnh>` chép `scripts/kg/cli.py`: `build` (`--full`, `--json`), `stats`, `validate` chạy thật; 9 lệnh hỏi graph đã có đủ cờ và cách đổi sang tham số `Tools`, chỉ chờ tầng tool.
- `pkm mcp install …` và `pkm doctor …` đã đăng ký; bỏ `#![allow(dead_code)]` ở `install.rs`, `doctor.rs`. Không phải đổi `server_entry`, `entry_paths`: hình dạng `pkm VAULT build --db DB …` và `pkm VAULT mcp --db DB …` giữ nguyên.
- Bộ so khớp trên kg-solo (bản sao trong scratchpad): `mcp/initialize`, `mcp/instructions` và 8 lệnh CLI (`build-full`, `build-noop`, `build-json`, `stats`, `validate`, `validate-json`, `validate-rule`, `loops-no-max-len`) **khớp, giống golden từng byte** (`cmp` từng file dump). 0 lệch chưa có tên. `mcp/tools_list` và 13 lệnh CLI hỏi graph báo "chưa có".
- Cài thật vào file cấu hình tạm (không đụng cấu hình Claude Desktop của máy): `install` dựng index qua `pkm VAULT build`, ghi `kg`, bắt tay được (`kg 0.1.0, 0 tool`); `--check`, `--remove` chạy đúng.

## 2. Chỗ nối với tầng tool (việc của người nối phase 10)

File: `crates/pkm/src/mcp/tool_layer.rs`. Chỉ một chỗ phải sửa: hàm `installed()`, hiện trả `None`.

```rust
pub struct ToolContext { db, vault /* tuyệt đối */, vault_name: Option<String>, limit: Option<i64>, ohs_cli: Option<String>, ohs_node: Option<String> }
pub trait ToolLayer: Send + Sync {
    fn defs(&self, hidden: bool) -> Vec<Value>;                       // = tool_defs(hidden) của Python
    fn open(&self, ctx: &ToolContext) -> Result<Box<dyn ToolSession>, String>;   // = Tools(Graph(db))
}
pub trait ToolSession: Send {
    fn call(&mut self, name: &str, args: Map<String, Value>) -> Result<Value, String>;   // = Server.call
    fn query(&mut self, method: &str, kwargs: Map<String, Value>) -> Result<Value, String>; // = Tools.<method>(**kwargs)
    fn reopen(&mut self) -> Result<(), String>;                       // index đổi: mở lại graph, giữ handle của `more`
}
pub fn installed() -> Option<Arc<dyn ToolLayer>> { None }
```

Hợp đồng chi tiết:

- `defs(hidden)`: mỗi phần tử `{"name", "description", "inputSchema": {"type": "object", "properties", "required"}}`, đúng thứ tự và câu chữ của `tool_defs` (golden `mcp/tools_list.json`). `hidden` đúng khi index của phiên có trang hệ thống (`meta.hidden` khác rỗng); vỏ tự tính.
- `call(name, args)`: `name` như client gửi (`"None"` khi thiếu), `args` là `arguments`. Gồm mọi việc của `Server.call`: NFC id, kiểm id và `did_you_mean`, `url` (dùng `ctx.vault_name`), `body` (đọc từ `ctx.vault`), kiểm `loops`/`select`, hạt OHS của `related`. Trả kết quả **trước** `compact`; lỗi nghiệp vụ là object có `error`. `Err(s)` là ngoại lệ: vỏ trả `error: s`, `isError: true`. Vỏ lo: chờ dựng đầu, refresh, `index unavailable: …` khi chưa có phiên, `compact`, `isError`, cắt, log.
- `query(method, kwargs)` cho CLI, tên tham số Python, `null` là `None`: `find {name, type}`; `note {node}`; `neighbors {node, dir, rels, as_of, statuses, accepted_only, system}`; `reach {node, dir, max_depth, …lọc}`; `paths {frm, to, max_len, …lọc}`; `loops {node, max_len, …lọc}`; `provenance {claim}`; `select {where, system}`; `valid {type, as_of, system}`. CLI mở phiên với `limit = --limit hoặc 20`, `vault_name = None` (như `Tools(dbpath, limit=…)`; CLI không có `url`, `body`). Ánh xạ cờ nằm ở `cli::tool_kwargs`, có test.
- `reopen()`: vỏ gọi sau build `full` hoặc có note đọc lại, xoá (như `_build` của Python).

Khi nối xong: `tools/list` có 11 tool, lệnh CLI hỏi graph chạy, bộ so khớp tự chuyển các mục "chưa có" sang so thật (không cần sửa `parity/`). Test vỏ đã có tầng tool giả (`crates/pkm/src/mcp/tests.rs`, danh mục lấy từ golden); thêm test thật cho 9 test `test_mcp.py` còn lại.

## 3. File

| File | Nội dung |
|---|---|
| `crates/pkm/src/main.rs` | Gọi `cli::run` |
| `crates/pkm/src/cli.rs` | Chia lệnh (`VAULT mcp`, `mcp install`, `doctor`, `--version`/`--help`, lệnh trên vault); `clap` với `infer_long_args` (viết tắt cờ như `argparse`); `build`, `stats`, `validate`; lệnh hỏi graph qua `ToolSession::query`, chưa có tầng tool thì thoát 3 |
| `crates/pkm/src/mcp.rs` | Cờ (`McpArgs`), `Server` (luồng nền, refresh, `handle`, `serve`) |
| `crates/pkm/src/mcp/instructions.rs` | Ghép `instructions` từ index và schema nhúng, `urllib.parse.quote` |
| `crates/pkm/src/mcp/pyjson.rs` | `json.dumps` của Python (dấu phân cách, thụt lề, `ensure_ascii`, `repr(float)`) cho bản tin MCP và đầu ra CLI |
| `crates/pkm/src/mcp/tool_layer.rs` | Chỗ nối tầng tool (mục 2) |
| `crates/pkm/src/mcp/tests.rs` | 15 test của vỏ MCP, tầng tool giả |
| `crates/pkm/tests/mcp_stdio.rs`, `crates/pkm/tests/cli_commands.rs` | Chạy binary thật: bắt tay qua stdio và thoát khi stdin đóng; `build`/`stats`/`validate`; `mcp install --dry-run --config <tạm>`; `doctor --json` |
| `crates/pkm/src/{install,doctor}.rs` | Chỉ bỏ `#![allow(dead_code)]` |
| `crates/pkm/Cargo.toml`, `Cargo.lock` | Thêm `chrono` (ngày máy, giờ log), `indexmap`, `rusqlite` (đọc index cho `stats`, `instructions`), feature `env` của `clap` (`KG_OHS_CLI`, `KG_OHS_NODE`); đều đã có trong workspace |
| `parity/src/rust_side/binary.rs` (mới) | Tầng MCP và CLI chạy binary `pkm` (`PKM_BIN` hoặc cạnh binary đang chạy), che giá trị đổi theo lần chạy như `mask_cli`, sắp khoá như `write_json` |
| `parity/src/rust_side.rs` | `mcp()`, `cli()` nối vào `binary`; `McpHandshake::tools_list` thành `Option` (rỗng thì "chưa có"); thêm `dump_with(…, pkm)` |
| `parity/src/rust_side/index_tables.rs` | Chỉ mở `pub(super)` cho `TempDir`, `Scrub`, `read_run` để dùng lại |
| `parity/tests/golden.rs` | Test "mọi tầng còn thiếu là chưa có" gọi `dump_with(…, None)` để không phụ thuộc binary đã dựng hay chưa |
| `spec/PORTING.md` | Mục "MCP server và CLI": giống, khác Python, bảng 13 test `test_mcp.py` |

## 4. Kiểm

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`: sạch.
- `cargo test --workspace --locked`: qua hết (`pkm`: 80 test đơn vị, 6 + 2 test binary, 1 test version; `pkm-core`, `pkm-parity` như trước, thêm 2 test của `binary`).
- `parity check --vault <bản sao kg-solo>`: 64 mục, 0 lệch chưa có tên, 34 mục chưa có (tầng answers, tools, `mcp/tools_list`, 13 lệnh CLI hỏi graph).

## 5. Lưu ý cho lead

- CI: `parity check` cần binary `pkm` đã dựng ở cùng `target/`. Bước `cargo test --workspace` đứng trước nên đã có; nếu đổi thứ tự bước, thêm `cargo build -p pkm` (không thấy binary thì hai tầng báo "chưa có" kèm cảnh báo, không bao giờ thành "khớp"). `parity/README.md` (ngoài quyền sửa) nên thêm một dòng về điều này.
- `pkm mcp install` lúc này bắt tay được nhưng báo `0 tool` (chưa có tầng tool).
- Bản tin MCP in như `json.dumps` của Python (`, ` và `: `); bài thử S2 in gọn. Client không phân biệt; chọn giống Python cho đúng PRD mục 8.

## Câu hỏi còn mở

1. PRD mục 8 kể `--limit`, `--system` trong cờ khởi động MCP, nhưng `parser()` của Python ở `5a3eb2f` không có hai cờ này (truyền vào thì Python báo lỗi). Tôi theo Python, chưa thêm. Có thêm không, và nếu thêm thì nghĩa là gì (giới hạn mặc định của phiên, mặc định `system` cho mọi tool)?
2. Nghĩa mới của `--ohs-cli` (đường dẫn gói OHS, PRD mục 9) hiện chỉ chuyển nguyên cho tầng tool qua `ToolContext`. Người nối tầng tool `related` quyết cách dùng; có cần đổi tên cờ không?
3. Bản ghi phiên Claude Code gọi đủ tool (nghiệm thu phase) phải chờ tầng tool.
