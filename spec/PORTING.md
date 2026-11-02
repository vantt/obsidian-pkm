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

### Adapter chỉ nạp theo tên ngắn, không nạp file `.py`

- **Python** (`5a3eb2f`, `scripts/kg/index.py`, `load_adapter`): `--adapter` nhận tên ngắn (`e11`) hoặc đường dẫn một file `.py` bất kỳ có lớp `Adapter` hay hàm `edges(ctx)`.
- **Rust** (`pkm_core::adapter::load`): chỉ nhận tên ngắn của adapter dựng sẵn (`e11`), rỗng và `none`. Chữ khác báo lỗi `không có adapter '<chữ>' (tên ngắn: e11; bản Rust không nạp adapter từ file .py)`.
- **Lý do:** binary không chạy Python (D1: khách không phải cài Python). Adapter mới là một impl của trait `Adapter` trong `pkm-core`.
- **Ngoại lệ trong bộ so khớp:** không cần; bộ so khớp chỉ dùng `e11` hoặc không adapter.
- **Issue ở repo đặc tả:** không cần.

### Sổ của adapter `e11`: dòng có YAML lỗi luôn được bỏ qua

- **Python** (`5a3eb2f`, `scripts/e11/kg_adapter.py`, `_log_keys`): chỉ bắt `yaml.YAMLError`; frontmatter trong sổ làm PyYAML ném `ValueError` (ví dụ ngày không có thật trong `!!timestamp`, số quá 4300 chữ số) thì cả lần build dừng.
- **Rust** (`pkm_core::adapter::e11`): mọi lỗi YAML có câu lỗi (`Fail::Error`, gồm cả các `ValueError` mà `vault.parse` cũng coi là lỗi YAML) làm bỏ dòng sổ đó; các dạng làm Python dừng vì ngoại lệ khác (`Fail::Crash`) vẫn làm build báo lỗi như Python.
- **Lý do:** lớp YAML của `vault` gộp `YAMLError` và `ValueError` thành một loại lỗi (như `vault.parse`). Một dòng sổ hỏng chỉ mất nguồn gốc của một cạnh, không đáng dừng cả index.
- **Ngoại lệ trong bộ so khớp:** không cần; trên các dòng này bản Python không dựng được index để so.
- **Issue ở repo đặc tả:** đề xuất (bắt cả `ValueError` trong `_log_keys`), chưa mở.

### Build hỏng giữa chừng: lần sau vẫn tính lại cạnh

- **Python** (`5a3eb2f`, `scripts/kg/index.py`, `build`): bảng note được commit ở giao dịch đầu, cạnh ở giao dịch sau. Nếu bước tính cạnh lỗi (adapter ném lỗi, schema hỏng, tiến trình bị dừng), lần build tăng dần sau thấy mtime đã khớp và `schema_digest` cũ, nên giữ `links`, `edges` cũ mãi, tới khi chạy `--full`.
- **Rust** (`pkm_core::index::build`): giao dịch ghi bảng note ghi thêm dòng `meta.dirty`; dòng này chỉ mất khi ghi `meta` cuối build. Lần sau còn thấy `dirty` thì tính lại cạnh. Build thành công cho `meta` và mọi bảng giống hệt Python (dòng `dirty` không còn).
- **Lý do:** index lệch vĩnh viễn mà không báo gì là lỗi rõ ràng (review mã ngày 2026-11-02, mục H1).
- **Ngoại lệ trong bộ so khớp:** không cần (chỉ khác khi build hỏng).
- **Issue ở repo đặc tả:** đề xuất, chưa mở.

### Build đồng thời, chế độ WAL, file biến mất

- **Python** (`5a3eb2f`, `scripts/kg/index.py`, `build`): đọc và parse note bên trong giao dịch ghi; hai build cùng lúc trên một DB có thể lỗi `database is locked`; `PRAGMA journal_mode=WAL` chạy mỗi lần build, không đổi được thì build lỗi; file mất giữa lúc liệt kê và lúc đọc làm cả build lỗi.
- **Rust** (`pkm_core::index::build`):
  - Mỗi lúc chỉ một build ghi một DB: khoá file `<db>.lock` cạnh DB (`File::try_lock`, chờ tối đa 5 phút; hệ thống file không có khoá thì build không khoá như Python). MCP tự build lại và CLI chạy cùng lúc thì lần lượt.
  - Đọc và parse note trước khi mở giao dịch; giao dịch ghi là `IMMEDIATE`, chờ khoá SQLite tối đa 30 s.
  - Chỉ chuyển sang WAL khi DB chưa ở WAL; không chuyển được (kết nối khác đang đọc ở chế độ rollback, ổ mạng) thì giữ chế độ cũ và build tiếp.
  - File mất giữa lúc liệt kê và lúc đọc (`NotFound`) coi như note đã xoá: không vào `notes`, `notes_read`; nếu đã có trong index thì tính vào `notes_removed` và bị xoá khỏi index.
- **Lý do:** review mã ngày 2026-11-02 (mục H2, M1, M3): MCP và CLI dùng chung một DB, kể cả trên Windows; Obsidian ghi note bằng cách đổi tên file.
- **Ngoại lệ trong bộ so khớp:** không cần (kết quả của build thành công trên vault không đổi giữa chừng giống hệt Python).
- **Issue ở repo đặc tả:** đề xuất, chưa mở.

### `paths`, `loops`, `all_loops`: trần `max_len` và hạn giờ

- **Python** (`5a3eb2f`, `scripts/kg/query.py`, `paths`, `loops`, `all_loops`): không giới hạn `max_len`, không hạn giờ. Số đường đơn tăng theo hàm mũ với độ dài, nên trên vault liên kết dày một lời gọi `max_len=8..10` chạy mãi và ăn hết bộ nhớ; MCP server một luồng kẹt theo, mọi request sau (kể cả `ping`) không được trả lời.
- **Rust** (`pkm_core::query::Graph`):
  - `max_len` lớn hơn `MAX_LEN_CAP` = 12 thì trả `ValueError` (`max_len N is too large: at most 12 ...`) trước khi chạy. Áp cho cả ba hàm, nên cả tool MCP, CLI và bộ câu đáp án.
  - Mỗi lần liệt kê có hạn giờ, mặc định `WALK_BUDGET` = 10 s; biến `PKM_QUERY_TIMEOUT` (số giây, có thể lẻ; `0` là không giới hạn) hoặc `Graph::set_walk_budget` đổi được. CTE của `paths`, `loops` bị ngắt qua progress handler của SQLite (kiểm mỗi 1000 lệnh máy ảo); DFS của `all_loops` tự kiểm hạn. Quá hạn thì trả `TimeoutError` (`query too expensive: stopped after 10.0 s; lower max_len or narrow rels ...`); kết nối vẫn dùng tiếp được, câu không phải liệt kê không bị hạn.
- **Lý do:** review mã ngày 2026-11-02: một lời gọi tool không được khoá cả server. Mọi lời gọi trong bộ so khớp dùng `max_len` tối đa 4, bộ câu đáp án tối đa 10, mặc định của `paths` là 6, của câu `cycles` là 10; tất cả xong trong vài ms.
- **Ngoại lệ trong bộ so khớp:** không cần (không lời gọi nào chạm trần hay hạn giờ; bộ so khớp kg-solo 0 lệch).
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
- **Bộ so khớp:** `related` chỉ có câu hỏi so trên máy có gói OHS (PRD mục 6); trường hợp không có OHS không so với nhánh `ohs-fts` của Python. Golden kg-solo không có lời gọi `related` chỉ có câu hỏi (cả 15 lời gọi đều có `hits`, trừ một lời gọi rỗng trả lỗi giống Python), nên không cần ngoại lệ có tên.
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
- Index mặc định ở `<thư mục pkm>/kg/<tên vault>-<8 hex sha256 khoá vault>.db` (Python: `workshop-b.db` cố định). Khoá là đường dẫn đã giải symlink, bỏ tiền tố `\\?\`, bỏ dấu phân cách cuối, NFC, gộp hoa thường trên Windows và macOS, để `D:\Vault` và `d:\vault\` dùng chung một index. Index đặt theo cách băm cũ (đường dẫn nguyên văn) mà đã có thì dùng tiếp.
- Có macOS (`~/Library/Application Support/Claude`) và Linux (`$XDG_CONFIG_HOME/Claude`, chỉ để test và cho bản cộng đồng).
- Chưa có file cấu hình nào nhưng đúng một thư mục Claude có sẵn thì tạo file mới (Python dừng với "found 0 config files").
- Ghi nguyên tử (file tạm mới tạo bằng `create_new`, tên `<file>.tmp-<pid>-<16 hex ngẫu nhiên>`, cùng thư mục, rồi đổi tên); cấu hình là symlink thì ghi vào file đích và giữ symlink (symlink gãy thì từ chối); trên Unix file mới giữ quyền của file cũ; thời điểm của bản sao lưu là UTC có hậu tố `Z`, thêm `-2`, `-3`… nếu trùng (Python dùng giờ máy và ghi đè trong file cũ).
- Mục `kg` không đổi thì không ghi lại; `--remove` khi không có `kg` thì không ghi (Python luôn ghi và sao lưu).
- `--dry-run` in kế hoạch (cấu hình, gói OHS, index) và phần khác của mục `kg`; dùng được với `--remove`.
- Bắt tay: chờ trả lời `initialize` rồi mới gửi `notifications/initialized` và `tools/list` (Python gửi cả ba một lúc), có hạn giờ (`--timeout`, mặc định 120 giây), bỏ qua dòng stdout không phải JSON.
- Kiểm Claude Desktop đang chạy: Windows liệt kê đường dẫn mọi `claude.exe` qua PowerShell và bỏ qua Claude Code (`~\.local\bin`, npm, winget `Anthropic.ClaudeCode` và `WinGet\Links`, scoop `apps\claude-code`, `Programs\claude-code`; danh sách `CLAUDE_CODE_DIRS` trong `install/desktop.rs`), không được thì lùi về `tasklist` như Python; macOS `pgrep -x Claude`; Linux `pgrep -x claude-desktop`. Lời từ chối in đường dẫn exe bị coi là Claude Desktop. Không hỏi được thì từ chối và gợi ý `--force`. `--force` chỉ bỏ qua kiểm này.
- Cài gói OHS dựng sẵn vào `<thư mục pkm>/ohs` (Python trỏ tới `cli.js` của OHS cài sẵn qua `--ohs-cli` và `node` trên PATH).
- Binary `pkm` đang chạy không ở chỗ cài ổn định (`%LOCALAPPDATA%\pkm\bin` trên Windows, `~/.local/bin` nơi khác, hằng `STANDARD_BIN_DIR`; hoặc `PKM_INSTALL_DIR`; hoặc thư mục có `.pkm-managed` của bộ cài) thì `mcp install` từ chối trừ khi `--allow-any-location` (cờ riêng, không bỏ qua kiểm Claude Desktop), nói rõ khi file nằm trong `Downloads` hay thư mục tạm, và in lệnh bộ cài cùng lệnh chạy lại từ chỗ chuẩn. `--dry-run` chỉ in cảnh báo.
- So rồi mới ghi: lưu sha256 của bytes cấu hình đã đọc; dựng index xong thì hỏi lại Claude Desktop có chạy không và đọc lại file. File đã đổi thì đặt lại mục `kg` trên bản mới, giữ thay đổi kia và báo cho người dùng.
- Cài với `--no-ohs` thì mục `kg` có `"env": {"PKM_NO_OHS": "1"}`, để `doctor` bỏ qua gói OHS. Lần cài sau không có `--no-ohs` thì bỏ biến này.
- Nguồn gói OHS đổi được mà không cần bản `pkm` mới: biến `PKM_OHS_PACKAGE` (đường dẫn như `--ohs-package`, hoặc URL `https://` của file gói, kèm `<URL>.sha256`) thay cho việc tự tải artifact ghim. `--ohs-package` cũng nhận URL. Nguồn URL tải bằng `curl` chỉ qua https, kiểm với file `.sha256` đi kèm.
- `pkm doctor` (không có bản Python) chỉ đọc: kiểm gói OHS có đủ file và `node --version` (`ohs_node`), chỉ chạy thử OHS khi có `--deep`. OHS là tuỳ chọn ở giai đoạn so khớp, nên mục OHS không ổn chỉ là `warn`, không bao giờ `fail`. Không có OHS thì chỉ `related` khi không có `hits` bị ảnh hưởng.
- `pkm doctor --json` theo envelope chung, khoá ổn định qua các bản: `{"schema": 1, "ok", "data": {"checks": [{"id", "status", "detail", "action", "fix"}]}, "error": null | {"code": "checks_failed", "why", "fix"}, "next": [lệnh]}`. Mã mục: `claude_config`, `kg_entry`, `db`, `ohs_package`, `ohs_node`, `ohs_state`, `long_paths`. `status` thuộc `ok`, `warn`, `fail`, `skip`. `fix` là lệnh không hỏi gì và chạy lại được, dựng từ `args` của mục `kg` (`mcp install` lại, hoặc `VAULT build --db DB <cờ đọc>`), cho PowerShell trên Windows và `sh` nơi khác. `next` là `fix` của các mục `fail`. Mã thoát 0 khi `ok`, 1 khi không.

## Module `index`, `validate`, `adapter::e11`

`pkm_core::index` dựng index SQLite như `scripts/kg/index.py` (DDL chép nguyên, `rusqlite` với SQLite đóng gói sẵn), `pkm_core::validate` giữ nguyên câu SQL của 19 view (so `sqlite_master.sql` với DB của Python: giống từng byte), `pkm_core::adapter` có trait `Adapter` và impl `e11` (`scripts/e11/kg_adapter.py`). Mọi câu đọc lại DB trong lúc build giống Python, nên thứ tự dòng, `edges.id` và rowid của `links` trùng với Python cả khi build tăng dần sau khi sửa vault. Bộ so khớp đọc DB qua `pkm_core::index::dump_tables`.

Đối chiếu với Python ở sha ghim, ngoài bộ so khớp kg-solo (20 bảng và view khớp): so cả DB (`sqlite_master`, mọi bảng theo rowid, `meta`) sau build đủ, build không đổi gì, build tăng dần sau khi sửa (đổi status, xoá, thêm note có alias trùng, đổi tên note), build tăng dần lần hai, build đủ lại, trên kg-solo với ba cách đọc (mặc định; `--no-aliases --edge-keys links --body-edges`; không gói và `--body-edges`), trên vault cài lỗi của `test_kg.py`, và trên vault e11 của `test_kg_adapter.py` với `--adapter e11` (sửa file cặp, sổ có dòng `\r\n`, dòng YAML lỗi và `rules` là chuỗi, đề xuất mới, trang chủ đề mới, thư mục lồng sâu, đổi status đề xuất): giống hết. Riêng `schema_field`, `schema_link` có thứ tự rowid khác vì Python chèn theo thứ tự của `set()` (đổi theo `PYTHONHASHSEED`); nội dung giống.

Giới hạn đã biết (không gặp trên kg-solo, bộ dữ liệu trên hay schema nhúng):

- Build tăng dần nhận ra note đổi chỉ qua (mtime tính bằng ms, cỡ file), như Python; không băm nội dung. Sửa giữ nguyên số byte mà mtime không đổi thì bị bỏ sót: trên FAT, exFAT (USB, thẻ nhớ; mtime theo bước 2 s) khi sửa hai lần trong cùng 2 s, hay khi công cụ đồng bộ (iCloud, Dropbox, Syncthing) ghi lại mtime gốc. Cách chữa: `build --full` (đọc lại mọi note). Đổi gói hoặc cách đọc tự dựng lại hết.
- `previous` (gói, adapter, cách đọc của lần trước) mở DB chỉ đọc; không mở được (thư mục không tạo được file `-shm` của WAL) thì thử mở đọc ghi, vẫn không được thì trả giá trị mặc định. `try_previous` báo lỗi thay vì trả mặc định, cho lệnh nào cần phân biệt.
- Id note luôn ở dạng NFC (`vault::list_notes`); `data.to` của đề xuất `fix-link` cũng được đưa về NFC trước khi so, nên tên file NFD (macOS) vẫn khớp (test `adapter::tests::fix_link_target_in_nfd_matches_nfc_note_id`).

- `regexp` của view R9 (mẫu `pattern` của schema) chạy bằng crate `regex`: `$` và `\Z` đổi theo nghĩa của Python, `[`, `&`, `~` trong lớp ký tự là chữ thường; mẫu dùng nhìn trước, nhìn lùi, tham chiếu ngược làm truy vấn view báo lỗi (Python chạy được). Schema nhúng chỉ có `^.+@schema_v[0-9]+$`.
- `float()` (cột `score` của claim, `data.score` của đề xuất) chỉ nhận chữ số ASCII.
- Adapter `e11`: file JSON có `NaN`, `Infinity` bị coi là hỏng (Python đọc được); khoá không phải chuỗi trong frontmatter của sổ so theo `repr` (Python còn coi `1 == 1.0 == True`).

Nguồn: `scripts/kg/tests/test_kg.py` (15 test, đủ 15 dòng dưới đây) và `scripts/e11/tests/test_kg_adapter.py` (16 test) ở sha ghim. Test nào kiểm qua `Graph`, `Tools`, CLI hay MCP thì chuyển cùng module đó (`query`, `tools`: mục sau); phần đọc cạnh của chúng đã kiểm ở đây bằng SQL. Chạy: `cargo test -p pkm-core` (lọc theo `index::`, `validate::`, `adapter::`, `query::`, `tools::`).

| Test Python | Test Rust | Trạng thái | Ghi chú |
|---|---|---|---|
| `test_kg.Planted.test_every_planted_error_fires` | `validate::tests::every_planted_error_fires` | đã chuyển | Thêm `planted_deviations_equal_python`: mọi chỗ lệch và số đếm đúng như Python trả |
| `test_kg.Planted.test_every_rule_category` | `validate::tests::every_rule_category` | đã chuyển | |
| `test_kg.Planted.test_clean_notes_have_no_deviation` | `validate::tests::clean_notes_have_no_deviation` | đã chuyển | |
| `test_kg.Planted.test_r9_kinds` | `validate::tests::r9_kinds` | đã chuyển | |
| `test_kg.Api.test_clean_vault_has_no_deviation` | `validate::tests::clean_vault_has_no_deviation` | đã chuyển | |
| `test_kg.Api.test_two_layers_default_and_accepted_only` | `query::tests::two_layers_default_and_accepted_only` | đã chuyển | Như Python, qua `Graph.neighbors`; `index::tests::two_layers_and_provenance` kiểm thêm các cột đó bằng SQL |
| `test_kg.Api.test_claim_edge_window_source_and_alias` | `query::tests::claim_edge_window_source_and_alias` | đã chuyển | Như Python, qua `edge_of_claim` và `neighbors(as_of)`; `index::tests::claim_edge_window_source_and_alias` kiểm phần cạnh bằng SQL |
| `test_kg.Api.test_contract_window` | `query::tests::contract_window` | đã chuyển | |
| `test_kg.Api.test_loops_need_start_and_report_polarity` | `query::tests::loops_need_start_and_report_polarity` | đã chuyển | `loops(None, 5)` của Python thành `loops("", 5)`; kiểm thêm `max_len = 0` |
| `test_kg.Api.test_paths_and_provenance` | `query::tests::paths_and_provenance` | đã chuyển | |
| `test_kg.Api.test_find_select` | `query::tests::find_select` | đã chuyển | |
| `test_kg.Api.test_long_result_envelope` | `tools::tests::long_result_envelope` | đã chuyển | Kiểm thêm thứ tự khoá của vỏ kết quả (`count`, `by_type`, `items`, `truncated`, `handle`) |
| `test_kg.Incremental.test_rebuild_after_edit_and_delete` | `index::tests::rebuild_after_edit_and_delete`, `query::tests::rebuild_after_edit_and_delete` | đã chuyển | Phần build (`mode`, `notes_read`, `notes_removed`) ở `index`; status cạnh của claim qua `edge_of_claim` và `loops` rỗng ở `query` |
| `test_kg.Fixture.test_answer_key` | `query::tests::answer_key` | đã chuyển | Cần `PKM_KG_SOLO=<vault kg-solo>` (không có thì bỏ qua, như Python khi thiếu node): 70/70 câu graph, 11/11 câu bảng. CI so từng câu với golden qua tầng `answers` của bộ so khớp |
| `test_kg.Fixture.test_fixture_planted_errors` | `validate::tests::fixture_planted_errors` | đã chuyển | Cần `PKM_KG_SOLO=<vault kg-solo>` (không có thì bỏ qua, như Python khi thiếu node); CI so cả `v3_deviations` qua bộ so khớp |
| `test_kg_adapter.AdapterEdges` (9 test: `folder_topic`, `proposal_waiting_is_proposed`, `rejected_and_accepted`, `applied_proposal_marks_vault_edge`, `who_and_fix_link`, `ohs_pairs`, `provenance_from_logs`, `proposal_body_links_dropped`, `counts`) | `adapter::tests::` cùng tên | đã chuyển | `counts` kiểm thêm `adapter_spec`, `hidden`, `schema_digest` |
| `test_kg_adapter.Incremental.test_digest_and_pair_file` | `adapter::tests::digest_and_pair_file` | đã chuyển | Không ngủ 1,1 s: file cặp mới khác cỡ nên digest đổi |
| `test_kg_adapter.HiddenAndLayers` (4 test: `system_hidden_unless_asked`, `paths_avoid_hubs`, `layers`, `select_skips_system`) | `tools::tests::` cùng tên | đã chuyển | Vault e11 chép từ `adapter::tests` (module đó riêng tư). `system_hidden_unless_asked` lấy đủ kết quả `reach` (`limit` 1000) thay vì 20 mục đầu |
| `test_kg_adapter.HiddenAndLayers.test_related_skips_system` | `tools::tests::related_skips_system` | đã chuyển | Qua điểm vào chung `call` |
| `test_kg_adapter.GraphIndexCli.test_commands` | — | chưa chuyển | `graph_index.py` gộp vào CLI `pkm` (PRD mục 2.1): chuyển cùng CLI |
| `test_kg_adapter.OverMcp.test_proposed_edges_and_restart` | — | chưa chuyển | Chuyển cùng MCP |

Test Rust thêm, không có bản Python: `index::tests::{noop_build_keeps_every_table, meta_like_python, counts_escape_non_ascii_status_like_json_dumps, changing_packs_or_options_rebuilds_everything, body_edges_and_edge_keys_links, options_pick_and_previous, ctx_add_and_drop_deduplicate_by_key, regexp_follows_python_re, missing_vault_is_an_error, empty_vault_builds_an_empty_index, failed_build_is_rederived_next_time, vanished_file_counts_as_removed, concurrent_builds_take_turns, wal_switch_failure_is_not_fatal, try_previous_reports_unreadable_db}`, `index::py::tests::*` (`float()`, `json.dumps`), `validate::tests::{rule_filter_and_limit, report_like_the_validate_command, views_are_recreated_and_sql_kept}`, `adapter::tests::{similar_file_replaces_both_pair_files, broken_private_data_is_an_error, load_and_system_pages, fix_link_target_in_nfd_matches_nfc_note_id}`.

Test Rust thêm, không có bản Python: `index::tests::{noop_build_keeps_every_table, meta_like_python, counts_escape_non_ascii_status_like_json_dumps, changing_packs_or_options_rebuilds_everything, body_edges_and_edge_keys_links, options_pick_and_previous, ctx_add_and_drop_deduplicate_by_key, regexp_follows_python_re, missing_vault_is_an_error, empty_vault_builds_an_empty_index}`, `index::py::tests::*` (`float()`, `json.dumps`), `validate::tests::{rule_filter_and_limit, report_like_the_validate_command, views_are_recreated_and_sql_kept}`, `adapter::tests::{similar_file_replaces_both_pair_files, broken_private_data_is_an_error, load_and_system_pages}`.

## Module `query`, `tools`

`pkm_core::query::Graph` là `scripts/kg/query.py`: câu SQL (recursive CTE của `reach`, `paths`, `loops`, điều kiện lọc `_w`, `_hid`, `_parts`, thứ tự `ORDER BY`) chép nguyên, phần Rust chỉ ghép câu, đọc dòng và gộp (`_collapse`, Tarjan của `all_loops`). `pkm_core::tools::Tools` là `scripts/kg/tools.py` (vỏ kết quả dài, handle `sha1` của `json.dumps(..., sort_keys=True)`, bộ nhớ 64 kết quả cho `more`); `pkm_core::tools::call` là `Server.call` của `scripts/kg/mcp_server.py` (note id chuẩn hoá NFC, `did_you_mean`, `url` `obsidian://open`, `body`, các lỗi `{"error": ...}`), điểm vào chung cho MCP, CLI và bộ so khớp; `Err` là chỗ Python ném ngoại lệ, mang tên lớp ngoại lệ Python (`KeyError`, `ValueError`, `TypeError`, ...). `pkm_core::tools::defs` là `tool_defs` (mô tả, schema tham số của `tools/list`): so với Python theo từng ký tự cả thứ tự khoá, với `hidden` đúng và sai. Kết quả dạng `dict` giữ thứ tự khoá của Python (`serde_json` có `preserve_order`). `pkm_core::query::answers` là `scripts/kg/tests/fixture_check.py` (`run`, `norm`, `check`). `related`: xem mục Module `related`.

Đối chiếu với Python ở sha ghim: bộ so khớp kg-solo (81 câu đáp án, 712 lời gọi tool trừ `related`, gồm 63 lời gọi `more`) khớp hết; thêm cùng bộ câu tự sinh của `dump-python.py` trên kg-solo đọc như bản B (`--no-aliases --edge-keys links --body-edges`), trên vault cài lỗi của `test_kg.py`, trên vault e11 với `--adapter e11` (trang hệ thống, `system`, lớp `proposed`, `rejected`), và 181 lời gọi lệch chuẩn trên kg-solo (tham số rỗng, sai kiểu, số âm, `limit` là `true`, `rels` là chuỗi, `max_depth` là số thực, `max_len` là chuỗi, note id dạng NFD hay là số, handle hết hạn, là số, là danh sách, `offset` âm hay vượt cỡ, tool lạ, thiếu tham số bắt buộc): giống hết, kể cả tên ngoại lệ.

Giới hạn đã biết (không gặp trong bộ so khớp; MCP client gửi đúng kiểu theo schema):

- Tham số sai kiểu mà Python vẫn chạy tiếp với giá trị lạ thì Rust trả `TypeError`: `as_of` không phải chuỗi (Python đưa số vào SQL, hoặc ném `ProgrammingError` với danh sách); phần tử không phải chuỗi trong `rels`, `statuses`; `type`, `as_of` của `valid` là số; `where` của `select` không phải object (Python nhận danh sách cặp); `limit`, `k`, `offset`, `body_chars` là số thực (Python chạy được khi kết quả không bị cắt); `max_depth`, `max_len` của `paths` là chuỗi (Python so số với chuỗi trong SQL: `reach` chạy không dừng).
- `int()` của `max_len` (`loops`) chỉ nhận chữ số ASCII; số thực ngoài khoảng `i64` bị chặn.
- `meta.hidden` không phải danh sách JSON thì coi như không có trang hệ thống (Python lặp trên chuỗi hay khoá); index luôn ghi danh sách.
- `by_type`: số bằng nhau thì Python xếp theo thứ tự băm (đổi theo lần chạy), Rust theo tên type.

| `fixture_check.py` | Rust | Ghi chú |
|---|---|---|
| `GRAPH_OPS`, `TABLE_OPS` | `query::answers::{GRAPH_OPS, TABLE_OPS}` | |
| `norm` | `query::answers::norm` | So danh sách theo `json.dumps(sort_keys=True, ensure_ascii=False)` của từng mục |
| `run` (9 phép: `neighbors`, `reach`, `paths`, `cycles`, `chain`, `provenance`, `find`, `select`, `valid`) | `query::answers::run` | Phép lạ: `KeyError`. Bộ so khớp chạy từng câu qua hàm này (tầng `answers`) |
| `check` (70 câu graph, 11 câu bảng) | `query::answers::check` | `query::tests::answer_key` |

Test Rust thêm, không có bản Python: `query::tests::{reach_depth_and_direction, note_card_counts_edges_by_relation_and_layer, all_loops_start_at_smallest_id, answers_like_fixture_check, max_len_above_cap_is_refused_before_running, walks_over_budget_stop_with_clear_error_and_graph_stays_usable, walk_budget_reads_seconds_and_zero_disables, full_rebuild_keeps_db_file_for_open_readers}`, `query::py::tests::*` (`json.dumps`, `str`, `repr`, `int()`, cắt danh sách, `bool`), `tools::tests::{handle_like_python, short_results_have_no_handle_and_cache_is_bounded, call_reports_unknown_notes_with_suggestions, default_vault_name_like_abspath_basename, call_note_has_link_and_body, call_errors_like_server_call, call_runs_each_tool_with_mcp_arguments, definitions_match_the_golden_tools_list, definitions_add_system_only_when_vault_has_system_pages}`.


## MCP server (`pkm VAULT mcp`) và CLI (`pkm VAULT <lệnh>`)

Thay `scripts/kg/mcp_server.py`, `scripts/kg/cli.py`, `scripts/kg/__main__.py` (`5a3eb2f`). Mã: `crates/pkm/src/mcp.rs` (và `mcp/`), `crates/pkm/src/cli.rs`. Bộ so khớp chạy chính binary `pkm` cho tầng MCP và CLI (`parity/src/rust_side/binary.rs`); trên kg-solo `initialize`, `tools/list`, `instructions` và cả 21 lệnh CLI của golden (`build` ba kiểu, `stats`, `validate` ba kiểu, 13 lệnh hỏi graph, `loops` thiếu `--max-len`) giống golden từng byte. Bản tin `tools/list` của `pkm` và của MCP server Python trên cùng vault giống nhau từng byte, kể cả thứ tự khoá. 15 lời gọi `tools/call` (mọi tool trừ `related`, cả lỗi) cũng giống từng byte, trừ số `id` của cạnh: bản Python chạy trên máy đánh số cạnh theo thứ tự duyệt thư mục, còn bản Rust đánh số như golden.

Tầng tool đi qua chỗ nối `crates/pkm/src/mcp/tool_layer.rs`, nối vào `pkm_core::tools`. `tools/list` là `tools::definitions(hidden)`. `tools/call` gọi `tools::call` (như `Server.call`; ngoại lệ thành `error: <repr>`). Lệnh CLI hỏi graph gọi các phương thức của `tools::Tools`, như `Tools.<method>` mà `cli.py` gọi: không chuẩn hoá NFC, không `did_you_mean`, không `url`, không `body`. Riêng `valid` của CLI đi qua `tools::call`, để khi vắng `--as-of` kết quả rỗng như `None` của Python. Vỏ vẫn chạy khi không có tầng tool (test của vỏ dùng tầng tool giả): `tools/list` rỗng, `tools/call` trả `index unavailable: …`, lệnh CLI hỏi graph thoát mã 3. `related` đi qua điểm vào chung; khi lõi chưa có tool này thì MCP trả `error: NotImplementedError(...)`.

Giống Python: tên server `kg` `0.1.0`, phiên bản giao thức trả lại đúng bản client xin nếu thuộc `2025-06-18`, `2025-03-26`, `2024-11-05` (không thì `2025-06-18`); mỗi bản tin in như `json.dumps(..., ensure_ascii=False)` (dấu `, ` và `: `), dòng hỏng `-32700` in như `json.dumps` mặc định; batch, notification, phương thức lạ (`method not found: None` khi thiếu `method`), ba phương thức danh sách rỗng; `instructions` ghép đủ tám khối, thêm khối trang hệ thống, đoạn adapter, khối link `obsidian://` (`--obsidian-vault ""` để tắt); dựng index ở luồng nền, `initialize` và `tools/list` chờ tối đa 20 giây; refresh trước mỗi `tools/call` theo `--refresh-every` (mặc định 5, 30 khi vault ở `/mnt/<ổ>` dưới WSL; `-1` chỉ dựng lúc khởi động), chỉ mở lại graph khi build `full` hoặc có note đọc lại, xoá; build lỗi thì dùng index cũ; kết quả tool thu gọn (`compact`), `isError` khi object có `error` và nhiều nhất hai khoá, cắt ở `--max-chars` ký tự (đếm theo ký tự như `len` của Python); log JSONL `--log` cùng khoá và thứ tự; hết stdin thì thoát 0. CLI: cùng lệnh, cùng cờ (viết tắt cờ được như `argparse`), cùng điều kiện dựng index (`build`, `--refresh`, DB chưa có), cùng đầu ra (`build` hai dòng, `build --json` thụt lề 1 không xuống dòng cuối, JSON thụt lề 1, `validate` dạng văn bản), `loops cần --max-len` thoát 1.

Khác Python (không có trên kg-solo; không cần ngoại lệ có tên):
- Lỗi ngoài dự kiến của CLI in `pkm: <lý do>` và thoát 1 (Python in traceback, cũng thoát 1). Thiếu tham số vị trí (`note` không có NODE) và `select` có mục không dạng `KHOÁ=GIÁ_TRỊ` báo lỗi như vậy thay vì `IndexError`, `ValueError`.
- MCP server không chết ở các chỗ Python ném ngoại lệ ra khỏi vòng đọc: `params` không phải object coi như rỗng; dòng không phải UTF-8 thành `-32700`; không ghi được file `--log` thì báo ra stderr rồi chạy tiếp.
- Thêm `pkm mcp install …` và `pkm doctor …` ở đầu dòng lệnh. Như Python, chữ thứ hai là `mcp` thì luôn là MCP server; vault tên `mcp` chỉ đụng với `pkm mcp install`, vault tên `doctor` chỉ đụng khi chữ sau là cờ (`pkm doctor --json`). Ghi `./mcp`, `./doctor` để tránh.
- `--ohs-cli`, `--ohs-node` (và `KG_OHS_CLI`, `KG_OHS_NODE`) chuyển nguyên cho tầng tool; nghĩa mới (gói OHS, PRD mục 9) thuộc tầng tool.

Nguồn: `scripts/kg/tests/test_mcp.py` (13 test). Chạy: `cargo test -p pkm` (lọc `mcp::`, `cli::`, `--test mcp_stdio`, `--test mcp_tools`, `--test cli_commands`). `tests/mcp_tools.rs` chạy binary thật trên vault `NOTES` của `test_kg.py` (bỏ các note `Lỗi/`), như `Client` của Python.

| Test Python | Test Rust | Trạng thái | Ghi chú |
|---|---|---|---|
| `Protocol.test_handshake` | `mcp::tests::handshake` | đã chuyển | Kiểm thêm từng byte của bản tin trả lời |
| `Protocol.test_old_protocol_version_echoed` | `mcp::tests::old_protocol_version_echoed`, `tests/mcp_stdio.rs` | đã chuyển | Bản qua binary thật kiểm cả thoát khi stdin đóng |
| `Protocol.test_tools_list` | `tests/mcp_tools.rs::tools_list`, `mcp::tests::tools_list_comes_from_the_tool_layer` | đã chuyển | Câu chữ và thứ tự khoá so với golden qua bộ so khớp (`mcp/tools_list`) |
| `Protocol.test_find_and_note` | `tests/mcp_tools.rs::find_and_note` | đã chuyển | |
| `Protocol.test_unknown_node` | `tests/mcp_tools.rs::unknown_node`, `mcp::tests::tool_results_are_compacted_and_errors_flagged` | đã chuyển | |
| `Protocol.test_neighbors_layers_window` | `tests/mcp_tools.rs::neighbors_layers_window` | đã chuyển | |
| `Protocol.test_reach_paths_loops` | `tests/mcp_tools.rs::reach_paths_loops` | đã chuyển | |
| `Protocol.test_provenance_select_valid` | `tests/mcp_tools.rs::provenance_select_valid` | đã chuyển | |
| `Protocol.test_more` | `tests/mcp_tools.rs::more` | đã chuyển | |
| `Protocol.test_related` | — | chưa chuyển | Cần tool `related` của lõi (hiện trả `NotImplementedError`) |
| `Refresh.test_edit_visible_on_next_call` | `tests/mcp_tools.rs::edit_visible_on_next_call`, `mcp::tests::edit_visible_on_next_call`, `mcp::tests::packs_come_from_the_previous_build` | đã chuyển | |
| `FixtureOverMcp.test_answer_key_over_protocol` | `tests/mcp_tools.rs::answer_key_over_protocol` | đã chuyển | Cần `PKM_KG_SOLO=<vault kg-solo>`, không có thì bỏ qua (Python bỏ qua khi thiếu node); CI so cùng bộ câu qua bộ so khớp |
| `FixtureOverMcp.test_related_on_fixture` | — | chưa chuyển | Cần tool `related` của lõi |

Test Rust thêm, không có bản Python: `mcp::tests::{protocol_edges_like_python, long_results_are_cut_at_max_chars_counting_characters, without_tool_layer_calls_report_index_unavailable, every_tool_call_is_logged_as_jsonl, missing_vault_reports_the_build_error_in_instructions, obsidian_vault_flag_names_or_disables_links, windows_drive_only_under_mnt_letter}`, `mcp::pyjson::tests::*` (`json.dumps`, `repr(float)`), `mcp::instructions::tests::*` (`urllib.parse.quote`), `cli::tests::*`, `tests/cli_commands.rs` (cả `graph_commands_query_the_index_like_python_cli`, `pkm mcp install --dry-run`, `pkm doctor --json`), `tests/mcp_stdio.rs::bad_flag_is_a_usage_error`.

## Module `related`

`pkm_core::related` là `scripts/kg/related.py` (`load_adjacency`, `ppr_push`, `_via`, `related`) cộng `Tools.related` của `scripts/kg/tools.py` (`hits_from`: hit dạng chuỗi, object `path`/`id` và `score`, danh sách `[đường dẫn, điểm]`). Nhánh `related` của `pkm_core::tools::call` là phần `related` của `Server.call`. Hằng số như Python: `WEIGHT` (accepted 1, superseded 0,3, proposed 0,5, nhân `score` của cạnh `proposed` trong (0, 1]), `alpha` 0,15, `eps` 1e-6, `max_pushes` 2.000.000, `k` mặc định 15, `via` tối đa 4 bước.

Điểm khớp Python từng bit chứ không chỉ tới 1e-6, vì mọi phép số thực đi đúng thứ tự Python (docs/DECISIONS.md, "Thứ tự liệt kê file"):
- cạnh đọc bằng đúng câu SQL của Python (không `ORDER BY`, thứ tự SQLite trả trên index dựng theo đường dẫn đã sắp);
- bậc của node và tổng điểm hạt giống cộng như `sum` của Python 3.12+ (bù sai số Neumaier, `related::py_sum`); cộng thẳng thì lệch ở bit cuối và có thể đổi số lần đẩy;
- hàng đợi FIFO và thứ tự chèn `p` như `deque` và `dict`; sắp ổn định theo điểm giảm dần; `round(x, 5)` làm tròn đúng trên giá trị nhị phân, hoà về số chẵn; hạt giống chưa có khối lượng có `score` là số nguyên `0` như `round(0, 5)`.

Đối chiếu với Python ở sha ghim trên kg-solo: 15 lời gọi `related` của golden khớp (so chặt cả thứ tự, `pushes`, `touched`, lệch số thực 0); thêm 400 lời gọi ngẫu nhiên chạy lại bằng `dump-python.py --replay` (1–8 hit đủ dạng, đường dẫn NFD, hit trùng, note không có, điểm là chuỗi, âm, 0, lớn hơn 1, `k` tới 100, `rels`, `statuses`, `accepted_only`, `as_of`, `types`, `system`): khớp hết, lệch số thực 0.

`related` chỉ có câu hỏi (không `hits`), khác Python theo D7 và PRD mục 9:
- Hạt giống lấy bằng `Ohs::search(query, {"limit": 10, "snippetLength": 0})` của OHS gắn vào phiên (`tools::Context::with_ohs`, `set_ohs`), đường dẫn đã NFC; `seed_source` giữ giá trị `ohs-cli` của Python để hợp đồng MCP không đổi.
- Phiên không gắn OHS, hoặc OHS chưa `ready` (thiếu gói, đang khởi động, đang index, thiếu model, hỏng): trả `ohs_not_ready` (mục `ohs` ở trên). Python lùi về FTS của DB OHS hoặc trả `{"error": "no OHS index reachable ..."}`.
- OHS `ready` mà không trả kết quả nào: `{"error": "no OHS index reachable ...", "detail": "ohs-cli"}` như Python. OHS báo lỗi, quá hạn, phản hồi sai: cùng lỗi đó, `detail` là `OHS lỗi: <lý do>` (Python: `OHS CLI lỗi: ...`).
- `query` không phải chuỗi: `TypeError` (Python cũng ném, ở `re.findall` hay `subprocess`).

Giới hạn đã biết: `k` là số thực thì Rust trả `TypeError` cả khi không có hạt giống nào (Python chỉ ném khi cắt danh sách), như mục `query`, `tools`. Điểm `NaN` hay vô cực trong hit (chuỗi `"nan"`, `"inf"`) cho thứ tự sắp không xác định ở cả hai bên.

Nguồn: `scripts/kg/tests/test_mcp.py` (`Protocol.test_related`, `FixtureOverMcp.test_related_on_fixture`), `scripts/e11/tests/test_kg_adapter.py` (`HiddenAndLayers.test_related_skips_system`). Chạy: `cargo test -p pkm-core related` và `cargo test -p pkm-core --test related_ohs_fake`.

| Test Python | Test Rust | Trạng thái | Ghi chú |
|---|---|---|---|
| `Protocol.test_related` | `tools::tests::{call_related_with_hits, call_related_query_without_ohs_is_not_ready}`, `related::tests::{seeds_reranked_missing_reported_and_graph_notes_found, types_and_accepted_only_filter_graph_notes}` | đã chuyển | Câu `query='lead'` không có OHS: Python kiểm lỗi có chữ `OHS`; Rust trả `ohs_not_ready`, `state: broken` (D7) |
| `FixtureOverMcp.test_related_on_fixture` | bộ so khớp kg-solo (`tools/related`) | đã chuyển | Golden so chặt hơn test Python (tập note, thứ hạng, điểm) |
| `HiddenAndLayers.test_related_skips_system` | `tools::tests::related_skips_system` | đã chuyển | |

Test Rust thêm, không có bản Python: `related::tests::{sum_compensates_like_python_312, round_half_even_on_the_binary_value, hit_shapes_like_tools_related, type_filter_like_python_in, no_known_hit_gives_empty_result_without_method, nfd_hit_paths_match_nfc_note_ids, bad_hit_values_raise_like_python, isolated_seed_keeps_all_its_mass, push_spreads_mass_both_ways_and_conserves_it, max_pushes_bounds_the_work}`, `tests/related_ohs_fake.rs` (hạt giống từ OHS giả ở dạng NFD, `indexing` thành `ohs_not_ready`, OHS báo lỗi, note OHS trả không có trong index, phiên không có OHS), `tests/related_ohs_real.rs` (`#[ignore]`, gói OHS thật; đã chạy qua trên Linux với gói `0.17.2-vantt.1`).
