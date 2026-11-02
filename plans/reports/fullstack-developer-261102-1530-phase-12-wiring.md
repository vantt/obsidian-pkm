# Phase 12 (nối tầng tool): `pkm_core::tools` vào MCP server và CLI của `pkm`

Ngày: 2026-11-02. Phase: [phase-12-mcp-server-and-cli.md](../261102-0535-rust-core-port-master/phase-12-mcp-server-and-cli.md) (đặt `done`). Nhánh: `worktree-agent-a26e0c02db8ddb58c`, đã merge `worktree-agent-a7edaab571b62fd7a` (tầng query, tools). Repo đặc tả ở sha ghim `5a3eb2f`.

## 1. Kết quả

| Nghiệm thu | Kết quả |
|---|---|
| Bộ so khớp MCP `tools/list` khớp golden từng byte | Khớp. `cmp` file dump với golden: giống. Bản tin `tools/list` của `pkm` so với MCP server Python trên cùng vault: giống từng byte (13 924 byte), kể cả thứ tự khoá |
| Mọi lệnh CLI của golden khớp | 21 trên 21 lệnh khớp, `cmp` từng file `.out` và `.err` đều giống. `cli/commands.json` giờ cũng giống từng byte |
| Chuyển các test của `test_mcp.py` | 11 trên 13 đã chuyển. `test_related` và `test_related_on_fixture` chưa chuyển vì cần `related`; đã đánh dấu trong `spec/PORTING.md` |
| Phiên Claude Code headless | 10 tool gọi được, cả lỗi id sai có `did_you_mean` (mục 3) |
| `pkm mcp install`, rồi `--check` | Báo `kg 0.1.0, 11 tool` (mục 4) |
| `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` | Qua cả ba |

`parity check --vault <bản sao kg-solo>` cho 64 mục, 0 lệch chưa có tên. Chỉ còn 1 mục báo "chưa có": `tools/related` (15 lời gọi), chờ phase 11.

Tôi so thêm qua giao thức thật, giữa `pkm` và MCP server Python trên cùng vault. Bộ so có 15 lời gọi `tools/call`, phủ mọi tool trừ `related`, kể cả `more` với handle hết hạn, `select` rỗng, tool lạ và `find` có `name` là số (ra `error: AttributeError(...)`). Kết quả giống từng byte, trừ số `id` của cạnh. Nguyên nhân là bản Python chạy trên máy này đánh số cạnh theo thứ tự duyệt thư mục. Bản Rust đánh số như golden (ví dụ cạnh `party` của `HĐ ABC 2025` là 339 ở cả Rust lẫn golden, còn Python trên máy này ra 253).

## 2. Thay đổi

| File | Nội dung |
|---|---|
| `crates/pkm/src/mcp/tool_layer.rs` | `installed()` trả `CoreLayer`. `defs` là `tools::definitions(hidden)`. `open` là `Tools::with_limit` (CLI dùng `--limit` hoặc 20, MCP dùng mặc định) rồi `tools::Context`. `call` là `tools::call`, ngoại lệ thành `e.repr()` để vỏ ghi `error: KeyError('…')` như Python. `query` gọi các phương thức có kiểu của `Tools`, như `cli.py` gọi `Tools.<method>`: không NFC, không `did_you_mean`, không `url`, không `body`. Riêng `valid` đi qua `tools::call`, để khi vắng `--as-of` kết quả là rỗng như so với NULL của Python. `reopen` là `Graph::open` rồi `replace_graph`, giữ handle của `more` |
| `crates/pkm/tests/mcp_tools.rs` (mới) | Bản chuyển của `test_mcp.py`, chạy binary thật như `Client` của Python trên vault `NOTES` của `test_kg.py`. Gồm 9 test: `tools_list`, `find_and_note`, `unknown_node`, `neighbors_layers_window`, `reach_paths_loops`, `provenance_select_valid`, `more`, `edit_visible_on_next_call`, `answer_key_over_protocol`. Test cuối cần `PKM_KG_SOLO`, không có thì bỏ qua. Tôi đã chạy nó với kg-solo: 70 câu graph và 11 câu bảng đều qua |
| `crates/pkm/tests/cli_commands.rs` | Thêm `graph_commands_query_the_index_like_python_cli`, phủ `find`, `note` (không `url`, `body`; id sai trả `{"error": …}`), `neighbors`, `reach`, `select --limit` (có handle), `valid` không có `--as-of` |
| `parity/src/normalize.rs` | `canonical()` tự sắp khoá đệ quy rồi mới in. Nghĩa khác giữ nguyên. Thêm test |
| `parity/src/rust_side.rs` | Giải xung đột merge: giữ cả `pkm` (binary) lẫn `graph`, `tools`. `cli/commands.json` ghi khoá đã sắp như `write_json`, nên giống golden từng byte |
| `parity/src/rust_side/index_tables.rs`, `spec/PORTING.md` | Giải xung đột merge. Ở `index_tables.rs` giữ `pub` của nhánh tools. Ở `PORTING.md` giữ cả hai mục |
| `parity/README.md` | Mục "Nối tầng Rust" viết lại theo phương thức thật của `RustSide`. Thêm điều kiện binary `pkm` phải được dựng trước `parity check`, và dòng về sắp khoá trong `canonical` |
| `spec/PORTING.md` | Mục MCP và CLI: cách nối tầng tool, kết quả so từng byte, bảng 13 test (11 đã chuyển, 2 chờ `related`) |

Không đụng `crates/pkm-core/**`, `crates/pkm/src/{install,doctor}*`. `Cargo.toml` và `Cargo.lock` không đổi. Không có chỗ nào giả định `\n` hay CRLF trên bản tin, cũng không có đường dẫn chỉ chạy trên Unix: test đọc dòng bằng `read_line` rồi `trim_end`, đường dẫn dựng bằng `join`.

## 3. Phiên Claude Code headless

Lệnh chạy trong thư mục scratch, không dùng cấu hình MCP chung của máy:

```text
claude -p --mcp-config mcp.json --strict-mcp-config \
  --allowedTools "mcp__kg__find,…,mcp__kg__related" --output-format stream-json --verbose < prompt.txt
```

`mcp.json` chỉ có server `kg`, chạy `pkm <bản sao kg-solo> mcp --db <scratch>/graph.db --packs business --log <scratch>/calls.jsonl`. Lúc khởi động, Claude Code báo `kg` ở trạng thái `connected` và thấy 11 tool `mcp__kg__*`. Phiên chạy 13 lượt, không lỗi.

Log `--log` của server (thời gian theo giờ Asia/Saigon):

```text
{"t": "2026-11-02T15:34:21", "tool": "find", "args": {"name": "Đông Á"}, "chars": 1240, "error": null, "refresh_ms": 11.2, "ms": 2.9}
{"t": "2026-11-02T15:34:21", "tool": "note", "args": {"node": "Claim/Chất lượng bàn giao tăng hài lòng của Đông Á.md"}, "chars": 997, "error": null, "refresh_ms": 0.0, "ms": 1.3}
{"t": "2026-11-02T15:34:22", "tool": "neighbors", "args": {"node": "Khách hàng/Công ty ABC.md", "dir": "in"}, "chars": 2752, "error": null, "refresh_ms": 0.0, "ms": 0.7}
{"t": "2026-11-02T15:34:22", "tool": "reach", "args": {"node": "Claim/Chất lượng bàn giao tăng hài lòng của Đông Á.md", "dir": "in", "max_depth": 3}, "chars": 22, "error": null, "refresh_ms": 0.0, "ms": 0.3}
{"t": "2026-11-02T15:34:23", "tool": "paths", "args": {"from": "Khái niệm/Bán hàng tư vấn.md", "to": "MOC/MOC Kinh doanh.md", "max_len": 4}, "chars": 226, "error": null, "refresh_ms": 0.0, "ms": 0.7}
{"t": "2026-11-02T15:34:24", "tool": "loops", "args": {"node": "Claim/Chất lượng bàn giao tăng hài lòng của Đông Á.md", "max_len": 4}, "chars": 33, "error": null, "refresh_ms": 0.0, "ms": 0.7}
{"t": "2026-11-02T15:34:24", "tool": "provenance", "args": {"claim": "Claim/Bài đăng LinkedIn đi cùng lead.md"}, "chars": 504, "error": null, "refresh_ms": 0.0, "ms": 0.5}
{"t": "2026-11-02T15:34:25", "tool": "select", "args": {"type": "claim", "limit": 3}, "chars": 215, "error": null, "refresh_ms": 0.0, "ms": 1.2}
{"t": "2026-11-02T15:34:25", "tool": "valid", "args": {"type": "claim", "as_of": "2026-10-15"}, "chars": 882, "error": null, "refresh_ms": 0.0, "ms": 0.9}
{"t": "2026-11-02T15:34:25", "tool": "neighbors", "args": {"node": "Doanh thu"}, "chars": 244, "error": true, "refresh_ms": 0.0, "ms": 4.1}
{"t": "2026-11-02T15:34:27", "tool": "more", "args": {"handle": "select-145687450a", "offset": 3, "limit": 3}, "chars": 219, "error": null, "refresh_ms": 13.5, "ms": 0.1}
```

Lời gọi `more` đến sau hơn 5 giây (`--refresh-every` mặc định), nên server dựng tăng dần trước (`refresh_ms` 13.5) mà vẫn giữ được handle của `select`.

Câu trả lời cuối của Claude, rút gọn:

1. `find "Đông Á"`: 12 kết quả. Kết quả đầu là `Claim/Chất lượng bàn giao tăng hài lòng của Đông Á.md`; khách hàng `Khách hàng/Quán Đông Á.md` có alias "Đông Á".
2. `note`: claim đã nhận, `Yếu tố/Chất lượng bàn giao.md` **increases** `Yếu tố/Mức hài lòng của Đông Á.md`, nguồn `Họp Đông Á 2025-03-18#^b1`.
3. `neighbors in` của Công ty ABC: 9 cạnh. Cạnh đầu là `contact` tới `Người/Nguyễn Văn An.md`, đã superseded (2024-06-01 đến 2026-03-31).
4. `reach in`, độ sâu 3: 0 mục.
5. `paths`: 1 đường, `Bán hàng tư vấn` → related → `Chân dung khách hàng` → up → `MOC Kinh doanh`, lớp accepted, dấu +.
6. `loops`, `max_len 4`: 0 vòng.
7. `provenance`: claim `correlates_with`, nguồn `Họp/Họp nội bộ 2026-01-05.md`.
8. `select claim limit 3`: 133 claim, handle `select-145687450a`. `more` với offset 3 trả 3 mục tiếp theo.
9. `valid claim 2026-10-15`: 74 claim, có handle `valid-2fd12f000c`.
10. `neighbors "Doanh thu"`: lỗi `no note with id 'Doanh thu'; ids are exact vault paths`, kèm 5 gợi ý `Yếu tố/Doanh thu …`.

Claude kết luận: "all 10 tools (find, note, neighbors, reach, paths, loops, provenance, select, more, valid) worked."

Bản ghi đầy đủ nằm ở `/tmp/claude-1000/-home-vantt-projects-obsidian-pkm/2dfb5915-3011-441f-a6fb-2c7cb5abd60b/scratchpad/p12b/e2e/session.jsonl`, ngoài repo.

## 4. `pkm mcp install` trên file cấu hình tạm

```text
$ pkm mcp install <kg-solo> --config <tạm>/claude_desktop_config.json --db <tạm>/graph.db --packs business --no-ohs
dựng index <tạm>/graph.db …
<tạm>/graph.db (full): 746 note, đọc 746, xoá 0; ms {…}
cạnh theo lớp: {'accepted': 708, 'proposed': 8, 'superseded': 10}
đã ghi `kg` vào <tạm>/claude_desktop_config.json; servers: kg
server trả lời: kg 0.1.0, 11 tool. Mở lại Claude Desktop để dùng `kg`.

$ pkm mcp install --check --config <tạm>/claude_desktop_config.json --no-ohs
<binary pkm>: kg 0.1.0, 11 tool: find, note, neighbors, reach, paths, loops, provenance, select, valid, related, more
```

## 5. Lưu ý cho lead

- **`related`:** tool này đi qua `tools::call`. Khi lõi chưa có, MCP trả `error: NotImplementedError('tool related chưa có trong bản Rust')` với `isError`. Phase 11 xong thì không phải sửa vỏ. Còn hai việc phía bộ so khớp và test: bỏ dòng `if name == "related"` trong `parity/src/rust_side.rs`, và chuyển hai test `related` vào `tests/mcp_tools.rs`.
- **Cờ OHS:** `ToolContext::ohs_cli` và `ohs_node` chưa được dùng, vì `tools::Context` chưa có chỗ nhận. Phase 11 cần quyết cách đưa hai giá trị này vào `related`. Có thể thêm trường vào `tools::Context`; khi đó `CoreLayer::open` truyền `ctx.ohs_cli`, `ctx.ohs_node` vào.
- **Trạng thái trong plan store:** tôi chỉ sửa frontmatter và checkbox của file phase. Tôi không gọi `ak plan phase close`, vì plan store là SQLite dùng chung giữa các worktree. Lead đồng bộ khi merge.
- **Windows:** không đổi `mcp::tests::handshake` hay `tools_list_comes_from_the_tool_layer`. Hai test đó vẫn dùng tầng tool giả.

## Câu hỏi còn mở

1. Cờ `--limit`, `--system` cho MCP (PRD mục 8): vẫn chờ lead quyết, như câu hỏi ở báo cáo trước. Mục này trong phase chưa đánh dấu xong.
2. Cách đưa `--ohs-cli`, `--ohs-node` vào `related`: phase 11 quyết.
