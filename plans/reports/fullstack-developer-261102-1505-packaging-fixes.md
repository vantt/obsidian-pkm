# Sửa blocker đóng gói và pipeline phát hành

- **Ngày:** 2026-11-02 (Asia/Saigon)
- **Nhánh:** `worktree-agent-a1f9526676784076c`, từ `main` ở `7d14601`
- **Đầu vào:** `plans/reports/packaging-audit-261102-1505-cs-packaging-distribution.md` (B1–B3, I6, I7, một phần I8, I9), skill `cs-packaging-distribution`, ràng buộc của user: repo giữ private, cài qua `gh`.
- **Trạng thái:** xong. Phần chạy trên Windows và macOS chỉ kiểm được trên CI (xem "Chưa kiểm được").

## Đã làm

| Audit | Việc | File |
|---|---|---|
| B1 | CRT tĩnh cho `x86_64-pc-windows-msvc`; mỗi lần đóng gói chạy `dumpbin -dependents` và dừng nếu còn `vcruntime*`, `msvcp*`, `ucrtbase*`, `api-ms-win-crt-*` | `.cargo/config.toml`, `scripts/release-assets.sh` |
| B2 | `install.ps1` và `install.sh`: kiểm sha256 theo `checksums.txt`, chạy thử `--version`, cài vào `%LOCALAPPDATA%\pkm\bin` / `~/.local/bin`, thêm PATH người dùng, cập nhật khi chạy lại (cùng bản thì không làm gì; `pkm.exe` đang chạy được đổi tên `.old-*`), `--uninstall`, `--mcp-install <vault>`, `--dry-run`, marker `.pkm-managed`, từ chối đè binary lạ, từ chối admin/root | `scripts/install.{sh,ps1}` |
| user | Tải công khai trước, gặp 401/403/404 thì dùng `gh release download`; thiếu `gh` hay chưa đăng nhập thì báo cách sửa. Đổi nguồn: `PKM_RELEASE_REPO`/`--repo`/`-Repo`, `PKM_RELEASE_BASE_URL`/`--release-base-url`/`-ReleaseBaseUrl` | như trên |
| B3 | README: tagline, lời hứa, bắt đầu nhanh bằng `gh`, `pkm mcp install`, `pkm doctor`, cập nhật/gỡ, SmartScreen; dòng trạng thái đúng hiện trạng | `README.md` |
| I6 | `verify` (tag == `v`+phiên bản crate) → gọi lại toàn bộ `ci.yml` → `package` (đóng dấu bộ cài, `checksums.txt` gộp, kiểm đủ 6 file) → `publish` chỉ khi có tag, tag có `-` thành pre-release. `workflow_dispatch` dừng ở artifact `release-assets` | `.github/workflows/release.yml` |
| I6 | Action ghim SHA, đã kiểm bằng `gh api`: checkout v7.0.1 `3d3c42e`, upload-artifact v7.0.2 `cf430e0`, download-artifact v8.0.2 `9000827`, rust-cache v2.9.3 `d1527c8`. Tag có sẵn trong `ci.yml` (`checkout@v5`, `setup-node@v7`, `rust-cache@v2`) cũng tồn tại | release.yml, release-assets.yml |
| I7 | Workflow tái dùng dựng bốn đích ở chế độ release trên mọi PR, chạy thử binary trên runner của nó, shellcheck, PSScriptAnalyzer (Windows PowerShell 5.1), thử vòng đời bộ cài | `.github/workflows/release-assets.yml`, `ci.yml` (thêm job `assets`) |
| I8 (1–3) | Chạy lại bộ cài là cập nhật; `-Uninstall`; README có cập nhật và gỡ | bộ cài, README |
| I9 | Bộ cài `Unblock-File` sau khi kiểm hash; README mục "Windows chặn pkm.exe" | `install.ps1`, README |

Tên asset giữ như cũ (`pkm-<target>[.exe]`), chỉ định nghĩa ở `scripts/release-assets.sh asset_name`; bộ cài dùng đúng tên đó. Bỏ các file `.sha256` riêng, thay bằng `checksums.txt`. Release có 7 file: 4 binary, `install.sh`, `install.ps1`, `checksums.txt`.

`ci.yml`: không đổi bước nào. Thêm trigger `workflow_call`, đổi nhóm `concurrency` từ `ci-${{ github.ref }}` thành `${{ github.workflow }}-${{ github.ref }}` (khi release gọi thì tên là "Release", nên không huỷ CI của main và không bị nó huỷ; với CI thường thì hành vi như cũ), thêm job `assets`. File mới ngoài danh sách được giao: `.github/workflows/release-assets.yml`, `scripts/release-assets.sh`, `scripts/test-installer.{sh,ps1}` (không agent nào khác đụng).

Quyết định ghi ở `docs/DECISIONS.md`: CRT tĩnh; "Repo private: cài qua gh, nguồn phát hành công khai tính sau"; bộ cài; pipeline phát hành.

## Kiểm

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`: qua.
- shellcheck 0.11.0 trên `scripts/*.sh`: sạch.
- `scripts/test-installer.sh` trên binary Linux release dựng tại máy: qua mọi bước (help, cờ lạ thoát 2, bản chưa đóng dấu từ chối, dry-run, cài, khối PATH nạp đúng, chạy lại không làm gì, checksum sai bị từ chối và giữ binary cũ, cập nhật, `--mcp-install` vault sai, gỡ không `--yes` khi không có TTY, gỡ, từ chối đè binary lạ). Chạy bằng `dash`.
- Đường private thật: `install.sh --version v0.1.0` bị 404 công khai, chuyển sang `gh` ("release not found", vì chưa có release); bỏ `gh` khỏi PATH thì dừng với câu "install gh … run 'gh auth login'".
- `release-assets.sh stamp` cho cả hai bộ cài và `checksums` (đúng số và sai số file): đúng.
- YAML ba workflow parse được.

## Chưa kiểm được trên máy này

- `install.ps1`, `test-installer.ps1`: hook cô lập worktree chặn chạy `pwsh`, nên chưa chạy lần nào. Bước "Thử vòng đời install.ps1" và "PSScriptAnalyzer" trên `windows-2025` sẽ là lần chạy đầu.
- Kiểm `dumpbin`, chạy thử Rosetta cho `x86_64-apple-darwin`, `install.sh` trên macOS: chỉ trên CI.

## Lead kiểm qua workflow_dispatch

Sau khi merge nhánh này (hoặc đẩy nhánh lên rồi chạy với `--ref`):

```sh
gh workflow run release.yml --repo vantt/obsidian-pkm --ref <nhánh hoặc main>
gh run list --repo vantt/obsidian-pkm --workflow release.yml --limit 1
gh run watch <run-id> --repo vantt/obsidian-pkm --exit-status
gh run download <run-id> --repo vantt/obsidian-pkm --name release-assets --dir /tmp/pkm-release-assets
(cd /tmp/pkm-release-assets && sha256sum -c checksums.txt && ls -1 | wc -l)   # 7 file
```

Trong log cần thấy: job `asset x86_64-pc-windows-msvc` in danh sách DLL không có `VCRUNTIME140.dll`; "OK: install.ps1 passed every step"; "OK: install.sh qua mọi bước" trên Linux và macOS arm64. `gh workflow run ci.yml --ref <nhánh>` cũng chạy job `assets`.

## Việc tiếp theo cho lead (cần sửa trong `crates/**`)

1. **`mcp install` cảnh báo khi chạy ngoài thư mục cài** (audit B2.2): `install.rs:182` ghi `current_exe()` vào cấu hình. Nếu exe nằm trong `Downloads` hay `%TEMP%` (ngoài `%LOCALAPPDATA%\pkm\bin`, `~/.local/bin`), in cảnh báo kèm lệnh cài đúng chỗ, hoặc từ chối nếu không có `--force`.
2. **Dọn `pkm.exe.old-*` khi khởi động** (INS-06): bộ cài đã tự dọn ở lần chạy sau; binary tự dọn thì gọn hơn khi người dùng không chạy lại bộ cài.
3. **`pkm VAULT mcp` và `mcp install`, `doctor` chưa nối vào CLI.** Khi nối xong, bước "update while the old pkm.exe is running" của `test-installer.ps1` mới thật sự giữ `pkm.exe` đang chạy (giờ chỉ in cảnh báo).
4. Các mục audit còn lại không thuộc phần này: I2 (`doctor` coi thiếu OHS là `warn`), I3, I4, I5, N1 (`[profile.release]`), N3.

## Câu hỏi chưa giải quyết

- Nhãn runner `macos-15` và `windows-2025` lấy theo hiểu biết hiện có, không kiểm được bằng API; nếu job xếp hàng mãi thì đổi về `macos-latest` / `windows-latest`.
- Chưa có release nháp và chạy thử bộ cài trên release thật trước khi công khai (REL-07); với repo private, bước đó cần `gh` có token trong job. Làm khi chọn nguồn phát hành công khai.
