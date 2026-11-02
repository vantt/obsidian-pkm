# Sửa các mục review H2, M2, M3, M4, M5, L1, L2 (2026-11-02)

Nguồn: `plans/reports/code-reviewer-261102-1550-pkm-crate.md` (repo chính). Nhánh `worktree-agent-a261492f4edd12b04`, gốc `56c4b8b`. Không đụng `crates/pkm/src/{mcp,cli}*`.

## Kết quả từng mục

| Mục | Đã làm | Test |
|---|---|---|
| H2 | `Graph::paths`, `loops`, `all_loops` từ chối `max_len` > `MAX_LEN_CAP` (12) bằng `ValueError`. Mỗi lần liệt kê có hạn giờ (`WALK_BUDGET` 10 s, `PKM_QUERY_TIMEOUT` đổi được, `0` tắt, `Graph::set_walk_budget`): CTE bị ngắt bằng progress handler của SQLite (bật feature `hooks` của rusqlite, không đổi `Cargo.lock`), DFS của `all_loops` tự kiểm hạn; quá hạn trả `TimeoutError` "query too expensive ... lower max_len or narrow rels". Đặt ở tầng `query` để tool MCP, CLI và bộ câu đáp án cùng được bảo vệ. Trần 12 chọn vì golden dùng `max_len` tối đa 4 (tool), 4 (CLI), câu đáp án tối đa `maxLen` 10. Ghi "Lệch có chủ ý" trong `spec/PORTING.md`. | `query::tests::{max_len_above_cap_is_refused_before_running, walks_over_budget_stop_with_clear_error_and_graph_stays_usable, walk_budget_reads_seconds_and_zero_disables}` |
| M2 | `--force` chỉ bỏ qua kiểm Claude Desktop; thêm `--allow-any-location` cho binary ngoài chỗ chuẩn, cờ này không bỏ qua kiểm Desktop. Lời từ chối in exe bị coi là Desktop (`Hooks::desktop_process` thay `desktop_running`). `CLAUDE_CODE_DIRS` thêm `\microsoft\winget\links\`, `\microsoft\winget\packages\anthropic.claudecode`, `\scoop\apps\claude-code\`, `\programs\claude-code\` (và `claudecode`, `claude code`). Mẫu cụ thể, không so chuỗi con "code" bừa, nên `AnthropicClaude\app-*`, `WindowsApps\Claude_*`, `Programs\Claude\`, `scoop\apps\claude\` và `?` vẫn là Desktop. | `desktop::tests::*` (thêm ca winget, scoop, Programs, MS Store, `first_desktop_exe`), `install::tests::{refuses_to_write_while_desktop_runs, refuses_binary_outside_install_dir_without_allow_any_location}` |
| M3 | `write_atomic` ghi vào file đích của symlink (giữ symlink; symlink gãy thì từ chối), file tạm ở thư mục của file đích; Unix chép quyền của file cũ. Bản sao lưu vẫn đặt cạnh đường dẫn người dùng thấy. | `config::tests::{write_atomic_writes_through_symlink, write_atomic_keeps_permissions}` |
| M4 | File tạm `<file>.tmp-<pid>-<16 hex>` tạo bằng `create_new` (không mở file có sẵn, không theo symlink đặt sẵn), thử lại khi trùng; Unix tạo `0600` khi có file cũ. | `config::tests::temp_files_are_new_and_distinct` |
| M5 | Không phải lỗi. `index::build` chế độ full chỉ `DELETE FROM` các bảng trong giao dịch `IMMEDIATE` trên chính file DB (`crates/pkm-core/src/index.rs:768-772`); không có `remove_file`/`rename` nào trên DB trong `pkm-core` và `pkm` (chỉ có ở cấu hình Desktop và gói OHS). Không sửa `index.rs`. | `query::tests::full_rebuild_keeps_db_file_for_open_readers` (reader mở sẵn đọc được dữ liệu mới, inode không đổi) |
| L1 | `default_db` băm `vault_key`: đường dẫn đã `canonicalize`, bỏ `\\?\` (và `\\?\UNC\`), bỏ dấu phân cách cuối (trừ gốc), NFC, casefold trên Windows và macOS. Tên phần đầu lấy từ đường dẫn đã giải, NFC. Index theo cách băm cũ mà đã có thì dùng tiếp. | `install::tests::{default_db_ignores_trailing_separator_and_symlinks, default_db_keeps_index_named_by_old_hash, vault_key_normalizes_spelling_of_one_path}` |
| L2 | `shell::powershell` nhân đôi cả `'` và U+2018..U+201B trong nháy đơn; `posix` không đổi. | `shell::tests::powershell_doubles_curly_single_quotes` |

## Kiểm

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`: sạch.
- `cargo test --workspace`: qua hết (cả khi đặt `PKM_KG_SOLO`, gồm `answer_key` 70/70 và 11/11).
- Bộ so khớp kiểu CI trên bản chép `kg-solo` mới: `Tổng: 64 mục, 0 lệch chưa có tên, 0 mục chưa có.`

## File đã đổi

`crates/pkm-core/Cargo.toml` (feature `hooks`), `crates/pkm-core/src/query.rs`, `crates/pkm-core/src/query/tests.rs`, `crates/pkm/src/install.rs`, `crates/pkm/src/install/{config,desktop,shell,tests}.rs`, `crates/pkm/src/doctor/tests.rs` (một dòng: `Fake` của doctor đổi tên method bị chặn `desktop_running` thành `desktop_process`, vì method trait đổi tên), `spec/PORTING.md`, `docs/DECISIONS.md`.

## Lưu ý

- Tách `--force` đảo lựa chọn trước đó ("giữ bề mặt lệnh của PRD, dùng lại `--force` thay vì thêm cờ", `docs/DECISIONS.md`). Làm theo yêu cầu của việc này; đã ghi mục mới trong `docs/DECISIONS.md`. Nếu user chưa duyệt việc thêm cờ, cần hỏi lại.
- `crates/pkm-core/Cargo.toml` và `crates/pkm/src/doctor/tests.rs` nằm ngoài danh sách file được giao nhưng cần cho H2 và M2; không chạm vùng của agent đang sửa `mcp`/`cli`.
- Đường dẫn winget của Claude Code: winget id `Anthropic.ClaudeCode` và lối tắt `WinGet\Links\claude.exe` đã có nguồn (issue anthropics/claude-code#31980); thư mục `WinGet\Packages\Anthropic.ClaudeCode_*` suy từ cách winget đặt gói portable, chưa kiểm trên máy Windows thật.
