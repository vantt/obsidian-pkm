# Sửa `pkm doctor` và `pkm mcp install` theo audit đóng gói

- **Ngày:** 2026-11-02 (Asia/Saigon)
- **Nhánh:** `worktree-agent-adf22cef46fb9ce43`, tách từ `main` ở `e9abb8a`. Chưa push, chưa merge.
- **Nguồn:** `plans/reports/packaging-audit-261102-1505-cs-packaging-distribution.md` (B2 phần 2, I1 phần biến môi trường, I2, I3, I4, I5). Skill `cs-packaging-distribution`: `common/60-doctor.md`, `common/07-command-surface.md` (CMD-11), `common/30-install.md` (INS-09).
- **Trạng thái:** xong. `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` và `cargo test --workspace` đều qua.

## Đã làm

| Việc | Kết quả | Test |
|---|---|---|
| Binary ngoài chỗ cài chuẩn | Binary ở chỗ cài ổn định là binary nằm ở một trong ba chỗ: thư mục chuẩn (hằng `STANDARD_BIN_DIR` trong `install.rs`: `%LOCALAPPDATA%\pkm\bin` trên Windows, `~/.local/bin` nơi khác), `PKM_INSTALL_DIR`, hoặc thư mục có `.pkm-managed`. Ba chỗ này cùng mặc định, cùng tên biến và cùng file đánh dấu với `install.ps1`/`install.sh` ở worktree bộ cài. Binary ở chỗ khác thì `mcp install` từ chối trừ khi có `--force`. Lời từ chối nói rõ khi file nằm trong Downloads hay thư mục tạm, rồi in lệnh bộ cài qua `gh release download` (repo riêng tư) cùng lệnh chạy lại từ chỗ chuẩn, với đủ vault và cờ. `--dry-run` chỉ in cảnh báo. | `refuses_binary_outside_install_dir_without_force`, `installer_marker_or_install_dir_makes_binary_location_stable` |
| Thiếu gói OHS | Là `warn`, kèm câu "không có OHS thì chỉ `related` khi không có `hits` bị ảnh hưởng". Nếu cài với `--no-ohs` thì là `skip`: lúc cài, mục `kg` được ghi `env.PKM_NO_OHS = "1"`, và lần cài sau không có `--no-ohs` thì biến này bị bỏ. | `missing_ohs_package_only_warns_and_explains_the_impact`, `missing_ohs_package_is_skipped_when_installed_with_no_ohs`, `no_ohs_is_recorded_in_the_entry_and_dropped_on_a_later_install` |
| `doctor` chỉ đọc | Mặc định chỉ kiểm gói có đủ file và `node --version` (mục mới `ohs_node`, qua hook `Hooks::node_version`). Chạy thử OHS chỉ khi có `--deep`; cờ `--no-ohs-start` bị bỏ. Mọi mục OHS chỉ tới `warn`. | `healthy_machine_passes_every_check_without_starting_ohs`, `deep_runs_ohs_on_the_entry_vault`, `broken_node_and_every_ohs_state_only_warn`, `doctor_args_parse_deep_flag` |
| `doctor --json` | Envelope `{schema: 1, ok, data: {checks: [{id, status, detail, action, fix}]}, error, next}`. `error` là `null` hoặc `{code: "checks_failed", why, fix}`. `fix` dựng từ `args` của mục `kg`: `mcp install` lại, hoặc `VAULT build --db DB <cờ đọc>`. Lệnh chạy bằng binary của `kg` nếu còn, không thì bằng binary đang chạy doctor, và được quote cho PowerShell (Windows) hay `sh` (`install/shell.rs`). `next` gồm `fix` của các mục `fail`. Hợp đồng ghi ở `spec/PORTING.md` và `docs/DECISIONS.md`. | `json_envelope_has_stable_keys`, `every_problem_has_an_action_and_fail_fixes_go_to_next`, `kg_entry_with_missing_command_or_vault_fails`, `explicit_db_overrides_entry_db`, `shell::tests::*`, `tests/cli_commands.rs::doctor_reports_json_and_fails_without_config_entry` |
| So rồi mới ghi | Lưu sha256 của bytes cấu hình đã đọc. Dựng index xong thì hỏi lại Claude Desktop (trừ khi `--force`), đọc lại file và so digest. Khác thì nạp lại, đặt lại mục `kg` và báo cho người dùng. Nếu Desktop đã mở thì dừng với câu "Index đã dựng xong; cấu hình Claude Desktop chưa đổi". | `rereads_config_changed_while_building_and_keeps_that_change`, `refuses_to_write_when_desktop_starts_during_build` |
| Nguồn gói OHS đổi bằng biến | `PKM_OHS_PACKAGE` thay việc tự tải artifact ghim. Giá trị là đường dẫn, hoặc URL `https://` của file gói, kèm `<URL>.sha256`. URL tải bằng `curl --proto =https` qua `Hooks::fetch`. `--ohs-package` cũng nhận URL; `http://` và các scheme khác bị từ chối. Nguồn từ biến lỗi thì chỉ cảnh báo, như khi tự tải lỗi. `--dry-run` nêu nguồn. | `ohs_source_env_installs_package_from_https_url`, `ohs_package_flag_rejects_plain_http`, `ohs_bundle::tests::{installs_from_https_url_checked_against_its_sha256_file, rejects_url_package_with_wrong_or_missing_sha256, source_parse_accepts_https_urls_and_paths_only}` |
| Ghi chú cho bộ so khớp | Thêm vào `parity/README.md` (mục CI): bộ so khớp cần binary `pkm` đã dựng; CI dựa vào bước `cargo test --workspace` chạy trước; không có binary thì tầng MCP/CLI báo "chưa có". | — |

Mọi test dùng `Hooks` giả và đường dẫn `--config` trong thư mục tạm. Không test nào đụng cấu hình Claude Desktop thật, mạng, `gh`, hay OHS thật.

## File đã sửa

- `crates/pkm/src/install.rs`: hằng và hàm về chỗ cài, `Install::{bin_dirs, ohs_source, exe_location_warning}`, `reinstall_args`, `rebuild_args`, `installer_command`, so digest trước khi ghi, nguồn OHS, `Hooks::{fetch, node_version}`.
- `crates/pkm/src/install/shell.rs` (mới, 76 dòng): quote dòng lệnh cho PowerShell và `sh`.
- `crates/pkm/src/install/ohs_bundle.rs`: `Source::Url`, `Source::parse`, `PACKAGE_ENV`, `curl`. `install` nhận thêm hàm tải.
- `crates/pkm/src/doctor.rs` và `doctor/tests.rs`: viết lại như trên.
- `crates/pkm/src/cli.rs`: chỉ dòng trợ giúp `pkm doctor [--json] [--deep]`.
- `crates/pkm/tests/cli_commands.rs`: test doctor bỏ `--no-ohs-start` và đọc envelope mới. Đây là test của doctor, nằm ngoài `src/`.
- `crates/pkm/src/install/tests.rs`.
- `spec/PORTING.md` (thêm 6 gạch đầu dòng vào mục bộ cài), `docs/DECISIONS.md` (thêm 2 mục), `parity/README.md` (thêm 1 đoạn).

## Lệch so với lời giao và lý do

- **`--force` dùng cho hai việc:** bỏ qua kiểm Claude Desktop và chấp nhận binary ngoài chỗ chuẩn. Làm vậy vì user giữ bề mặt lệnh của PRD. Lời từ chối nói rõ `--force` cũng bỏ qua kiểm Desktop.
- **Nguồn từ `PKM_OHS_PACKAGE` lỗi thì chỉ cảnh báo, không dừng:** biến này thay cho việc tự tải, và tự tải lỗi xưa nay chỉ là cảnh báo. Còn `--ohs-package` lỗi thì vẫn dừng như trước.
- **Mục `long_paths` có `fix` là lệnh PowerShell cần quyền admin:** đây là `warn`, nên lệnh này không nằm trong `next`.

## Câu hỏi còn mở

- **Nguồn URL kiểm bằng `.sha256` cùng chỗ với gói** (cùng mức tin như asset của Release), không bằng digest ghim trong binary. Nếu muốn chặt hơn, có thể thêm `PKM_OHS_SHA256`, nhưng khi đó đổi gói lại phải đặt thêm một biến.
- **Lệnh trong `fix` theo shell mặc định của nền tảng** (PowerShell trên Windows). Agent chạy Git Bash trên Windows sẽ không dán chạy được dòng có `& '…'`. Nếu cần thì thêm một trường riêng cho từng shell.
