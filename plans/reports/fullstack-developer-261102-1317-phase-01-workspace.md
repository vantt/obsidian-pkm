# Báo cáo: khung workspace Cargo (R00)

- Phase: `plans/261102-0535-rust-core-port-master/phase-01-workspace-skeleton.md`. Trạng thái: **xong**. CI ba nền chờ lead push rồi kiểm.
- Ngày: 2026-11-02.

## Đã làm
- Workspace hai crate (`resolver = "3"`, edition 2024, `publish = false`):
  - `pkm-core` (lib) khai báo sẵn mười module rỗng: `vault`, `schema`, `index`, `validate`, `query`, `tools`, `related`, `adapter`, `ohs`, `platform`.
  - `pkm` (bin) dùng `clap`, có `cli.rs` và `mcp.rs` rỗng.
- `crates/pkm/build.rs` đọc `[spec].sha` trong `spec/spec.lock`, dựng đường dẫn bằng `Path::join` (chạy được trên mọi nền). Build dừng nếu sha thiếu hoặc không phải 40 ký tự hex. `pkm --version` in ra `pkm 0.1.0 (spec 5a3eb2f7990cd093b2ee3a1ccd5b46bd46e3ca4e)`.
- Test `crates/pkm/tests/version.rs` chạy binary và so kết quả với sha đọc từ `spec.lock`.
- `rust-toolchain.toml` ghim `1.97.1` (kèm `rustfmt`, `clippy`).
- `.github/workflows/ci.yml` chạy trên ubuntu, macos, windows-latest: `actions/checkout@v5`, `rustup toolchain install` (đọc `rust-toolchain.toml`), `Swatinem/rust-cache@v2`, rồi `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`. Không cần token, không cần Python.
- `docs/DECISIONS.md`: khung, mục Q3 và Q4 để trống, bốn quyết định đã chốt (toolchain, `clap`, cách lấy sha cho `--version`, seed).
- `spec/PORTING.md`: bảng nhật ký và mục "Lệch có chủ ý".
- `.gitignore` (`/target`). `Cargo.lock` được commit vì repo build ra binary.

## Seed (bằng chứng)
`spec.lock` ghi `[fixture].seed = 11`.
- Repo đặc tả ở `5a3eb2f`: lớp `Fixture` trong `scripts/kg/tests/test_kg.py` chạy `node otf.mjs generate --preset kg-solo --out …` mà không truyền `--seed`.
- Fixture `v0.7.0` (`2a514a8`): `presets/kg-solo.mjs` dòng 14 ghi `seed: { default: 11 }`, dòng 97 ghi `build({ seed = 11, clients = 30, days = 365, … })`.
- Đối chiếu thêm: `docs/knowledge-graph/bench/r2/README.md` mục 1 của repo đặc tả ghi "hạt giống 11".
- Seed được ghi dạng số nguyên TOML, không phải chuỗi.

## Kiểm
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` đều qua trên Linux (1 test).
- `cargo run -p pkm -- --version` in ra đúng sha.

## Vấn đề gặp
- Lần build đầu kích hoạt rustup cài `1.97.1` nhưng hỏng giữa chừng. Ở lần sau, toolchain này bị gỡ trong khi phiên của tôi không chạy lệnh gỡ nào. Có vẻ một tiến trình khác trên máy cũng đang đụng `~/.rustup`. Đã gỡ và cài lại một lần; bây giờ chạy ổn.
- Bản clone repo đặc tả nằm ở `/tmp/claude-1000/-home-vantt-projects-obsidian-pkm/2dfb5915-3011-441f-a6fb-2c7cb5abd60b/scratchpad/spec-repo`, đang checkout `5a3eb2f`. Bản clone fixture `v0.7.0` nằm ở `…/scratchpad/otf-v0.7.0`. Không commit cả hai.
- `README.md` vẫn ghi "Chưa có mã". File này ngoài phạm vi phase, nên tôi không sửa.

## Câu hỏi còn mở
- Không có.
