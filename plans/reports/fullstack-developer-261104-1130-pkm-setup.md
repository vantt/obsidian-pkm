# Báo cáo: `pkm setup` và plugin trong Release (bước 4)

Nhánh `product-b-setup` (từ `origin/product-b` `87eb6d8`, đã merge `origin/product-b` `f2930c7`). Commit: `e5f1b1e` (lệnh), `d78b154` (khoá `graphDbPath`), `e93a34d` (CI, Release), commit docs cuối nhánh.

## Đã làm

- `pkm setup VAULT` (`crates/pkm/src/setup.rs`, `setup/preset.rs`, `setup/files.rs`): chạy thử mặc định, `--apply`, `--remove [--apply]`, `--json`, `--plugin-dir`, `--ohs`, `--no-ohs`, `--config`, `--force`, `--allow-any-location`. Sáu bước theo thứ tự của `setup_vault.py`; bước `mcp` gọi thẳng `install::run_with` với cờ đọc của bản B (không chép mã). Hooks thử được (`SetupHooks: Hooks`) như `install`.
- Preset nhúng (`setup.json`, `vault-agent.md`, `theme/e11-theme.css`). `setup.json`: bỏ snippet `e11-folder-grid` (plugin có sẵn), thêm `theme.style_settings_plugin`.
- `data.json` của plugin: `pkmPath`, `graphDbPath` (theo khoá của `apps/obsidian-plugin/src/settings.ts`), chỉ khi chưa có.
- Release: `scripts/release-assets.sh plugin <dist> <out>` ra `pkm-obsidian-plugin.zip` (kiểm id và phiên bản); `release-assets.yml` dựng plugin trên job Linux (cả ma trận rút gọn và đầy đủ), chạy thử `pkm setup --json` với zip đó, đẩy artifact `pkm-obsidian-plugin`; `release.yml` gộp vào `checksums.txt` (7 asset + checksums). `ci.yml` có job `plugin` (install frozen, typecheck, test, build).
- Docs: README "Dựng bản B", `docs/user-guide/README.md`, `presets/b/README.md`, `docs/DECISIONS.md` (mục 2026-11-04 `pkm setup`), dòng bước 4 trong plan.

## Kiểm

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -D warnings`, `cargo test --workspace --locked`: xanh (19 unit test `setup::tests`, 2 integration test `cli_setup`).
- `pnpm --filter pkm-obsidian-plugin test` (63) và `build`: xanh. `release-assets.sh plugin` chạy tay: zip đủ ba file, từ chối manifest khác phiên bản.
- Chạy tay binary trên vault tạm (HOME, PATH riêng): chạy thử, `--apply`, chạy lại (không đổi gì), `--remove --apply` (file đã sửa giữ).
- Chưa chạy: workflow trên GitHub (không có `actionlint`, `shellcheck` ở máy này; YAML parse được), Release thật, máy sạch Windows/macOS.

## Còn mở

- Theme Primary và Style Settings: chỉ cảnh báo, chờ user quyết có tải ghim không.
- Không có Claude Desktop trên máy (không `--config`) thì bước `mcp` dừng lệnh với câu của `mcp install`; bước viết bài không chạy. Có nên cho bản B chạy không Claude Desktop không là việc user quyết.
