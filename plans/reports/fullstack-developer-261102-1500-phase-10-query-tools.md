# Báo cáo phase 10: `query`, `tools`, `more`

- Phase: [phase-10-query-tools-more.md](../261102-0535-rust-core-port-master/phase-10-query-tools-more.md)
- Trạng thái: xong, trừ bản B (user hoãn; ô nghiệm thu bản B để trống).
- Nhánh: `worktree-agent-a7edaab571b62fd7a` (commit ghi ở cuối lời trả về cho lead).

## 1. Kết quả

Mọi tool trừ `related` cho JSON giống Python ở sha ghim, kể cả lỗi và tên ngoại lệ.

| Tầng của bộ so khớp (kg-solo, golden) | Đơn vị | Lệch |
|---|---|---|
| `answers` (9 phép của `fixture_check.run`) | 81 câu | 0 |
| `tools` (`find`, `note`, `neighbors`, `reach`, `paths`, `loops`, `provenance`, `select`, `valid`, `more`) | 712 lời gọi | 0 |
| `tools/related` | 15 | chưa có (phase 11) |

`cargo run -q --locked -p pkm-parity -- check --vault <kg-solo>`: 0 lệch chưa có tên, mã thoát 0. `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` qua. `PKM_KG_SOLO=<kg-solo>`: `query::tests::answer_key` cho 70/70 câu graph, 11/11 câu bảng.

Ngoài golden, tôi chạy `dump-python.py` của repo đặc tả rồi `parity rust-dump` và `parity compare` (Python với Rust, không qua golden) trên:

- kg-solo đọc như bản B (`--no-aliases --edge-keys links --body-edges`): 769 lời gọi tự sinh;
- vault cài lỗi của `test_kg.py`;
- vault e11 của `test_kg_adapter.py` với `--adapter e11`: trang hệ thống, `system`, lớp `proposed`, `rejected`;
- 181 lời gọi lệch chuẩn trên kg-solo, chạy lại bằng `--replay`. Bộ này gồm tham số rỗng, sai kiểu, số âm, `limit: true`, `rels` là chuỗi, `max_depth` là số thực, `max_len` là chuỗi, note id dạng NFD hay là số, handle hết hạn, là số, là danh sách, `offset` âm hay vượt cỡ, tool lạ, thiếu tham số bắt buộc.

Cả bốn đều 0 lệch. Lần đầu bộ lệch chuẩn có 14 lệch, do Python trả lại `offset: true` nguyên như người gọi đưa, còn Rust trả `1`. Tôi đã sửa cho Rust trả đúng giá trị người gọi đưa.

Bộ định nghĩa tool (`tools/list`) giống `tool_defs` của Python từng ký tự, kể cả thứ tự khoá, với `hidden` đúng và sai. Tôi kiểm phần này một lần trên máy, so với JSON do Python sinh. Test CI so với `mcp/tools_list.json` của golden, nhưng phép so đó không xét thứ tự khoá.

## 2. File

| File | Nội dung |
|---|---|
| `crates/pkm-core/src/query.rs` | `Graph`, `Filter`, `Dir`, `QueryError`; SQL chép nguyên từ `query.py` |
| `crates/pkm-core/src/query/py.rs` | `bool`, `str`, `repr`, `json.dumps(sort_keys)`, `int()`, cắt danh sách kiểu Python |
| `crates/pkm-core/src/query/answers.rs` | Bản chuyển của `fixture_check.py` (`run`, `norm`, `check`, `GRAPH_OPS`, `TABLE_OPS`) |
| `crates/pkm-core/src/query/tests.rs` | 12 test |
| `crates/pkm-core/src/tools.rs` | `Tools` (vỏ kết quả dài, handle, bộ nhớ 64 kết quả, `more`), `EdgeArgs`, `Context`, `call`, `default_vault_name` |
| `crates/pkm-core/src/tools/defs.rs` | `ToolDef`, `definitions(hidden)`, `tools_list(hidden)` |
| `crates/pkm-core/src/tools/tests.rs` | 14 test, gồm 4 test `HiddenAndLayers` trên vault e11 |
| `parity/src/rust_side.rs` | Nối `answer` và `tool`; thêm `Outcome` (`result` hoặc `raised`); dòng ghi ra có khoá đã sắp |
| `parity/src/rust_side/index_tables.rs` | `Built` giữ DB tạm và `Scrub` cho tầng câu đáp án và tool (cùng DB, như `dump-python.py`) |
| `parity/tests/golden.rs` | Test "chưa có" cập nhật: `answers` và `tools` (trừ `related`) giờ có kết quả |
| `spec/PORTING.md` | Bảng `test_kg.py` tách đủ 15 dòng; dòng `HiddenAndLayers`; mục mới `query`, `tools` (bảng `fixture_check`, giới hạn đã biết) |

Không thêm crate, `Cargo.lock` không đổi. `sha1_smol` đã có sẵn.

## 3. API cho phase sau

- **Điểm vào chung:** `tools::call(&mut Context, name, args: Value) -> Result<Value, ToolError>`.
  - Hành vi như `Server.call`, trước bước `compact`.
  - `Ok({"error": ...})` cho các lỗi mà Python trả thành kết quả.
  - `Err(ToolError { kind, message })` khi Python ném ngoại lệ. `kind` là tên lớp ngoại lệ Python; `e.repr()` cho chuỗi `KeyError('name')` mà MCP ghi sau `error: `.
- **Phiên:** `Context::new(Tools, vault, vault_name)`. Tên mặc định của vault lấy bằng `tools::default_vault_name(vault)` (như `os.path.abspath` rồi `basename`); truyền `None` để tắt link.
- **Tool `related` (phase 11):** sửa hàm `related(ctx, &Args)` trong `tools.rs`. Hiện hàm trả `NotImplementedError`, và bộ so khớp bỏ qua `related` trong `RustSide::tool`. Phase 11 cần:
  - `Graph::edge_where` (`_w`), `Filter::new` (`_f`), `Graph::hidden`, `is_hidden`, `conn`, `types_of`, `rels`;
  - bỏ dòng `if name == "related"` trong `parity/src/rust_side.rs`.
- **MCP (phase 12):**
  - `tools::tools_list(hidden)` trả nguyên kết quả `tools/list`, với `hidden = !graph.hidden().is_empty()`.
  - `Tools::replace_graph` đổi index sau khi build lại mà vẫn giữ bộ nhớ của `more`.
  - Phase 12 còn phải tự làm: `compact`, cắt theo `max_chars`, cách ghi số thực vào JSON văn bản (Python ghi `1e-06`, `serde_json` ghi `1e-6`).
- **CLI:** `Tools::with_limit(g, limit)` và các phương thức có kiểu `find`, `note`, `neighbors(node, dir, &EdgeArgs, limit)`, `reach`, `paths`, `loops`, `provenance`, `select`, `valid`, `more`, giống `Tools` của Python mà `cli.py` gọi.

## 4. Chỗ lệch và việc ngoài quyền sửa

- **Không cần ngoại lệ có tên mới.** Các chỗ Rust khác Python đều chỉ xảy ra khi tham số sai kiểu. Ví dụ: `as_of` là số, `rels` chứa số, `limit` là số thực, `max_depth` là chuỗi (với giá trị này `reach` của Python chạy không dừng). Rust trả `TypeError` cho các trường hợp đó. Tôi ghi chúng ở mục "Giới hạn đã biết" của `spec/PORTING.md`, như phase 09 đã làm. Bộ so khớp không gặp trường hợp nào như vậy.
- **`parity/src/normalize.rs::canonical`:** chú thích nói khoá object đã sắp, nhưng `preserve_order` bật cho cả workspace (do `pkm-core`), nên chuỗi so phụ thuộc thứ tự khoá. Tôi sửa ở phía dump của Rust: dòng ghi ra có khoá đã sắp, đúng định dạng dump của README. Hàm `canonical` vẫn nên tự sắp khoá, để tầng CLI (phase 12, 13) không gặp lại lỗi này. File đó ngoài quyền sửa của tôi.
- **`parity/README.md`, mục "Nối tầng Rust":** câu "đang trả `None`" đã cũ, vì `answer` và `tool` giờ trả `Outcome`. File này ngoài quyền sửa của tôi.
- **Chép lại code để tránh sửa module khác:**
  - `FM_RE` chép từ `vault.rs` vào `tools.rs`, vì `vault` để regex này riêng tư.
  - Hàm `json_str` viết lại trong `query/py.rs`, vì `index::py` riêng tư.
  - Vault e11 chép vào `tools/tests.rs`, vì `adapter::tests` riêng tư.
  - Muốn gộp lại thì cho `vault` công khai `FM_RE`, và cho `index::py` hoặc `adapter::tests` mở cho cả crate.
- **Kiểm chéo `--target` Windows và macOS:** không chạy được trên máy này, vì SQLite đóng gói sẵn cần trình biên dịch C của đích. CI chạy trên cả ba nền.

## 5. Câu hỏi còn mở

1. Bản B chưa chạy (user hoãn). Lệnh: `python3 parity/parity.py run --private <bản B> --adapter e11 --packs capture --no-aliases --edge-keys links --body-edges`. Tầng `tools` cần 0 lệch chưa có tên, trừ `related`.
2. Có cho phép sửa `parity/src/normalize.rs::canonical` (sắp khoá trước khi so) trong một phase sau không?
