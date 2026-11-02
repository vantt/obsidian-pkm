# Nguồn gói OHS: GitHub Release `v0.17.2-vantt.3`; phiên bản `0.1.0-rc.1`

Ngày 2026-11-02. Quyết định của user: ghim `vantt.3`, tag `v0.1.0-rc.1`.

## Kết quả

- `pkm mcp install` tự tải `ohs-0.17.2-vantt.3-<nền tảng>` từ Release công khai của `vantt/obsidian-hybrid-search` bằng `curl` (https, không đăng nhập), kiểm sha256 ghim trong binary. Bỏ hẳn đường `gh` và dạng zip artifact. Giữ `--ohs-package` (file gói khớp sha ghim, thư mục, URL kèm `.sha256`) và `PKM_OHS_PACKAGE`. Nền tảng không có gói thì báo rõ bốn nền tảng có gói.
- `pkm doctor`: lời hướng dẫn cài OHS nói tải từ Release, không cần đăng nhập, kèm trang Release cho file tải tay.
- Phiên bản workspace `0.1.0-rc.1`. Giữ kiểm "tag == phiên bản crate" của `release.yml`, vì bộ cài so `pkm --version` với tag. `serverInfo.version` của MCP là hằng `"0.1.0"`, nên so khớp không đổi.
- Lệnh bộ cài mà `pkm` in khi binary nằm ngoài chỗ cài chuẩn nay ghi tag của chính bản đang chạy. Lý do: `gh release download` không ghi tag thì lấy bản "latest", mà pre-release không bao giờ là "latest". README và handoff cũng ghi tag `v0.1.0-rc.1`.

## Bằng chứng

- Bốn asset tải ẩn danh. Sha256 đo được khớp file `.sha256` của fork: win32-x64 `ad63f1a5…`, darwin-arm64 `8c32b8aa…`, darwin-x64 `f67d90f7…`, linux-x64 `a4178475…`. Smoke của fork qua trên cả bốn nền tảng.
- Gói linux-x64 có `node/`, `app/`, `bin/`, `lang/ohs-lang-vi`. `tests/ohs_real.rs` và `tests/related_ohs_real.rs` (gói thật, kg-solo) qua. `status` báo `language_pack.loaded: true` (vi 0.2.0). Sau đó không còn tiến trình `node` mới nào.
- Test tải thật `downloads_and_installs_pinned_release_asset` (`#[ignore]`) qua.
- `release-assets.sh package` cho linux in `pkm 0.1.0-rc.1 (spec 223c9c3…)`. `test-installer.sh <out> v0.1.0-rc.1` qua mọi bước.
- `cargo fmt --check`, `clippy -D warnings`, `cargo test --workspace --locked` đều qua. So khớp như CI: 64 mục, 0 lệch chưa có tên, 0 mục chưa có.

## File đổi

`Cargo.toml`, `Cargo.lock`, `crates/pkm/src/install/ohs_bundle.rs`, `crates/pkm/src/install.rs`, `crates/pkm/src/install/tests.rs`, `crates/pkm/src/doctor.rs`, `docs/DECISIONS.md` (hai mục mới), `spec/PORTING.md`, `README.md`, `plans/261102-0535-rust-core-port-master/handoff-windows-test.md`.

## Còn mở

- Phần B của handoff vẫn clone đặc tả ở `5a3eb2f` (`spec-5a3eb2f`), trong khi spec ghim nay là `223c9c3`. Việc này nằm ngoài phạm vi lần này nên chưa sửa.
- Gói darwin và win32 mới chỉ kiểm sha256. Chưa chạy thật trên macOS hay Windows.
