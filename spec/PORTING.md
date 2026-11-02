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

### `yaml-python-dung-build`: YAML làm bản Python dừng cả lần build

- **Python** (`5a3eb2f`, `scripts/kg/vault.py`, `parse`): chỉ bắt `yaml.YAMLError` và `ValueError`. Một số frontmatter làm PyYAML hoặc `vault.flat` ném ngoại lệ khác, không ai bắt, nên `build` dừng ở note đó và không có index nào:
  - `!!bool` với chữ lạ (`!!bool maybe`): `KeyError`;
  - `!!timestamp` không đúng dạng (`!!timestamp soon`): `AttributeError`;
  - `!!int` hoặc `!!float` rỗng, `!!int` chỉ có dấu (`!!int ""`, `!!int "-"`): `IndexError`;
  - list chứa chính nó qua alias (`a: &r [1, *r]`), hoặc list lồng từ 997 cấp (đo khi gọi `vault.parse` thẳng; qua `index.build` thì sớm hơn vài cấp): `RecursionError` trong `flat`;
  - số nguyên dựng từ cơ số 2, 8, 16 dài quá 4300 chữ số thập phân: `ValueError` khi `str()` (ngoài khối `try`).
- **Rust** (`pkm_core::vault::parse`, lớp `vault::yaml`): không dừng; ghi `nodes.yaml_error` = `YAML lỗi: <tên kiểu ngoại lệ>` (ví dụ `YAML lỗi: KeyError`) cho riêng note, frontmatter coi như rỗng (không `props`, không link frontmatter), thân note vẫn đọc như mọi lỗi YAML khác. Mốc list lồng của Rust cố định ở 997 cấp (`FLAT_DEPTH` trong `vault.rs`); dict lồng thì đọc và in như Python ở mọi độ sâu (`repr` viết dạng vòng lặp), Python chỉ hỏng khi hết ngăn xếp C (khoảng 100.000 cấp).
- **Lý do:** user chọn (docs/DECISIONS.md, "Bốn dạng YAML làm Python crash: Rust ghi lỗi cho note"). Một note gõ sai không được làm hỏng index của cả vault; validator báo lỗi R0 như mọi lỗi YAML.
- **Ngoại lệ trong bộ so khớp:** `yaml-python-dung-build` (tầng `index`, bảng `nodes`, cột `yaml_error`). Trên vault có các dạng này bản Python không dựng được dump, nên ngoại lệ chủ yếu để ghi nhận; kg-solo không có dạng nào. Ca biên nằm trong `crates/pkm-core/tests/yaml-cases/` (`crash-*.md`).
- **Issue ở repo đặc tả:** chưa mở (user chưa cho phép).

### `yaml-ngan-sach-note`: alias lồng nhau làm nổ bộ nhớ

- **Python** (`5a3eb2f`, `scripts/kg/vault.py`, `flat` và `str()`): alias lồng nhau (`a1: &a1 [*a0, *a0, ...]`, `a2: &a2 [*a1, ...]`) dùng chung object khi đọc, nhưng `flat` và `str()` mở rộng ra hết. Vài trăm byte YAML thành hàng chục triệu `props` (7 tầng × 10: 11 triệu dòng, khoảng 900 MB) hoặc chuỗi `repr` hàng GB; thêm một tầng là hết bộ nhớ và cả lần index chết.
- **Rust** (`pkm_core::vault::parse`): mỗi note có ngân sách `NOTE_PROPS_LIMIT` = 1.000.000 giá trị `props` và `NOTE_BYTES_LIMIT` = 64 MiB cho tổng khoá, giá trị và chuỗi `str()`/`repr()`; `Val::py_str`/`py_repr` công khai cũng dừng ở 64 MiB (`yaml::REPR_LIMIT`). Quá ngân sách: `nodes.yaml_error` = `YAML lỗi: MemoryError`, frontmatter coi như rỗng, thân note vẫn đọc.
- **Lý do:** lead quyết định (review 2026-11-02): một note độc trong vault đồng bộ không được làm chết cả lần index. Note bình thường cách ngân sách hàng nghìn lần.
- **Ngoại lệ trong bộ so khớp:** `yaml-ngan-sach-note` (tầng `index`, bảng `nodes`, cột `yaml_error`). Các dòng `props`, `raw_links` của chính note đó cũng khác (Rust không có); chỉ gặp với dữ liệu cố tình hoặc hỏng, kg-solo không có.
- **Issue ở repo đặc tả:** chưa mở (user chưa cho phép).

### `list_notes` bỏ qua file không đọc được

- **Python** (`5a3eb2f`, `scripts/kg/vault.py`, `list_notes`): `os.scandir`/`e.stat()` lỗi (symlink `.md` gãy, file bị xoá giữa lúc liệt kê khi Obsidian hay dịch vụ đồng bộ đang ghi, thư mục con không có quyền) thì ngoại lệ lan ra và cả lần build dừng.
- **Rust** (`pkm_core::vault::list_notes`, `list_notes_reporting`): bỏ qua đúng file hay thư mục con đó và ghi vào danh sách `Unreadable` (đường dẫn, lỗi) mà `list_notes_reporting` trả kèm; chỉ lỗi đọc chính thư mục vault mới là `Err`.
- **Lý do:** lead quyết định (review 2026-11-02): đua với đồng bộ là chuyện thường, một file không được làm hỏng cả index.
- **Ngoại lệ trong bộ so khớp:** không cần; khi gặp trường hợp này bản Python không dựng được dump để so.
- **Issue ở repo đặc tả:** chưa mở (user chưa cho phép).

### Không chuyển `list_windows_drive` (WSL)

`vault.list_notes` của Python liệt kê vault trên ổ Windows nhìn từ WSL bằng PowerShell (`platform_paths.list_windows_drive`). Rust chạy native trên Windows nên không cần; WSL nằm ngoài phạm vi (PRD mục 2.2, bảng test `platform`: các dòng WSL "không áp dụng"). Kết quả liệt kê giống nhánh `os.scandir`, trừ việc nhánh PowerShell bỏ cả file bắt đầu bằng `.` còn nhánh `scandir` chỉ bỏ thư mục chấm; Rust theo nhánh `scandir`.

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
| 17 | `ListNotes.test_same_set_as_walk` | `vault::tests::list_notes_same_set_as_walk` | đã chuyển | Test của `kg.vault.list_notes`, chuyển cùng module `vault` |
| 18 | `WindowsText.test_block_id_in_crlf_note` | `vault::tests::block_id_in_crlf_note` | đã chuyển | Test regex `BLOCK` của `kg.vault` trên note CRLF, chuyển cùng module `vault` |
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

## Test `vault` và `schema`

Nguồn ở sha ghim: `scripts/kg/tests/test_kg.py` không có test riêng cho `vault.py` hay `schema.py` (15 test của nó kiểm qua `build`, validator, truy vấn và tool, nên chuyển cùng các module đó); hai test của `vault` nằm trong `scripts/tests/test_platform_paths.py` (dòng 17, 18 ở bảng trên). Hành vi còn lại so trực tiếp với Python: bộ so khớp (bảng `nodes`, `props`, `raw_links`, `links`, `blocks`, `names`, `schema_*` trên kg-solo) và ca biên YAML có kết quả PyYAML ghi sẵn. Chạy: `cargo test -p pkm-core`.

| Nguồn Python | Test Rust | Trạng thái | Ghi chú |
|---|---|---|---|
| `ListNotes.test_same_set_as_walk` | `vault::tests::list_notes_same_set_as_walk` | đã chuyển | Thêm `list_notes_applies_the_skip_rule`, `list_notes_skips_symlinked_folders` (Unix) |
| `WindowsText.test_block_id_in_crlf_note` | `vault::tests::block_id_in_crlf_note` | đã chuyển | Thêm `crlf_note_keeps_frontmatter_and_body`, `blocks_are_sorted_unique_and_need_line_end` |
| 58 ca biên của bài thử S1 (`probes/s1-yaml/cases/`, kết quả `CSafeLoader`) | `tests/yaml_cases.rs::every_edge_case_matches_pyyaml` | đã chuyển | Cùng `props` và câu lỗi YAML; bốn ca `crash-*` thành lỗi YAML của note (lệch có chủ ý `yaml-python-dung-build`) |
| `vault.parse`, `vault.Resolver`, `vault.fold` (hành vi) | `vault::tests::*`, `vault::resolve::tests::*`, `vault::pystr::tests::*` | đã chuyển | Giá trị mong đợi đã đối chiếu với `vault.py` ở sha ghim; `nesting_boundaries_match_python` giữ mốc list 996/997 và dict 999/1000/1001/3000 đo bằng Python 3.14 |
| PyYAML `SafeConstructor` (hành vi) | `vault::yaml::tests::*`, `vault::yaml::scalar::tests::*`, `vault::yaml::repr::tests::*` | đã chuyển | Khoá dict so như Python, alias vòng in `{...}`, `<<`, `=`, câu lỗi `int()`, `float()`, `binascii`, `datetime` của Python 3.14 |
| `schema.resolve`, `schema.load`, `schema.digest` (hành vi) | `schema::tests::*` | đã chuyển | Schema nhúng có digest `bbaa9f0197ef` như golden; schema tự viết đối chiếu với `schema.py` (gói, `extends` vòng, `widen`, `required_if`, `enum_of_edges`) |

## Bộ cài `pkm mcp install` và `pkm doctor`

Thay `scripts/kg/desktop_config.py` (`5a3eb2f`). Mã: `crates/pkm/src/install.rs` (và `install/`), `crates/pkm/src/doctor.rs`. Bản Python không có test cho file này; test Rust nằm trong hai module (`cargo test -p pkm`). Đây là công cụ cài đặt, không thuộc tầng nào của bộ so khớp, nên các chỗ khác dưới đây không cần ngoại lệ có tên.

Giống Python: tìm cấu hình ở `%APPDATA%\Claude` và bản Microsoft Store (`%LOCALAPPDATA%\Packages\Claude_*\LocalCache\Roaming\Claude`), thấy nhiều file thì bắt chọn `--config`; chỉ thay mục `kg`, giữ mọi khoá và mục khác theo thứ tự; `mcpServers` không phải object thì thay bằng object mới; thụt lề 2, không thoát ký tự ngoài ASCII, giữ CRLF, bỏ BOM; sao lưu `<file>.bak-<thời điểm>` trước khi ghi; từ chối ghi khi Claude Desktop đang chạy trừ khi `--force`; dựng index trước, lỗi thì không đổi cấu hình; `--check` bắt tay `initialize` rồi `tools/list` với `protocolVersion` `2025-06-18`.

Khác Python (cố ý, vì binary thay cho bản chép mã Python):
- `command` là chính binary `pkm`, `args` là `VAULT mcp --db DB <cờ đọc> [--about TEXT]`; không chép file mã nào, không cần Python hay `wsl.exe`. Python ghim sẵn vault bản B và cờ đọc `e11`; Rust bắt buộc truyền `VAULT` và cờ đọc.
- Index mặc định ở `<thư mục pkm>/kg/<tên vault>-<8 hex sha256 đường dẫn vault>.db` (Python: `workshop-b.db` cố định).
- Có macOS (`~/Library/Application Support/Claude`) và Linux (`$XDG_CONFIG_HOME/Claude`, chỉ để test và cho bản cộng đồng).
- Chưa có file cấu hình nào nhưng đúng một thư mục Claude có sẵn thì tạo file mới (Python dừng với "found 0 config files").
- Ghi nguyên tử (file tạm cùng thư mục rồi đổi tên); thời điểm của bản sao lưu là UTC có hậu tố `Z`, thêm `-2`, `-3`… nếu trùng (Python dùng giờ máy và ghi đè trong file cũ).
- Mục `kg` không đổi thì không ghi lại; `--remove` khi không có `kg` thì không ghi (Python luôn ghi và sao lưu).
- `--dry-run` in kế hoạch (cấu hình, gói OHS, index) và phần khác của mục `kg`; dùng được với `--remove`.
- Bắt tay: chờ trả lời `initialize` rồi mới gửi `notifications/initialized` và `tools/list` (Python gửi cả ba một lúc), có hạn giờ (`--timeout`, mặc định 120 giây), bỏ qua dòng stdout không phải JSON.
- Kiểm Claude Desktop đang chạy: Windows liệt kê đường dẫn mọi `claude.exe` qua PowerShell và bỏ qua Claude Code (`~\.local\bin`, npm), không được thì lùi về `tasklist` như Python; macOS `pgrep -x Claude`; Linux `pgrep -x claude-desktop`. Không hỏi được thì từ chối và gợi ý `--force`.
- Cài gói OHS dựng sẵn vào `<thư mục pkm>/ohs` (Python trỏ tới `cli.js` của OHS cài sẵn qua `--ohs-cli` và `node` trên PATH).
