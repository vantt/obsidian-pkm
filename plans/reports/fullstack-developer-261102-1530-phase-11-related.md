# Báo cáo: phase 11, phần `related` (2026-11-02)

## Kết quả

`related` của bản Rust cho kết quả giống Python từng bit trên kg-solo. Có hai cách gọi:

- **Có `hits`:** 15 lời gọi của golden khớp. Tôi so chặt thêm một lượt: đúng thứ tự, `pushes` và `touched` bằng nhau, số thực lệch 0. Tôi cũng sinh 400 lời gọi ngẫu nhiên, chạy lại bằng `dump-python.py --replay`, và chúng khớp hết.
- **Chỉ có câu hỏi:** hạt giống lấy qua `pkm_core::ohs`. Nếu OHS thiếu hoặc chưa sẵn sàng, tool trả `ohs_not_ready` (D7).

Bộ so khớp kg-solo: 64 mục, không có lệch nào chưa có tên. Còn 14 mục "chưa có", đều thuộc tầng MCP và CLI (phase 12).

Nhánh: `worktree-agent-a0da0c15a5e4b7988`. Nhánh này đã merge `worktree-agent-a7edaab571b62fd7a` (tầng query/tools), có xử lý xung đột ở `spec/PORTING.md` và `parity/src/rust_side*.rs`: tôi giữ cả hai phía.

## File đã sửa

| File | Việc |
|---|---|
| `crates/pkm-core/src/related.rs` (mới) | `load_adjacency`, `ppr_push`, `via`, `related`, `hits_from`, `py_sum`, `Options` |
| `crates/pkm-core/src/related/tests.rs` (mới) | 12 test |
| `crates/pkm-core/src/tools.rs` | Nhánh `related` của `call`. `Context` có thêm `ohs: Option<Arc<Ohs>>`, cùng `with_ohs`, `set_ohs`, `ohs()`. Thay đổi chỉ thêm, không sửa API cũ |
| `crates/pkm-core/src/tools/tests.rs` | Bỏ assert `NotImplementedError` cũ. Thêm `related_skips_system`, `call_related_with_hits`, `call_related_query_without_ohs_is_not_ready` |
| `crates/pkm-core/tests/related_ohs_fake.rs` (mới) | 4 test với `fake-ohs`, chạy trong CI |
| `crates/pkm-core/tests/related_ohs_real.rs` (mới) | Test `#[ignore]` với gói OHS thật. Đã chạy qua trên Linux với gói `0.17.2-vantt.1` trong 6,6 s, không còn tiến trình `node` sau khi xong |
| `parity/src/rust_side.rs` | Bỏ đoạn bỏ qua `related` |
| `parity/tests/golden.rs` | Tầng `tools` giờ đủ, kể cả `related` |
| `spec/PORTING.md` | Thêm mục "Module `related`". Sửa câu về ngoại lệ ở mục `ohs`, dòng `test_related_skips_system`, và câu "`related` chưa có" |
| `plans/.../phase-11-related-and-ohs-module.md` | Tick hết các ô (`ak plan check`), đặt `status: done`, ghi trạng thái |

## Vì sao khớp từng bit

- **Cộng bậc của node:** Python 3.12+ (máy này chạy 3.14.4) cộng float bằng `sum` có bù sai số Neumaier. Cộng thẳng sẽ lệch ở bit cuối, nên `related::py_sum` chép đúng thuật toán của CPython.
- **Thứ tự cạnh:** cạnh đọc bằng đúng câu SQL của Python, không có `ORDER BY`, nên thứ tự là thứ tự SQLite trả trên index dựng theo đường dẫn đã sắp.
- **Hàng đợi và thứ tự chèn:** dùng `VecDeque` và `IndexMap` để giống `deque` và `dict`. Mọi phép sắp đều ổn định.
- **Làm tròn:** `round(x, 5)` dùng `format!("{:.5}")`. Tôi đã kiểm rằng định dạng này làm tròn hoà về số chẵn trên giá trị nhị phân, giống Python.
- **Hạt giống không có khối lượng:** trường hợp này cho số nguyên `0`, như `round(0, 5)` của Python.
- **Tham số lạ:** chỗ Python ném ngoại lệ thì Rust trả lỗi cùng tên lớp, như `TypeError`, `ValueError`, `IndexError`. Ví dụ: hit dạng chuỗi, object hay danh sách sai kiểu, `float()` của điểm dạng chuỗi, `in` trên `types`.

## Quyết định cần biết

- **`seed_source` khi hạt giống lấy từ OHS:** tôi giữ giá trị `ohs-cli` của Python để hợp đồng MCP không đổi (PRD mục 8), dù bây giờ hạt giống đi qua stdio chứ không qua CLI. Điều này đã ghi trong PORTING.
- **Phiên không gắn OHS:** tool trả `ohs_not_ready` với `state: broken`. `action` nói cách chạy pkm với gói OHS (`--ohs`, `PKM_OHS_DIR`), hoặc gọi `search` của MCP `ohs` rồi đưa kết quả vào `hits`.
- **OHS `ready` mà không trả kết quả, hoặc OHS lỗi hay quá hạn:** tool trả lỗi `no OHS index reachable ...` như Python. Khi không có kết quả, `detail` là `ohs-cli`. Khi lỗi, `detail` là `OHS lỗi: <lý do>`.
- **Không cần ngoại lệ có tên trong bộ so khớp:** golden không có lời gọi `related` nào chỉ có câu hỏi.

## Việc cho agent khác

- **Phase 12 (MCP/CLI, `crates/pkm/**`):** khi khởi động, gọi `Ohs::launch(flag_ohs, OhsConfig::new(vault))`, bọc trong `Arc`, rồi gắn vào phiên bằng `ctx.set_ohs(Some(arc))` hoặc `Context::with_ohs`. Nếu không gắn, `related` chỉ có câu hỏi luôn trả `broken`.
- **`parity/README.md`** (ngoài quyền sửa của tôi): câu ở dòng 50 vẫn đúng. Mục "Nối tầng Rust" vẫn còn chỗ cũ mà báo cáo phase 10 đã nêu.

## Kiểm tra

- `cargo fmt --all --check`: qua.
- `cargo clippy --workspace --all-targets -- -D warnings`: qua.
- `cargo test --workspace`: qua hết. Có 3 test `#[ignore]` cần OHS thật.
- `parity check --vault kg-solo`: không có lệch nào chưa có tên.

## Câu hỏi còn mở

- Có muốn đổi `seed_source` sang `ohs` cho đúng nghĩa không? Nếu đổi thì hợp đồng MCP thay đổi, cần ghi vào `docs/MCP-CHANGELOG.md`.
