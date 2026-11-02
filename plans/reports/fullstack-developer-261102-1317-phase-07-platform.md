# Báo cáo: module `platform` của `pkm-core`

- Phase: `plans/261102-0535-rust-core-port-master/phase-07-platform-module.md`
- Trạng thái: xong, có hai lệch có chủ ý đã ghi trong `spec/PORTING.md`.

## Đã làm

- `crates/pkm-core/src/platform.rs`:
  - `pkm_dir()`, `cache_dir(name)`: `%LOCALAPPDATA%\pkm` (rỗng thì `~\AppData\Local\pkm`), `~/Library/Application Support/pkm`, `$XDG_CACHE_HOME/pkm` hoặc `~/.cache/pkm`. Biến rỗng coi như không đặt (giống Python). Không có home thì trả lỗi `NotFound`.
  - `name_taken(dst, src)`: so tên theo NFC rồi casefold (crate `caseless`, giống `str.casefold`), bỏ qua chính `src` theo `normcase` (Windows: không phân biệt hoa thường, `/` là `\`). Thư mục chưa có thì `None`.
  - `move_file`, `move_file_with(src, dst, Retry)`: không đè; lỗi `AlreadyExists` với thông điệp `"{dst}: already taken by {clash}"` như Python; thử lại khi file bị giữ (`PermissionDenied`, thêm mã 32 và 33 trên Windows), chờ `wait * i`, mặc định 5 lần, 200 ms.
  - Windows: đổi tên bằng `MoveFileExW` không cờ, tiền tố `\\?\` (cả UNC) cho đường dẫn dài. Lý do: `std::fs::rename` của Rust đè đích trên Windows, còn `os.rename` của Python thì không.
- `crates/pkm-core/Cargo.toml`: thêm `caseless`, `unicode-normalization`; `windows-sys` (chỉ Windows, feature `Win32_Storage_FileSystem`); dev-dependency `tempfile`. `Cargo.lock` cập nhật theo.
- `spec/PORTING.md`: thay dòng "Chưa có mục nào." bằng hai mục lệch có chủ ý; thêm bảng 19 test Python và test Rust tương ứng.
- Phase file: đánh dấu xong các checkbox (bằng `ak plan check`), đặt `status: done`.

## Bảng 19 test (tóm tắt)

Chuyển được 5 test (cache dir, cùng 4 test `Move`; `test_never_overwrites` và các test thử lại). Có 11 test không áp dụng: 8 test chỉ dành cho WSL, `local_path` trên Linux và Windows, `node_command` bản native (module `ohs` thay thế), `open_uri` (chỉ E11 dùng) và `utf8_stdio`. Hai test thuộc `kg.vault` (`list_notes`, regex `BLOCK` trên note CRLF) để chuyển cùng module `vault`. Chi tiết từng dòng ở `spec/PORTING.md`.

## Kiểm tra

- `cargo test -p pkm-core platform`: 15 test qua trên Linux. Test riêng cho Windows và macOS dùng `#[cfg]`, CI sẽ chạy chúng.
- `cargo clippy -p pkm-core --all-targets --target x86_64-pc-windows-msvc` và `--target aarch64-apple-darwin` với `-D warnings` đều sạch. Lệnh này chỉ kiểm tra biên dịch, test chưa chạy trên Windows và macOS. Hai target này đã được thêm vào toolchain trên máy.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` và `cargo test --workspace --locked` đều qua.

## Lệch và câu hỏi còn mở

1. Thư mục cache có thêm `pkm` trên Linux và macOS, còn macOS dùng `Application Support`. Đây là theo PRD mục 7, còn Python thì không có thư mục `pkm`. Đề xuất mở issue ở repo đặc tả, nhưng cần user cho phép.
2. Với `tries=0`, Python không đổi tên mà cũng không báo lỗi. Rust vẫn thử một lần.
3. Phase file gợi ý crate `dirs`, nhưng code không dùng nó mà dùng `std::env::home_dir` cùng biến môi trường, đi theo cách Python làm. `caseless` không có trong danh sách crate gợi ý của PRD. Có cần ghi vào `docs/DECISIONS.md` không? File này nằm ngoài phạm vi được sửa của phase.
4. Bảng phase trong `plan.md` và kho plan `ak` (`ak plan phase close`) chưa được cập nhật, vì việc đó thuộc người điều phối.
