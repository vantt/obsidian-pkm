# Nhật ký chuyển từ bản Python

Ghi mỗi commit của repo đặc tả ([`vantt/obsidian-pkm-automation`](https://github.com/vantt/obsidian-pkm-automation)) chạm đường dẫn theo dõi (PRD mục 5.1) và đã được xử lý ở repo này. Cách chuyển một commit và khi nào nâng ghim: PRD mục 5.4 và 5.5. Sha đang ghim: `spec/spec.lock`.

Mốc ban đầu: bản Rust chuyển từ đặc tả ở `5a3eb2f7990cd093b2ee3a1ccd5b46bd46e3ca4e` (2026-11-02). Bảng chỉ ghi các commit **sau** mốc đó.

Trạng thái là một trong:
- **đã chuyển:** Rust làm giống, bộ so khớp ở sha đó sạch.
- **không áp dụng:** commit chỉ đổi phần WSL, phần Python thuần, hoặc phần ngoài phạm vi (ghi lý do ở cột Ghi chú).
- **lệch có chủ ý:** Rust cố ý làm khác; ghi chi tiết ở mục "Lệch có chủ ý".

| Sha Python | Ngày | Tiêu đề | Commit Rust | Trạng thái | Ghi chú |
|---|---|---|---|---|---|

## Lệch có chủ ý

Chỗ bản Python sai rõ ràng và Rust không làm theo (PRD mục 4). Mỗi mục ghi: sha Python và vị trí (file, hàm), hành vi Python, hành vi Rust, lý do, tên ngoại lệ trong bộ so khớp, issue ở repo đặc tả (nếu user cho mở).

### Thư mục cache có thêm `pkm` trên Linux và macOS

- **Python** (`5a3eb2f`, `scripts/platform_paths.py`, `cache_dir`): ngoài Windows trả `$XDG_CACHE_HOME/NAME` hoặc `~/.cache/NAME`, không có thư mục `pkm`, cả trên macOS. Windows: `%LOCALAPPDATA%\pkm\NAME`.
- **Rust** (`pkm_core::platform::cache_dir`): `%LOCALAPPDATA%\pkm\NAME` (Windows), `~/Library/Application Support/pkm/NAME` (macOS, không đọc `XDG_CACHE_HOME`), `$XDG_CACHE_HOME/pkm/NAME` hoặc `~/.cache/pkm/NAME` (Linux và Unix khác).
- **Lý do:** PRD mục 7 (dòng `platform`) chốt ba thư mục này. Bản Python không đồng nhất giữa các nền và `~/.cache/kg` dễ trùng thư mục của chương trình khác.
- **Ngoại lệ trong bộ so khớp:** không cần; đường dẫn cache không thuộc tầng nào của bộ so khớp (PRD mục 6).
- **Issue ở repo đặc tả:** đề xuất (thêm `pkm` vào `cache_dir` ngoài Windows), chưa mở.

### `move` với `tries=0`

- **Python** (`5a3eb2f`, `scripts/platform_paths.py`, `move`): `tries=0` thì vòng lặp không chạy, file không được đổi tên và không có lỗi nào.
- **Rust** (`pkm_core::platform::move_file_with`): `tries: 0` được tính như 1, vẫn đổi tên một lần.
- **Lý do:** im lặng bỏ qua việc đổi tên là lỗi rõ ràng; không ai gọi Python với `tries=0`, nên không đổi kết quả nào đang có.
- **Ngoại lệ trong bộ so khớp:** không cần (không thuộc tầng nào).
- **Issue ở repo đặc tả:** đề xuất, chưa mở.

## Test `platform` chuyển từ Python

Nguồn: `scripts/tests/test_platform_paths.py` ở sha ghim, 19 test. Test Rust nằm trong `crates/pkm-core/src/platform.rs` (`platform::tests::…`), chạy bằng `cargo test -p pkm-core platform` trên cả ba nền. Test có `#[cfg(...)]` chỉ chạy trên nền tương ứng (Python giả nền bằng mock; Rust kiểm nền thật trong CI).

| # | Test Python | Test Rust | Trạng thái | Ghi chú |
|---|---|---|---|---|
| 1 | `Paths.test_local_path_wsl` | — | không áp dụng | Dịch `C:\…` thành `/mnt/c/…` chỉ cho WSL (PRD mục 2.2) |
| 2 | `Paths.test_local_path_linux_keeps_drive_paths` | — | không áp dụng | `local_path` chỉ có để dịch `/mnt/X`; binary native mở đường dẫn như được đưa, không có hàm tương ứng |
| 3 | `Paths.test_local_path_windows` | — | không áp dụng | Dịch đường dẫn kiểu WSL `/mnt/c/…` sang `C:\…` (PRD mục 2.2) |
| 4 | `Paths.test_windows_drive_and_host_path` | — | không áp dụng | Ổ Windows nhìn từ WSL |
| 5 | `Paths.test_cache_dir` | `cache_dir_windows_uses_localappdata` (Windows), `cache_dir_macos_uses_application_support` (macOS), `cache_dir_linux_uses_xdg_cache_home` (Linux); thêm `cache_dir_without_home_is_an_error`, `cache_dir_of_this_user_ends_in_pkm_name` | lệch có chủ ý | Thêm thư mục `pkm` ngoài Windows, macOS dùng `Application Support` (mục "Lệch có chủ ý") |
| 6 | `Node.test_native` | — | không áp dụng | `node_command` thay bằng module `ohs`: chạy `node` đi kèm gói OHS (PRD mục 9), không chọn `node` theo vault |
| 7 | `Node.test_wsl_drive_through_cmd` | — | không áp dụng | Chạy node của Windows qua `cmd.exe` từ WSL |
| 8 | `Node.test_wsl_drive_node_exe` | — | không áp dụng | `node.exe` từ WSL |
| 9 | `Node.test_list_windows_drive_only_for_drive` | — | không áp dụng | Liệt kê ổ Windows bằng PowerShell từ WSL (PRD mục 2.2) |
| 10 | `OpenUri.test_wsl_escapes_ampersand` | — | không áp dụng | `cmd.exe start` từ WSL |
| 11 | `OpenUri.test_linux_xdg_open` | — | không áp dụng | `open_uri` chỉ `scripts/e11/material.py` dùng (việc E11, ngoài phạm vi theo D9); lõi chỉ trả link `obsidian://`, không mở |
| 12 | `Move.test_never_overwrites` | `move_never_overwrites` | đã chuyển | Lỗi `io::ErrorKind::AlreadyExists` thay cho `FileExistsError`, thông điệp giống Python |
| 13 | `Move.test_name_differing_only_in_case_or_unicode_form_is_taken` | `move_refuses_name_differing_only_in_case_or_unicode_form`; thêm `name_taken_finds_case_and_form_variants_but_not_itself`, `name_key_folds_case_and_unicode_form` | đã chuyển | NFC rồi casefold (crate `caseless`) như Python |
| 14 | `Move.test_case_only_rename_of_itself` | `move_allows_case_only_rename_of_itself` | đã chuyển | |
| 15 | `Move.test_retries_while_file_is_held` | `move_retries_while_file_is_held` | đã chuyển | Hàm đổi tên được truyền vào thay cho mock `os.rename` |
| 16 | `Move.test_gives_up_after_tries` | `move_gives_up_after_tries`; thêm `move_does_not_retry_other_errors`, `sharing_violation_counts_as_held` (Windows) | đã chuyển | |
| 17 | `ListNotes.test_same_set_as_walk` | — | chuyển cùng module `vault` | Test của `kg.vault.list_notes`, không phải của `platform_paths` |
| 18 | `WindowsText.test_block_id_in_crlf_note` | — | chuyển cùng module `vault` | Test regex `BLOCK` của `kg.vault` trên note CRLF |
| 19 | `WindowsText.test_utf8_stdio_on_an_ansi_pipe` | — | không áp dụng | `utf8_stdio` là phần Python thuần (PRD mục 5.4): Rust luôn ghi UTF-8, không phụ thuộc code page |

Test Rust thêm, không có bản Python: `move_with_zero_tries_still_tries_once` (lệch có chủ ý ở trên), `move_renames_into_vietnamese_name`, `name_taken_in_missing_folder_is_none`, `name_taken_resolves_dot_segments_of_src`, `verbatim_prefixes_drive_and_unc_paths` (Windows: tiền tố `\\?\` cho đường dẫn dài).

Trên Windows, Rust đổi tên bằng `MoveFileExW` không cờ (như `os.rename` của Python), vì `std::fs::rename` của Rust đè đích trên Windows.

## Module `ohs`

Bản Python không có module tương ứng: `scripts/kg/related.py` (`ohs_seeds`) chạy `node cli.js --db VAULT/.obsidian-hybrid-search.db search --json` mỗi lần một tiến trình. Rust thay bằng `pkm_core::ohs` theo PRD mục 9 và D6, D7: một tiến trình `serve --stdio` sống lâu, request có `id` đồng thời, khởi động lại có giới hạn rồi `broken`, trạng thái và lỗi `ohs_not_ready`. `OhsConfig::db` mặc định `None`: OHS tự chọn DB như MCP `ohs` của người dùng, nên hai bên dùng chung một index như bản Python. Thư mục gói mặc định là `<thư mục pkm>/ohs` (`platform::pkm_dir`); bộ cài đặt gói vào đây. Test: `crates/pkm-core/tests/ohs_fake.rs` (server giả, chạy trong CI) và `tests/ohs_real.rs` (`#[ignore]`, cần gói thật).

### Không lùi về đọc thẳng DB của OHS

- **Python** (`5a3eb2f`, `scripts/kg/related.py`, `ohs_seeds`): CLI lỗi hoặc không có thì mở chỉ đọc `.obsidian-hybrid-search.db` của OHS và tìm từ khoá BM25 trong bảng FTS5; nguồn hạt giống ghi `ohs-fts`.
- **Rust:** không có đường lùi. OHS chưa sẵn sàng hoặc không chạy được thì tool trả `ohs_not_ready` với `state` và `action`.
- **Lý do:** D7 (không có đường lùi cho ra kết quả kém) và PRD mục 9 (không bao giờ đọc thẳng DB của OHS; lược đồ DB là chi tiết nội bộ của OHS).
- **Ngoại lệ trong bộ so khớp:** `related` chỉ có câu hỏi so trên máy có gói OHS (PRD mục 6); trường hợp không có OHS không so với nhánh `ohs-fts` của Python. Phần `related` ghi tên ngoại lệ khi nối bộ so khớp.
- **Issue ở repo đặc tả:** không cần; bản Python giữ đường lùi vì chạy được không cần Node.

### Lỗi `ohs_not_ready`: `progress` và `eta_s` có thể là `null`

Lỗi luôn có đủ năm khoá `error`, `state`, `progress`, `eta_s`, `action` theo thứ tự đó (PRD mục 8). Ví dụ ở PRD có số cho `progress` và `eta_s`, nhưng hai khoá này chỉ có số khi `state` là `indexing`. Ở trạng thái khác, cả hai là `null`. Ngay trong `indexing`, `eta_s` vẫn là `null` cho tới khi đo được tốc độ index, vì OHS không báo `eta_s`. Người gọi (agent, plugin) phải xử lý được giá trị `null`. Khi thêm `docs/MCP-CHANGELOG.md` thì ghi điều này vào đó.
