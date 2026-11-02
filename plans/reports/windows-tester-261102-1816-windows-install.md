# Báo cáo: cài và thử `pkm` v0.1.0-rc.1 trên Windows thật

Ngày 2026-11-02, giờ Asia/Saigon. Làm theo [handoff-windows-test.md](../261102-0535-rust-core-port-master/handoff-windows-test.md) ở commit `01ddd4e`. Agent chạy trong WSL trên chính máy Windows đó, gọi Windows qua `powershell.exe`. Vì agent chạy bên trong Claude Desktop, các bước phải tắt hẳn Claude Desktop (A3 cài thật, A6, A7) do user chạy bằng script agent soạn sẵn (`C:\pkm-test\desktop-a3.ps1`, `desktop-a7.ps1`); agent đọc lại file kết quả. Vault của user gọi là "bản B"; mọi lệnh trên bản B chỉ ghi số đếm và trạng thái.

## 1. Máy

| Mục | Giá trị |
|---|---|
| Windows | Windows 11 Pro N, 10.0.26200; PowerShell 5.1.26100 |
| CPU | AMD Ryzen 7 PRO 6850HS |
| Python lúc làm phần A | **Có**: Python 3.13.1 (`C:\Python313`), PyYAML 6.0.3 ở thư mục site của người dùng. Nghiệm thu "Windows sạch" của phase 13 vì vậy chỉ đạt một phần |
| Claude Desktop | Bản Microsoft Store 4.4212.0.0; cấu hình ở `%APPDATA%\Claude\claude_desktop_config.json` (không ở `%LOCALAPPDATA%\Packages\Claude_*`) |
| `gh` | 2.83.2, đã đăng nhập |
| Khác | Rust stable MSVC, Node 26 qua nvm4w, Git for Windows với `core.autocrlf=true`, `LongPathsEnabled = 1` |

## 2. Binary

- Nguồn: Release `v0.1.0-rc.1` (pre-release, tag trỏ commit `b8995f7`). Bộ cài tải bằng `gh release download … -D` rồi chạy bằng `-File` (chạy kiểu `| iex` bị bộ lọc quyền của agent chặn, nên đọc hết file trước rồi mới chạy).
- Bộ cài: `checksum ok`, rồi `installed: pkm 0.1.0-rc.1 (spec 223c9c33c99c6a087d474b66da78736ad18f849c) -> C:\Users\<user>\AppData\Local\pkm\bin\pkm.exe`, rồi thêm thư mục vào PATH. Tải ẩn danh báo HTTP 404 (repo riêng tư) và tự chuyển sang `gh`, đúng thiết kế. Chạy lại: `pkm v0.1.0-rc.1 is already installed …; nothing to do`.
- `pkm --version`: `pkm 0.1.0-rc.1 (spec 223c9c33c99c6a087d474b66da78736ad18f849c)`.
- Phần B dựng từ tag đó: `C:\src\obsidian-pkm` ở `b8995f7`, `cargo build --locked -p pkm` và `-p pkm-parity` qua (1 phút 39 giây và 39 giây).

## 3. Nghiệm thu

| Phase | Kết quả | Bằng chứng |
|---|---|---|
| 02 | pass | Bài thử S1 trên bản B: 2.079 note, đều có trong bản tham chiếu; dạng YAML: 1.914 note có frontmatter, 1.587 frontmatter CRLF, 633 chuỗi không ASCII, 3.810 ngày giờ để dạng chuỗi, 2 null. Dòng "NOTES THE INDEX WOULD SEE DIFFERENTLY": 0 ở cả năm ứng viên, gồm `unsafe-libyaml+layer`. Chỉ chạy được khi tránh lỗi 4.1 và 4.2 |
| 03 | pass | Claude Desktop Store 4.4212 trên Windows 11 26200; `kg` (pkm) chạy; câu 2 ở A6 trả đúng `Người/Nguyễn Văn Đức.md`, dấu còn nguyên. Không cần chạy hai server thử của `probes/s2-mcp` |
| 05 | pass | `parity.py run --private <B> … --against python`: 54 mục, 0 lệch chưa có tên, 0 mục chưa có, mã 0 (chạy với `-X utf8`, xem 4.2; chạy đúng lệnh của handoff thì cũng 54 mục 0 lệch nhưng sập ở dòng in cuối, mã 1). `git grep -F` 25 tên note lấy ngẫu nhiên từ bản B trong repo này: 25 tên, 0 tên có kết quả |
| 08 | pass | Python với Rust trên bản B: `nodes` 2.079, `props` 8.967, `links` 7.336, `raw_links` 7.336, `blocks` 0, `names` 2.576, mọi bảng `schema_*`: khớp, 0 lệch (sau khi clone spec không đổi CRLF, xem 4.3) |
| 09 | pass | `edges` 4.788, `meta` 16, `v3_deviations` 1.887: khớp. Mã thoát của lệnh là 1, do hai lệch CLI ở dòng P10 |
| 10 | fail | Tầng tool (10 tool, 757 câu) và MCP (`initialize`, `tools_list`, `instructions`): 0 lệch. Tầng CLI: 2 lệch chưa có tên ở `build-json` và `stats`, cả hai là trường đường dẫn vault. Trên kg-solo lệch y hệt và xem được: phía Python in đường dẫn vault chưa che, phía Rust in `<VAULT>`. Đây là lỗi của bộ so khớp (4.4), không phải của `pkm` |
| 11 | pass | Gói OHS `0.17.2-vantt.3` (sha256 khớp bản ghim) chạy trên Windows thật. `related "website"` trên vault thử: 9,8 giây, nguồn `ohs-cli`. B3 trên kg-solo (5 câu `khách hàng`, `hợp đồng`, `báo giá`, `quy trình`, `Đà Nẵng`): hạt giống của `pkm` và `ohs_seeds` của Python ở `223c9c3` trùng tập và thứ tự ở cả năm câu, cả hai nguồn `ohs-cli`; câu đầu 78,8 giây (OHS index 746 note), các câu sau 6–14 giây. `doctor --deep` sau khi cài: `ohs_state ok` (`ready`) sau 2,4 giây trên vault thử, 1,3 giây trên bản B (index OHS của bản B có sẵn). Không còn `node` mồ côi sau mỗi lệnh |
| 13 | pass | Máy có Python (mục 1). Bộ cài như mục 2. `--check`: `kg 0.1.0, 11 tool`. `doctor --json` sau A3: `ok: true`; `claude_config`, `kg_entry`, `db`, `ohs_package`, `ohs_node`, `long_paths` đều `ok`, `ohs_state` `skip` (không `--deep`). Bốn phép kiểm A3 qua (mục 3.1). A6 đúng 7/7 (mục 3.2). A7 trên bản B: mục 3.3 |

### 3.1. Bốn phép kiểm của A3

1. `--dry-run` không ghi: `Get-FileHash` của file cấu hình trước và sau bằng nhau.
2. Claude Desktop đang mở: `pkm mcp install` từ chối, mã 1, in đúng exe `C:\Program Files\WindowsApps\Claude_4.4212.0.0_x64__pzs8sxrjxfjjc\app\Claude.exe`. Máy có Claude Code ở `~\.local\bin\claude.exe` đang chạy cùng lúc; `pkm` không nhận nhầm nó (Claude Code này không cài bằng winget, nên phép thử winget vẫn chưa có).
3. Chép `pkm.exe` vào `Downloads` rồi chạy `mcp install … --dry-run`: cảnh báo đúng (binary ở thư mục tải về, sẽ từ chối nếu không có `--allow-any-location`, in lệnh bộ cài). Đã xoá bản chép.
4. `mcp install --remove --dry-run` chỉ nhắm mục `kg`.

Cài thật (Claude Desktop tắt): mã 0, 37,1 giây gồm tải gói OHS từ Release (`sha256 của gói khớp bản ghim (ad63f1a5…)`), dựng index 6 note và ghi mục `kg` (có sao lưu). Mục `kg` cũ là MCP Python chạy qua `wsl.exe`; user đồng ý thay. Bản sao cấu hình trước khi thử ở `C:\pkm-test\claude_desktop_config.before.json`.

### 3.2. A6: Claude Desktop trên vault thử

| # | Tool | Kết quả |
|---|---|---|
| 1 | `select` theo từng type | Đúng: 6 note, person 2, tool 2, project 1, concept 1. Claude tự đếm bằng `select` thay vì đọc số trong `instructions` |
| 2 | `find` | Đúng: `Người/Nguyễn Văn Đức.md`, person |
| 3 | `neighbors` | Đúng: `uses` Obsidian, SQLite; `related` Nguyễn Văn Đức; cả ba `accepted` |
| 4 | `paths` | Đúng: một đường Mai, Đức, Website Mới, Obsidian, Markdown; `related`, `related`, `uses`, `uses` |
| 5 | `related` có `hits` | Đúng: Obsidian và Đức đồng hạng 0,156, SQLite 0,113, rồi Markdown (cùng điểm với Mai, 0,058) |
| 6 | `related` chỉ có câu hỏi | Đúng: OHS chạy, Website Mới đứng đầu (0,388), điểm trùng lần chạy CLI ở A5 |
| 7 | `paths` với `max_len 13` | Đúng: lỗi trần 12; Claude chạy lại với 12 và server vẫn trả lời |

Thoát Claude Desktop rồi đợi 5 giây: 0 tiến trình `pkm.exe` hay `node.exe` của `%LOCALAPPDATA%\pkm`. Không tìm thấy file `mcp-server-kg.log` ở `%APPDATA%\Claude\logs` (bản Store 4.4212 không ghi log riêng cho `kg` ở đó), nên không có log tool để kèm; cả bảy câu đều đúng nên không cần.

### 3.3. A7: bản B qua Claude Desktop

Lệnh như handoff, thêm `--about` lấy nguyên từ mục Python cũ. Mã 0; dựng index lần đầu 2.079 note, cả lệnh 4,1 giây. `--check`: 11 tool. `doctor`: `ok=True`, sáu mục `ok`, `ohs_state` `skip`; `doctor --deep`: `ohs_state ok` sau 1,3 giây. User tự hỏi Claude Desktop vài câu trên bản B và đánh giá: ổn.

rc.1 chưa có cờ `--ohs`, nên `pkm` dùng gói OHS thứ hai ở `%LOCALAPPDATA%\pkm\ohs` cạnh `C:\tools\ohs` của dự án. Hai bản cùng `0.17.2-vantt.3`, cùng gói tiếng Việt, cùng mẫu bỏ qua mặc định, nên dùng chung DB OHS của bản B mà index không bị dựng lại. Agent cài fork đã hoãn nâng bản B lên `vantt.4` trong lúc thử.

## 4. Lỗi và chỗ lệch kỳ vọng

Mọi lỗi dưới đây nằm ở công cụ thử, không ở binary `pkm`.

1. **`probes/s1-yaml` không thấy PyYAML.** `compare … --python python` báo `pyyaml-dump.py failed (exit code: 1)`. Chạy trên vault thử: `ModuleNotFoundError: No module named 'yaml'`. Probe gọi Python với `-I`, mà `-I` bỏ thư mục site của người dùng, nơi `pip install --user` đặt PyYAML trên Windows. Tránh bằng venv riêng (`C:\pkm-test\venv`, PyYAML 6.0.3 có libyaml, như Python của máy).
2. **In tiếng Việt ra pipe sập trên Windows.** `pyyaml-dump.py` và `parity.py` (dòng `print` cuối của `cmd_run`) báo `UnicodeEncodeError: 'charmap' codec can't encode character …`: stdout bị pipe dùng `cp1252`, và `-I` cũng bỏ `PYTHONIOENCODING`. Tránh bằng `C:\pkm-test\py-utf8.cmd` gọi `python.exe -X utf8 %*`. Nên sửa ở hai script (`sys.stdout.reconfigure(encoding='utf-8')` hoặc ghi bytes UTF-8).
3. **`schema_digest` lệch vì CRLF.** Git for Windows mặc định `core.autocrlf=true`, nên checkout spec có `docs/knowledge-graph/schema/schema.yaml` dạng CRLF; Python băm file đó, Rust băm bản nhúng lúc build (LF). Lần đầu ra 5 lệch (`index/meta` `schema_digest`, và `schema_digest` trong `cli/build-json`, `cli/stats`). Clone spec bằng `git -c core.autocrlf=false clone …` thì hết. Handoff mục 4 nên ghi cờ này.
4. **`Scrub` của `dump-python.py` không che đường dẫn Windows trong JSON.** `cli/build-json.out` và `cli/stats.out` của Python giữ `"C:\\…\\kg-solo"` (dấu `\` nhân đôi), còn phía Rust in `<VAULT>`. Phía Rust đã sửa đúng lỗi này (commit "fix(parity): mask JSON-escaped Windows vault paths in CLI output"); phía Python chưa. Đổi `TEMP` sang đường dẫn không có tên ngắn 8.3 không thay đổi kết quả. Đây là hai lệch còn lại của P10.
5. **`npx` của máy hỏng** (`EPERM: operation not permitted, lstat 'C:\Users\Admin\AppData'`, cấu hình npm của nvm4w trỏ sang hồ sơ người dùng khác). Không phải lỗi của `pkm`. Bộ so khớp kg-solo chạy bằng `--otf C:\src\otf` (clone `obsidian-test-fixture` ở `v0.7.0`). kg-solo cho B3 sinh trong WSL với đúng tham số `fixture-args` rồi chép sang `C:\pkm-test\kg-solo`.
6. **`gh repo clone` hỏng trên máy này** (`failed to run git: exit status 128`) dù `gh auth status` đã đăng nhập; `git clone https://…` sau `gh auth setup-git` thì được.
7. **Hiển thị:** dòng `ohs_node` của `doctor` in `…\pkm\ohs\node/node.exe` (lẫn `\` và `/`).
8. **Trên kg-solo** (Python với Rust, sau khi sửa 4.3): 64 mục, 2 lệch chưa có tên, chính là lỗi 4.4. Hiệu năng build đầy đủ: Python 1.093 ms, Rust 1.789 ms (debug build).

## 5. Trạng thái cuối của máy

- `pkm` 0.1.0-rc.1 còn cài ở `%LOCALAPPDATA%\pkm\bin`, có trong PATH người dùng; gói OHS `0.17.2-vantt.3` ở `%LOCALAPPDATA%\pkm\ohs`; index `kg` ở `%LOCALAPPDATA%\pkm\kg`.
- Mục `kg` của Claude Desktop trỏ bản B qua `pkm`; user chọn **giữ** (không trả lại MCP Python). Nếu cần trả lại: tắt Claude Desktop, chép `C:\pkm-test\claude_desktop_config.before.json` đè lên file cấu hình.
- Thư mục thử: `C:\pkm-test` (vault thử, kg-solo, venv, gói OHS giải nén, script, kết quả), `C:\src\obsidian-pkm`, `C:\src\spec-223c9c3`, `C:\src\otf`.

```text
Status: DONE_WITH_CONCERNS
Summary: pkm v0.1.0-rc.1 cài, chạy MCP trong Claude Desktop và chạy OHS trên Windows thật; index, tool và MCP khớp Python trên bản B. Hai lệch CLI và ba lỗi chạy công cụ thử là của bộ so khớp và probe trên Windows, không phải của pkm.
P02: pass - 2.079 note, unsafe-libyaml+layer 0 lệch (cần venv và -X utf8 để chạy probe)
P03: pass - Claude Desktop Store 4.4212, Windows 11 26200; find 'duc' trả đúng id có dấu
P05: pass - 54 mục 0 lệch, mã 0; git grep 25 tên bản B, 0 kết quả
P08: pass - nodes, props, links, raw_links, blocks, names, schema_* khớp trên bản B
P09: pass - edges, meta, v3_deviations khớp; mã thoát 1 chỉ do hai lệch CLI của P10
P10: fail - tool và MCP 0 lệch; CLI build-json, stats lệch đường dẫn vault do Scrub của dump-python.py
P11: pass - OHS vantt.3 chạy trên Windows; related chỉ có câu hỏi khớp Python 5/5 câu kg-solo; doctor --deep ready; không node mồ côi
P13: pass - cài, --check 11 tool, doctor ok, A6 7/7; máy có Python nên "Windows sạch" chỉ một phần
Report: plans/reports/windows-tester-261102-1816-windows-install.md (nhánh test/windows-install-results)
Concerns/Blockers: sửa dump-python.py (Scrub JSON, in UTF-8), pyyaml-dump.py (in UTF-8, không phụ thuộc site người dùng), handoff ghi clone spec với core.autocrlf=false; A7: user đánh giá câu trả lời trên bản B là ổn và giữ pkm
```
