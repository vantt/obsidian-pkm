# Báo cáo: module `ohs` của phase 11

- Phase: `plans/261102-0535-rust-core-port-master/phase-11-related-and-ohs-module.md`, chỉ phần module OHS. Phần `related` và PageRank chờ tầng truy vấn. Phase vẫn `in-progress`.
- Ngày: 2026-11-02. Máy: Linux x64.
- Kết quả: **xong.** Test với server giả qua; test với OHS thật (`#[ignore]`) cũng qua.

## Đã làm

| File | Nội dung |
|---|---|
| `crates/pkm-core/src/ohs.rs` | API công khai (`Ohs`, `OhsConfig`, `OhsError`) và luồng giám sát: khởi động, theo dõi, khởi động lại có giới hạn, tắt |
| `crates/pkm-core/src/ohs/package.rs` | Tìm gói theo thứ tự: cờ `--ohs`, `PKM_OHS_DIR`, rồi `<pkm_dir>/ohs`. Kiểm đủ Node đi kèm và `app/dist/src/cli.js` |
| `crates/pkm-core/src/ohs/process.rs` | Một tiến trình `serve --stdio`: JSON Lines, ghép phản hồi theo `id`, đọc stdout và stderr ở luồng nền, đóng có giới hạn rồi kill |
| `crates/pkm-core/src/ohs/state.rs` | `OhsState` theo D7, `from_status`, lỗi `ohs_not_ready`, tính `eta_s` từ tốc độ index |
| `crates/pkm-core/tests/support/fake_ohs.rs` | Server giả `fake-ohs` (`[[bin]]` chỉ dùng khi test; bản phát hành chỉ dựng `-p pkm`) |
| `crates/pkm-core/tests/ohs_fake.rs` | 17 test tích hợp với server giả |
| `crates/pkm-core/tests/ohs_real.rs` | 1 test `#[ignore]` với gói OHS thật |
| `crates/pkm-core/Cargo.toml`, `Cargo.lock` | Thêm `serde_json` với `preserve_order` (Q4). Lock chỉ thêm `indexmap` vào `serde_json` |
| `spec/PORTING.md` | Thêm mục "Module `ohs`" và lệch có chủ ý "Không lùi về đọc thẳng DB của OHS" (Python `ohs_seeds` lùi về FTS5 của DB OHS; Rust thì không, theo D7 và PRD mục 9) |

## API cho tầng `related`, `tools` và MCP

Module chạy đồng bộ bằng `std::thread`, không dùng async runtime (Q4). `Ohs` là `Send + Sync`, nên MCP server giữ một `Arc<Ohs>` dùng chung.

- `Ohs::launch(flag: Option<&Path>, OhsConfig) -> Ohs`: tìm gói rồi khởi động ở nền, trả về ngay. Không có gói thì trạng thái là `broken`, kèm `action` chỉ cách cài.
- `Ohs::search(&self, query, options: Value) -> Result<Vec<Value>, OhsError>`: trước khi tìm, hỏi `status`. Chỉ khi `ready` mới gửi `search`. Mọi `path` trả về đã chuẩn hoá sang NFC.
- `OhsError::NotReady(state)`: `err.not_ready_error()` trả đúng `{"error":"ohs_not_ready","state","progress","eta_s","action"}`, đủ năm khoá theo thứ tự đó. `progress` làm tròn 2 chữ số; ngoài `indexing` thì `progress` và `eta_s` là `null`.
- `Ohs::state()`, `Ohs::wait_ready(timeout)` (cho CLI chạy một lần), `Ohs::pid()`, `Ohs::shutdown()` (bỏ `Ohs` cũng tắt).

## Hành vi chính

- Lệnh chạy: `node cli.js serve --stdio`. Thư mục hiện tại là thư mục gói. Môi trường gồm `OBSIDIAN_VAULT_PATH`, `OBSIDIAN_DB_PATH` khi `OhsConfig::db` có giá trị, và `OHS_DISABLE_UPDATE_CHECK=1`; bỏ `NODE_OPTIONS`. Trên Windows chạy với `CREATE_NO_WINDOW`. Đường dẫn vault, DB và gói được đổi sang tuyệt đối trước khi chạy.
- `db` mặc định là `None`: OHS tự chọn DB, nên dùng chung index với MCP `ohs` của người dùng (giống Python dùng `VAULT/.obsidian-hybrid-search.db`).
- Khởi động lại: tiến trình chết, thoát trước `ready`, hoặc quá `ready_timeout` thì khởi động lại sau `restart_backoff * n`. Quá `max_restarts` (mặc định 3) lần liên tiếp thì thành `broken`; `action` có mã thoát và stderr cuối. Nếu lần chạy trước sống quá `stable_after` (mặc định 5 phút) thì đếm lại từ đầu.
- `eta_s`: lấy mẫu `status` đầu tiên của một lượt index làm mốc, tốc độ là số note index được kể từ mốc. Mốc bị bỏ khi đổi tiến trình hoặc khi `indexed` giảm. Luồng giám sát hỏi `status` mỗi `poll_interval` (mặc định 2 s) tới khi index xong, nên lần tool hỏi đầu tiên đã có số đo.
- Khi tắt: đóng stdin, chờ tối đa `shutdown_timeout` (mặc định 10 s), quá hạn thì kill và luôn reap. `Drop` của tiến trình là lưới an toàn cuối. Chỉ kill tiến trình con do chính module khởi động.
- `status` hết hạn hoặc mất kết nối thì báo `starting`, vì trường hợp này tự khỏi. OHS từ chối `status` hoặc trả sai định dạng thì báo `broken`, kèm hướng dẫn cài lại gói đúng phiên bản.

## Kiểm

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`: qua.
- `pkm-core`: 38 unit test (gồm 23 của `ohs`) và 17 test với server giả. Chạy 5 lần liên tiếp rồi 4 bộ song song: đều qua, mỗi bộ khoảng 2 s.
- Các test server giả kiểm: ghép `id` khi phản hồi về ngược thứ tự; NFC; môi trường và tham số khởi động; vault tương đối; `starting` trước `ready` và khi index rỗng sau `ready`; `indexing` có `progress` và `eta_s`; `model_missing`; lỗi từ xa; chết trước `ready` thì `broken` sau đúng 3 lần chạy; treo không `ready` thì bị kill rồi `broken`; chết sau `ready` thì được khởi động lại; `kill -9` (Windows: `taskkill /F`) thì được khởi động lại và tiến trình cũ không còn; ngân sách khởi động lại được nạp lại; `shutdown` và `Drop` không để lại tiến trình; tiến trình bỏ qua EOF thì bị kill sau `shutdown_timeout`.
- OHS thật (`PKM_OHS_DIR=…/p04/pkg/ohs-0.17.2-vantt.1-linux-x64`, `PKM_OHS_TEST_VAULT=…/p04/kg-solo`, DB tạm): qua trong 4,6 s. Có 5 câu tìm đồng thời; mọi đường dẫn đều NFC và có trong vault. Note thử tên NFD đứng đầu với câu `quyxuyenzorblat`, ở dạng NFC. Sau `shutdown`, node không còn.
- `ps` sau các lần chạy không còn `fake-ohs` hay `serve --stdio` nào do test khởi động. Trước đó máy có một node `serve --stdio` từ `ohs-wt` của người dùng; tiến trình này không do test tạo và không bị đụng tới.

## Còn mở

1. Nối `ohs_not_ready` vào `tools` (checkbox phase 11 chưa tick): làm cùng `related`. `related` chỉ có câu hỏi gọi `Ohs::search` và đổi `NotReady` thành lỗi tool. Python còn chế độ `note` (`search --path`); `options` của `search` được chuyển nguyên cho OHS, nên `related` cần kiểm tên tuỳ chọn stdio tương ứng.
2. Thư mục gói mặc định chọn là `<pkm_dir>/ohs`. Bộ cài (phase 13) phải đặt gói vào đây, hoặc đổi `DEFAULT_DIR_NAME`.
3. Vault rỗng thì trạng thái đứng mãi ở `starting` (đã ghi trong báo cáo bài thử S3). Chưa xử lý vì cần tín hiệu "quét xong" từ fork.
4. Khi tắt trong lúc luồng giám sát đang chờ `status`, việc tắt chậm thêm tối đa `status_timeout` (5 s).
