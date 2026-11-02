# Đưa nhánh `ohs-release-pin` vào `main`

Ngày: 2026-11-02. Kết quả: merge commit `0038144` trên `main` đưa vào cả ba commit của `origin/ohs-release-pin` (tip `f816962`), một commit sửa (`2d000c2`) và bản cherry-pick ghim OHS `v0.17.2-vantt.4` (từ `3dd81f0` của `origin/phase-17-one-door`). rc.2 sau này sẽ có các thay đổi này.

## Review

Đạt, không cần sửa:

- `--ohs DIR`: `OhsPackage::from_dir` kiểm gói và đưa về đường dẫn tuyệt đối trước khi ghi; không phải UTF-8 thì báo lỗi; `serde_json` thoát `\` nên `C:\tools\ohs` ghi đúng vào JSON. `--ohs` xung đột với `--ohs-package` và `--no-ohs`. `reinstall_args` giữ `--ohs`, `rebuild_args` bỏ nó. `pkm VAULT mcp` có cờ `--ohs` nên mục `kg` chạy được.
- Ghi cấu hình vẫn theo CAS (đường ghi trong `run_with` không đổi). `doctor` vẫn chỉ đọc: chỉ đọc `MANIFEST.json` và mục `kg`.
- `language_pack.loaded=false` trả `OhsState::Broken`, đi qua đường `not_ready_error` sẵn có nên vẫn đủ 5 khoá.
- `PACK_IGNORE_ON=true` gửi `OBSIDIAN_IGNORE_PATTERNS='.obsidian/**,templates/**,*.canvas,_ohs/**'`, nhưng chỉ khi gói có `lang/ohs-lang-vi`; fake OHS test cả hai trường hợp.
- Thay gói dùng `swap_into` (đổi tên rồi mới xoá gói cũ), nên tải hỏng thì gói cũ vẫn còn.

Lỗi thật đã sửa trong `2d000c2`, "fix(install): upgrade only OHS packages older than the pin, never downgrade":

- Nhánh nâng gói ở `<thư mục pkm>/ohs` mỗi khi `version` **khác** bản ghim. Vì vậy một gói **mới hơn** bị hạ cấp ở lần `pkm mcp install` sau. Ví dụ: trước khi nâng ghim, `main` ghim `vantt.3`; máy có `vantt.4` ở thư mục mặc định sẽ bị đưa về `vantt.3`. Đã thêm `ohs_bundle::older_than_pin`, so theo dạng `X.Y.Z[-vantt.N]`. Gói không có phiên bản thì coi là cũ và được nâng. Gói bằng bản ghim, mới hơn, hoặc có phiên bản không so được thì giữ nguyên.
- `doctor` cũng sửa theo. Gói cũ hơn bản ghim thì báo `warn`. Gói mới hơn thì báo `ok`, không còn khuyên "nâng". Lệnh sửa `mcp install` chỉ gợi ý khi gói nằm ở thư mục mặc định, vì `mcp install` không bao giờ nâng gói chỉ bằng `--ohs` hay `PKM_OHS_DIR`.
- Gói chỉ bằng `--ohs DIR` không bao giờ bị ghi đè. Code đã đúng từ trước (nhánh `--ohs` trả về trước mọi bước cài); nay có test cho trường hợp gói của người dùng cũ hơn bản ghim mà `MANIFEST.json` vẫn giữ nguyên.
- Test mới hoặc mở rộng: `install::ohs_bundle::tests::older_than_pin_never_counts_newer_or_unknown_forms_as_old`; `install::tests::ohs_older_than_the_pin_is_upgraded_and_the_pin_or_newer_is_kept` (thêm trường hợp gói mới hơn được giữ); `install::tests::ohs_flag_points_kg_at_an_existing_package_without_installing_one` (thêm trường hợp gói cũ của người dùng giữ nguyên); `doctor::tests::ohs_package_older_than_the_pin_or_without_vietnamese_pack_warns` (thêm `ok` cho gói mới hơn, và không có `fix` khi gói chỉ bằng `--ohs`).
- Docs: đã cập nhật DECISIONS và `spec/PORTING.md` theo quy tắc "cũ hơn bản ghim".

## Nâng ghim lên `v0.17.2-vantt.4`

Đã cherry-pick `3dd81f0`, gồm sha256 bốn nền tảng, README, test install và mục DECISIONS. Xung đột chỉ ở `docs/DECISIONS.md`, đã giữ cả hai mục; dòng "Còn mở" của mục OHS nay ghi là đã nâng ghim. Trong `handoff-windows-test.md` (mục A5, A7) và `phase-13-install-and-doctor.md` đã đổi `vantt.3` thành `vantt.4`. Commit sau, cùng commit với báo cáo này, đổi sha256 win32-x64 ở mục A5 sang `5cb480f8…` của `vantt.4`, xử lý ghi chú "handoff vẫn ghi ghim cũ" trong `fullstack-developer-261102-2138-windows-report-fixes.md`.

## Kiểm

Đã chạy trên trạng thái merge: `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` và `cargo test --workspace --locked` đều qua. Parity chạy đúng như CI (vault `kg-solo`, fixture `v0.7.0` seed 11, dump ra thư mục tạm) cho kết quả 64 mục, 0 lệch chưa có tên.

CI trên `main`: run của merge (`41464677591`) và của commit kế (`41464799859`) bị huỷ vì agent khác push docs chồng lên. Trong run `41464799859`, các job ubuntu, windows và asset linux/windows đã qua trên code có merge; ba job macOS bị huỷ khi còn chờ máy. Kết quả trọn bộ xem ở run của commit chứa báo cáo này.

## Agent deploy Windows cần biết

- **Không cần rebase** `ohs-release-pin` nữa: nhánh đã vào `main`. Làm tiếp từ `main` (hoặc từ rc.2 khi có). Có thể xoá nhánh đó.
- `main` nay ghim `0.17.2-vantt.4`, bằng gói trên bản B ở `C:\tools\ohs`. Vì thế `doctor` phải báo `ohs_package` là `ok` với `C:\tools\ohs (0.17.2-vantt.4, có gói tiếng Việt)`.
- Cài với `--ohs C:\tools\ohs`: `pkm` chỉ ghi `--ohs` vào mục `kg`, không cài gì vào `%LOCALAPPDATA%\pkm\ohs` và không bao giờ đụng vào `C:\tools\ohs`.
- Nếu `%LOCALAPPDATA%\pkm\ohs` có gói từ lần cài trước: gói cũ hơn `vantt.4` sẽ được nâng ở lần `mcp install` sau (khi không dùng `--ohs`); gói bằng hay mới hơn thì giữ nguyên.
