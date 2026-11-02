# Đuổi theo `223c9c3`: OHS chạy với gói tiếng Việt

Ngày 2026-11-02. Một lượt đuổi theo (phase 15), user cho chạy.

## Kết quả

`223c9c3` đã chuyển. `spec.lock` ghim `223c9c33c99c6a087d474b66da78736ad18f849c` (trước: `5a3eb2f`), golden dựng lại ở `parity/golden/223c9c33c99c6a087d474b66da78736ad18f849c/`, thư mục golden cũ đã xoá. `python3 parity/follow.py`: 0 commit chưa chuyển. `223c9c3` là đầu `origin/main` của repo đặc tả lúc làm (sau `fetch`), nên không có commit nào sau nó, trong hay ngoài đường dẫn theo dõi. Commit trước nó và sau `5a3eb2f` là `aaffe2e` (chỉ docs, ngoài đường dẫn theo dõi).

## Từng phần của commit Python

| Phần | Xử lý |
|---|---|
| `platform_paths.ohs_env`: đặt `OHS_LANGUAGE_PACK` cho mọi tiến trình của gói | **Đã chuyển.** `OhsPackage` có thêm trường `pack: Option<PathBuf>` (thư mục `lang/ohs-lang-vi` khi có), `ohs::LANGUAGE_PACK_ENV`; `Ohs::command` đặt biến khi gói có pack, không có thì để nguyên môi trường. Mọi đường chạy OHS của `pkm` (MCP server, `pkm VAULT related`, `doctor --deep`) đi qua đây |
| `ohs_env`: `WSLENV` khi chạy node của Windows từ WSL | Không áp dụng (WSL, PRD mục 2.2) |
| `platform_paths.ohs_install`, nhánh gói fork | Đã có từ trước: `OhsPackage::from_dir`; chỉ thêm `pack` |
| `ohs_install`, nhánh bản cài npm (`node_modules/…`, node trên PATH) | Không áp dụng: Rust chỉ chạy gói của fork có Node đi kèm (PRD mục 9) |
| `related.ohs_seeds`: node của gói | Đã có từ trước (module `ohs` chạy node của gói) |
| `desktop_config.entry`: `--ohs-node` là node của gói | Đã có từ trước: mục `kg` của Rust không mang `--ohs-node`; `pkm mcp install` cài gói có Node |
| Hằng `OHS_CLI` mới (`C:\tools\ohs\app\dist\src\cli.js`) trong `graph_index.py`, `desktop_config.py` | Không áp dụng: Rust tìm gói qua `--ohs`, `PKM_OHS_DIR`, thư mục cài; `--ohs-cli` cũ trỏ vào `app/dist/src/cli.js` đã được nhận (`mcp::legacy_package`) |
| `setup_vault.py`, `.mjs`, docs playbook | Ngoài đường dẫn theo dõi |

Không có lệch có chủ ý mới. Không nâng gói ghim của bộ cài: artifact ghim `0.17.2-vantt.1` (run `41398112909`) đã có `lang/ohs-lang-vi` (`vi 0.2.0`); đã tải `bundle-linux-x64`, sha256 gói `9160d45e…` khớp bản ghim. `bin/ohs` của gói không tự đặt biến, nên `pkm` phải đặt.

## Kiểm

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`: sạch.
- `cargo test --workspace --locked`: qua hết. Test mới: `ohs::package::tests::vietnamese_pack_is_found_only_as_a_folder`, `tests/ohs_fake.rs::ohs_runs_with_the_vietnamese_pack_of_its_package`; `ohs_is_started_with_vault_db_and_clean_environment` kiểm thêm không có biến khi gói không có pack. Server giả ghi lại `OHS_LANGUAGE_PACK` nhận được.
- Gói thật (`0.17.2-vantt.1` linux-x64): `tests/ohs_real.rs` qua; chạy gói trên kg-solo với biến thì `status` báo `language_pack.loaded: true`, `id: vi`, `version: 0.2.0`.
- Golden ở `223c9c3`: dump Python hai lần khớp, `fixture_check` graph 70/70, table 11/11; khác golden cũ đúng một dòng `spec_sha` trong `manifest.json`.
- `python3 parity/parity.py run --against python`: 64 mục, 0 lệch chưa có tên, 0 mục chưa có.
- Rust với golden mới, chạy như CI (vault dựng bằng `npx … --out <tmp>/kg-solo`, `parity check --dump-out <tmp>/rust-dump`): 64 mục, 0 lệch chưa có tên, 0 mục chưa có, mã 0.
- CI và `parity/` không ghi cứng sha (đọc `spec.lock`), nên không sửa.

## File đổi

- `crates/pkm-core/src/ohs.rs`, `crates/pkm-core/src/ohs/package.rs`, `crates/pkm-core/tests/{ohs_fake.rs, related_ohs_fake.rs, support/fake_ohs.rs}`
- `spec/spec.lock`, `spec/PORTING.md` (dòng nhật ký, mục `ohs`, bảng test `platform` thêm 4 test `OhsInstall`, mục bộ cài)
- `parity/golden/` (thay thư mục golden)
- `docs/DECISIONS.md` (mục "Gói tiếng Việt của OHS")
- `plans/261102-0535-rust-core-port-master/phase-15-chase-python-commits.md`

## Câu hỏi còn mở

- Release `v0.17.2-vantt.1`…`vantt.3` của fork nay đã có file gói, trong khi bộ cài vẫn ghim artifact của run CI (hết hạn 2027-01-31). Chuyển bộ cài sang Release (và có nâng lên `vantt.2`/`vantt.3` không) là việc riêng, cần user chọn.
- Dòng phase 15 trong bảng trạng thái của `plan.md` ("Chưa bắt đầu, chờ user") chưa sửa, vì ô trạng thái không sửa tay; lead cập nhật qua công cụ kế hoạch.
