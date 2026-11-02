# Bàn giao: cài và thử `pkm` trên Windows thật

Tài liệu này dành cho một agent chạy trên máy Windows của user, không có ngữ cảnh nào khác. Đọc hết một lượt trước khi làm. Làm theo đúng thứ tự các phần, vì phần A cần máy còn sạch (chưa có Python), còn phần B cài thêm công cụ cho máy.

## 1. Mục tiêu và phạm vi

`pkm` là binary Rust thay cho MCP `kg` viết bằng Python: nó đọc vault Obsidian, dựng index graph SQLite và phục vụ Claude Desktop qua MCP. Mã đã xong tới bộ cài; những gì còn thiếu chỉ kiểm được trên Windows thật và trên vault thật của user (gọi là bản B). Việc của bạn:

1. Cài `pkm` trên Windows sạch bằng bộ cài, nối vào Claude Desktop, kiểm `pkm doctor`.
2. Hỏi Claude Desktop trên một vault thử và so câu trả lời với kết quả đúng ghi ở mục 5.
3. Cài gói OHS (tìm kiếm ngữ nghĩa) và kiểm `related` khi chỉ có câu hỏi.
4. Chạy bộ so khớp và bài thử YAML trên bản B.
5. Ghi bằng chứng vào một báo cáo và trả về dòng trạng thái (mục 9).

Ngoài phạm vi: sửa mã, sửa workflow, gắn tag, tạo Release, mở issue, đổi `docs/`, `spec/`. Gặp lỗi thì ghi lại, không tự sửa.

**Quy tắc riêng tư cho bản B, không có ngoại lệ:** mọi lệnh chạy trên bản B chỉ được in số đếm. Không in, không chép, không ghi vào báo cáo hay commit bất kỳ tên note, đường dẫn note, nội dung, đường dẫn vault, khoá frontmatter hay giá trị nào của bản B. Không dùng `--show` của bài thử YAML, không dùng `--keep` của bộ so khớp trên bản B, không chép log của Claude Desktop khi `kg` đang trỏ vào bản B. Trong báo cáo, gọi vault đó là "bản B".

## 2. Điều kiện trước

| Thứ | Vì sao | Cách có |
|---|---|---|
| Tài khoản Windows thường (không chạy PowerShell với quyền admin) | Bộ cài từ chối phiên admin | — |
| GitHub CLI `gh` đã đăng nhập | Repo `vantt/obsidian-pkm` là private. Gói OHS thì không cần `gh`: `pkm` tải từ Release công khai của fork | `winget install --id GitHub.cli`, mở PowerShell mới, `gh auth login`, rồi `gh auth status` |
| Claude Desktop (bản cho Windows) | Phần A | Trang tải của Anthropic |
| Đường dẫn vault bản B | Phần B và bước cuối phần A | Hỏi user. Chỉ giữ trong biến `$B` của phiên, không ghi ra file |
| Quyền ghi lên repo (`gh` của user) | Chạy `workflow_dispatch` khi chưa có Release | Tài khoản của user |

Phần B cần thêm (chỉ cài sau khi xong phần A): Git, Python 3.11 trở lên có PyYAML, Node 20 trở lên (có `npx`), Rust qua rustup bản MSVC (rustup sẽ đòi Visual Studio Build Tools có C++ nếu máy chưa có; `rust-toolchain.toml` ở gốc repo tự kéo đúng phiên bản).

Mọi lệnh dưới đây là PowerShell. Thư mục làm việc gợi ý là `C:\pkm-test`, ngoài mọi vault.

## 3. Phần A: cài trên Windows sạch

### A0. Ghi hiện trạng máy

1. Kiểm máy có Python không: `Get-Command python, python3, py -ErrorAction SilentlyContinue`. Nghiệm thu phase 13 cần máy không có Python; nếu máy đã có, ghi rõ trong báo cáo (phần A vẫn làm được, nhưng nghiệm thu "Windows sạch" khi đó chỉ là một phần).
2. Thoát hẳn Claude Desktop (cả biểu tượng ở khay hệ thống).
3. Sao một bản cấu hình Claude Desktop để trả lại khi xong: `Copy-Item "$env:APPDATA\Claude\claude_desktop_config.json" C:\pkm-test\claude_desktop_config.before.json`. Bản Microsoft Store nằm ở `%LOCALAPPDATA%\Packages\Claude_*\LocalCache\Roaming\Claude\`.
4. Ghi vào báo cáo: có mục `kg` sẵn không, và nếu có thì nó chạy Python hay `pkm` (chỉ ghi loại, không ghi `args`, vì `args` có thể chứa đường dẫn bản B). **Nếu đã có mục `kg` chạy Python, hỏi user trước khi thay**, vì `pkm mcp install` sẽ thay đúng mục đó.

### A1. Lấy binary

Hiện **chưa có tag và chưa có Release nào** (`gh release list -R vantt/obsidian-pkm` rỗng). Kiểm lại trước:

```powershell
gh release list -R vantt/obsidian-pkm
```

**Nếu đã có Release** (user gắn tag sau ngày viết tài liệu này; bản đầu dự kiến là pre-release `v0.1.0-rc.1`), dùng bộ cài của Release như README. Pre-release không phải bản "latest", nên phải ghi tag:

```powershell
gh release download v0.1.0-rc.1 --repo vantt/obsidian-pkm --pattern install.ps1 --output - | Out-String | iex
```

**Nếu chưa có Release** (trường hợp hiện tại), dùng artifact `release-assets` của workflow Release chạy bằng `workflow_dispatch`. Artifact này có bốn binary, `install.sh`, `install.ps1` đã đóng dấu phiên bản và `checksums.txt`, giống hệt bộ file của một Release.

1. Kiểm CI của `main` xanh. Nếu lần chạy mới nhất đỏ, dừng và báo `BLOCKED`, vì workflow Release chạy lại toàn bộ CI và sẽ đỏ theo.
   ```powershell
   gh run list -R vantt/obsidian-pkm --branch main --workflow ci.yml -L 3
   ```
2. Chạy workflow Release trên `main`, lấy id của lần chạy, chờ xong:
   ```powershell
   gh workflow run release.yml -R vantt/obsidian-pkm --ref main
   gh run list -R vantt/obsidian-pkm --workflow release.yml -L 1
   gh run watch <RUN_ID> -R vantt/obsidian-pkm --exit-status
   ```
   Có thể đã có một lần chạy `workflow_dispatch` thành công trên đúng commit đầu của `main`; khi đó dùng lại `RUN_ID` của nó. Ghi `RUN_ID` và `headSha` vào báo cáo: `gh run view <RUN_ID> -R vantt/obsidian-pkm --json headSha,conclusion`.
3. Tải artifact và chạy bộ cài với `-LocalFixture` (bộ cài lấy file từ thư mục này thay vì tải, vẫn kiểm sha256 theo `checksums.txt`):
   ```powershell
   gh run download <RUN_ID> -R vantt/obsidian-pkm -n release-assets -D C:\pkm-test\assets
   powershell -NoProfile -ExecutionPolicy Bypass -File C:\pkm-test\assets\install.ps1 -LocalFixture C:\pkm-test\assets
   ```
   Kết quả đúng: dòng `installed: pkm 0.1.0-rc.1 (spec 223c9c3…) -> C:\Users\<user>\AppData\Local\pkm\bin\pkm.exe` và dòng thêm thư mục đó vào PATH. Mở PowerShell mới rồi `pkm --version`.

Tuỳ chọn khác của bộ cài (`-Version`, `-InstallDir`, `-McpInstall`, `-NoModifyPath`, `-DryRun`, `-Uninstall`, `-Yes`, biến `PKM_*` tương ứng) ghi ở đầu [scripts/install.ps1](../../scripts/install.ps1). Chạy lại đúng lệnh trên lần nữa phải in `already installed … nothing to do`.

### A2. Vault thử

Tạo vault thử để kiểm Claude Desktop mà không đụng bản B. File phải là UTF-8 **không BOM**: `pkm` giữ BOM như Python, nên BOM đứng trước `---` làm frontmatter không được đọc. Vì vậy đừng dùng `Set-Content -Encoding utf8` của PowerShell 5.1. Dán khối dưới thẳng vào cửa sổ PowerShell; nếu lưu thành file `.ps1` thì lưu UTF-8 có BOM, vì PowerShell 5.1 đọc file không BOM theo bảng mã cũ và làm hỏng dấu tiếng Việt.

```powershell
$v = 'C:\pkm-test\vault-thu'
$utf8 = New-Object System.Text.UTF8Encoding $false
function Note($rel, $text) { $p = Join-Path $v $rel; New-Item -ItemType Directory -Force (Split-Path $p) | Out-Null; [IO.File]::WriteAllText($p, $text, $utf8) }
Note 'Dự án\Website Mới.md' "---`ntype: project`nuses: [`"[[Obsidian]]`", `"[[SQLite]]`"]`nrelated: [`"[[Nguyễn Văn Đức]]`"]`n---`nDự án làm lại website.`n"
Note 'Công cụ\Obsidian.md' "---`ntype: tool`nuses: [`"[[Markdown]]`"]`n---`nỨng dụng ghi chú.`n"
Note 'Công cụ\SQLite.md' "---`ntype: tool`n---`nCơ sở dữ liệu nhúng.`n"
Note 'Khái niệm\Markdown.md' "---`ntype: concept`n---`nCú pháp văn bản.`n"
Note 'Người\Nguyễn Văn Đức.md' "---`ntype: person`n---`nTrưởng dự án.`n"
Note 'Người\Trần Thị Mai.md' "---`ntype: person`nrelated: [`"[[Nguyễn Văn Đức]]`"]`n---`nĐồng nghiệp.`n"
```

Kiểm nhanh bằng CLI (index tạm, ngoài vault): `pkm C:\pkm-test\vault-thu build --db C:\pkm-test\thu.db` phải báo `6 note` và `cạnh theo lớp: {'accepted': 5}`.

### A3. `pkm mcp install`

Claude Desktop phải đang tắt hẳn. Xem trước, không ghi gì:

```powershell
pkm mcp install C:\pkm-test\vault-thu --dry-run
```

Rồi cài thật. Lần cài đầu dựng index vào `%LOCALAPPDATA%\pkm\kg\<tên vault>-<hash>.db` (ngoài vault), cài gói OHS (mục A5), ghi mục `kg` và giữ nguyên mọi mục khác (kể cả `ohs`), sao lưu file cũ thành `<file>.bak-<thời điểm UTC>`:

```powershell
pkm mcp install C:\pkm-test\vault-thu
pkm mcp install --check
```

`--check` bắt tay với server `kg` đã cấu hình như Claude Desktop và phải báo đủ 11 tool.

Cờ của lệnh (đúng theo `pkm mcp install --help`): `--db PATH`, `--packs`, `--no-aliases`, `--edge-keys schema|links`, `--body-edges`, `--adapter`, `--about TEXT`, `--config FILE`, `--check`, `--dry-run`, `--remove`, `--force`, `--allow-any-location`, `--ohs DIR`, `--ohs-package PATH|URL`, `--no-ohs`, `--timeout SECS` (mặc định 120). Hai cờ dễ nhầm:

- `--force` chỉ bỏ qua bước kiểm Claude Desktop đang chạy. Claude Desktop ghi đè cấu hình khi thoát, nên chỉ dùng khi chắc nó đã tắt mà `pkm` vẫn nhận nhầm.
- `--allow-any-location` cho phép ghi khi `pkm.exe` đang chạy không nằm ở chỗ cài chuẩn (`%LOCALAPPDATA%\pkm\bin`, `PKM_INSTALL_DIR`, hoặc thư mục có `.pkm-managed`). Cờ này không bỏ qua kiểm Claude Desktop.

Các phép kiểm hành vi của bộ cài (ghi kết quả từng phép):

1. `--dry-run` không ghi: so `Get-FileHash` của file cấu hình trước và sau.
2. Mở Claude Desktop rồi chạy lại `pkm mcp install C:\pkm-test\vault-thu`: phải từ chối và in đường dẫn exe bị coi là Claude Desktop. Nếu máy có Claude Code (đặc biệt bản cài bằng winget), ghi lại `pkm` có nhận nhầm Claude Code là Claude Desktop không.
3. Chép `pkm.exe` vào `$env:USERPROFILE\Downloads` rồi chạy bản đó với `mcp install C:\pkm-test\vault-thu --dry-run`: phải cảnh báo binary nằm ngoài chỗ cài chuẩn. Xoá bản chép sau khi thử.
4. `pkm mcp install --remove --dry-run` chỉ nhắm mục `kg`. Không chạy `--remove` thật ở đây; bước cuối (mục 3, A7) mới gỡ hay trả lại.

### A4. `pkm doctor`

```powershell
pkm doctor
pkm doctor --json
```

Đúng: mã thoát 0, `ok: true`, không mục nào `fail`. Mã mục: `claude_config`, `kg_entry`, `db`, `ohs_package`, `ohs_node`, `ohs_state`, `long_paths`. Thiếu gói OHS chỉ là `warn`. Mục `long_paths` báo Windows có bật đường dẫn dài không; ghi kết quả. Cờ khác: `--config FILE`, `--db PATH`, `--ohs DIR`, `--deep`, `--ohs-wait SECS` (mặc định 15).

### A5. Gói OHS

OHS là bản fork `vantt/obsidian-hybrid-search`, gói dựng sẵn có Node và gói tiếng Việt (`lang\ohs-lang-vi`) đi kèm, cài vào `%LOCALAPPDATA%\pkm\ohs`. `pkm` ghim Release `v0.17.2-vantt.4` và chạy mọi tiến trình OHS với `OHS_LANGUAGE_PACK` trỏ vào gói tiếng Việt của gói. Ba đường, theo thứ tự ưu tiên:

1. **Tự tải từ GitHub Release** (mặc định của `pkm mcp install` khi không có `--ohs-package`, `--no-ohs` hay `PKM_OHS_PACKAGE`): tải `ohs-0.17.2-vantt.4-win32-x64.zip` của Release `v0.17.2-vantt.4` (repo công khai, không cần đăng nhập, không cần `gh`) bằng `curl.exe` có sẵn trong Windows 10 trở lên, kiểm sha256 với bản ghim trong `pkm` (`ad63f1a59c06daa46d5c55c10d7eec1d912556b389f2c69062cbe46ed45e304d`). Ghi dòng `tải gói OHS … từ …` và `sha256 của gói khớp bản ghim` vào báo cáo.
2. **File tải tay**, khi đường 1 hỏng (máy chặn GitHub, proxy…): tải file bằng trình duyệt ở https://github.com/vantt/obsidian-hybrid-search/releases/tag/v0.17.2-vantt.4, hoặc:
   ```powershell
   curl.exe -fL -o C:\pkm-test\ohs-0.17.2-vantt.4-win32-x64.zip https://github.com/vantt/obsidian-hybrid-search/releases/download/v0.17.2-vantt.4/ohs-0.17.2-vantt.4-win32-x64.zip
   pkm mcp install C:\pkm-test\vault-thu --ohs-package C:\pkm-test\ohs-0.17.2-vantt.4-win32-x64.zip
   ```
   `--ohs-package` nhận file gói (phải khớp sha256 ghim), thư mục gói đã giải nén (có `node\` và `app\dist\`), hoặc URL `https://` của file gói kèm `<URL>.sha256`.
3. **Biến `PKM_OHS_PACKAGE`**: cùng dạng giá trị như `--ohs-package`, dùng khi không muốn sửa lệnh.

Máy đã có OHS của dự án (bản B: `C:\tools\ohs`, cũng là `0.17.2-vantt.4` có gói tiếng Việt) thì **không** cài thêm gói thứ hai cho vault đó: dùng `--ohs C:\tools\ohs` (mục A7). Hai bản OHS khác cấu hình gói ngôn ngữ cùng chạy trên một vault làm bên làm chủ index dựng lại bảng view qua lại, kết quả tìm đổi theo bên chạy sau.

`pkm doctor` báo ở `ohs_package` phiên bản gói và có gói tiếng Việt không; khác bản ghim hoặc thiếu gói tiếng Việt là `warn`. Ghi dòng này vào báo cáo.

Sau khi cài gói: `pkm doctor --deep` chạy thử OHS trên vault của mục `kg` (OHS có thể bắt đầu index và nạp model). Lần đầu thường là `warn` với trạng thái `starting` hoặc `indexing`; chạy lại sau một lúc phải thành `ok`. Ghi thời gian tới `ready` và mọi trạng thái `broken` hay `model_missing` kèm `action`.

### A6. Hỏi Claude Desktop

Mở Claude Desktop. Trong Settings, phần Developer, server `kg` phải ở trạng thái chạy. Hỏi từng câu trong một chat mới. Kết quả đúng dưới đây đã kiểm bằng chính tầng tool của `pkm` trên vault y hệt (CLI và MCP dùng chung tầng tool).

| # | Câu hỏi | Kết quả đúng |
|---|---|---|
| 1 | "Theo kg, vault có bao nhiêu note, mỗi loại bao nhiêu?" | 6 note: tool 2, person 2, project 1, concept 1 (lấy từ `instructions` của server) |
| 2 | "Dùng tool find của kg tìm 'duc'. Cho tôi id nhận được." | `Người/Nguyễn Văn Đức.md`, type `person`, dấu tiếng Việt còn nguyên |
| 3 | "Dùng kg neighbors cho 'Dự án/Website Mới.md': dự án này nối với note nào, qua quan hệ gì?" | `uses` tới `Công cụ/Obsidian.md` và `Công cụ/SQLite.md`; `related` tới `Người/Nguyễn Văn Đức.md`; cả ba ở lớp `accepted` |
| 4 | "Dùng kg paths tìm đường từ 'Người/Trần Thị Mai.md' tới 'Khái niệm/Markdown.md'." | Một đường: Trần Thị Mai, Nguyễn Văn Đức, Website Mới, Obsidian, Markdown; quan hệ `related`, `related`, `uses`, `uses` |
| 5 | "Dùng kg related với hits là 'Dự án/Website Mới.md'. Note nào liên quan nhất?" | Hạt giống là Website Mới; đứng đầu là `Công cụ/Obsidian.md` và `Người/Nguyễn Văn Đức.md` (cùng điểm 0.15586), rồi `Công cụ/SQLite.md` (0.11328), rồi `Khái niệm/Markdown.md` |
| 6 | "Dùng kg related với câu hỏi 'website', không đưa hits." | Khi OHS đã `ready`: `seed_source` là `ohs-cli`, hạt giống có `Dự án/Website Mới.md`. Khi OHS chưa xong: lỗi `ohs_not_ready` có đủ `state`, `progress`, `eta_s`, `action`; hỏi lại sau ít phút phải ra kết quả. `state: broken` kèm `action` cài đặt nghĩa là chưa có gói |
| 7 | "Dùng kg paths từ 'Người/Trần Thị Mai.md' tới 'Khái niệm/Markdown.md' với max_len 13." | Lỗi `max_len 13 is too large: at most 12 …`, server vẫn trả lời các câu sau |

Ghi cho mỗi câu: tool Claude đã gọi, câu trả lời (vault thử nên chép được), đúng hay sai. Lỗi thì chép phần cuối `%APPDATA%\Claude\logs\mcp-server-kg.log` (chỉ khi `kg` đang trỏ vào vault thử).

Sau cùng: thoát hẳn Claude Desktop, đợi vài giây, rồi `Get-Process node, pkm -ErrorAction SilentlyContinue`. Không được còn tiến trình `pkm` hay `node` nào do `kg` khởi động.

### A7. Bản B qua Claude Desktop, rồi trả lại máy

Chỉ làm khi user đồng ý thay mục `kg`. Claude Desktop tắt hẳn:

```powershell
pkm mcp install "$B" --adapter e11 --packs capture --no-aliases --edge-keys links --body-edges --ohs C:\tools\ohs
pkm mcp install --check
pkm doctor
```

`--ohs C:\tools\ohs` ghi `--ohs` vào mục `kg`, không cài gói vào `%LOCALAPPDATA%\pkm\ohs`: `pkm`, MCP `ohs` và script dự án dùng chung một bản OHS và một gói tiếng Việt. `doctor` phải báo `ohs_package` là `C:\tools\ohs (0.17.2-vantt.4, có gói tiếng Việt)`. `doctor --deep` trên bản B chỉ ghi trạng thái (`ready` hay không), không ghi `action` nếu nó chứa đường dẫn vault.

User tự hỏi Claude Desktop vài câu trên bản B và tự đánh giá. Báo cáo chỉ ghi: thời gian dựng index lần đầu, `--check` báo bao nhiêu tool, kết quả `doctor` theo mã mục, và user nói đúng hay sai (không chép câu hỏi hay câu trả lời).

Kết thúc: hỏi user giữ mục `kg` mới hay trả lại. Trả lại bằng cách tắt Claude Desktop rồi chép `C:\pkm-test\claude_desktop_config.before.json` đè lên file cấu hình. Gỡ `pkm` (khi user muốn) theo README: `pkm mcp install --remove`, rồi `install.ps1` với `-Uninstall`.

## 4. Phần B: bản B và bài thử (sau phần A)

Cài công cụ ở mục 2, rồi lấy mã đúng commit của binary đã thử và repo đặc tả ở sha ghim:

```powershell
gh auth setup-git
gh repo clone vantt/obsidian-pkm C:\src\obsidian-pkm
git -C C:\src\obsidian-pkm checkout <headSha của RUN_ID ở A1>
gh repo clone vantt/obsidian-pkm-automation C:\src\spec-223c9c3
git -C C:\src\spec-223c9c3 checkout 223c9c33c99c6a087d474b66da78736ad18f849c
python --version
python -c "import yaml; print(yaml.__version__)"
cd C:\src\obsidian-pkm
cargo build --locked -p pkm
```

Trên Windows, `python3` thường là lối tắt của Microsoft Store chứ không phải Python thật; dùng `python` (đã kiểm phiên bản ở trên). Sha ghim nằm trong `spec/spec.lock`. `cargo build -p pkm` đặt `pkm.exe` cạnh binary `parity` để bộ so khớp chạy được tầng MCP và CLI; thiếu nó thì hai tầng này báo "chưa có" (không tính là khớp). Cách khác là đặt `PKM_BIN` trỏ tới một `pkm.exe` cùng commit.

### B1. Bài thử YAML trên bản B (phase 02)

```powershell
cargo run --release --manifest-path probes/s1-yaml/Cargo.toml -- compare --spec C:\src\spec-223c9c3 --python python "$B"
```

Lệnh chỉ in loader, bản PyYAML, số note, số lần mỗi dạng YAML rủi ro xuất hiện và số lệch theo loại cho từng crate ứng viên. Không thêm `--show`. Đạt khi cột `unsafe-libyaml+layer` bằng 0 ở mọi loại. Ghi nguyên bảng số vào báo cáo (bảng này không chứa nội dung note).

### B2. Bộ so khớp trên bản B (phase 05, 08, 09, 10)

Python với chính nó (phase 05):

```powershell
python parity\parity.py run --private "$B" --adapter e11 --packs capture --no-aliases --edge-keys links --body-edges --spec-dir C:\src\spec-223c9c3 --against python
```

Python với Rust (phase 08 cho các bảng `nodes`, `props`, `links`, `raw_links`, `blocks`, `names`, `schema_*`; phase 09 cho mọi bảng còn lại và view `v3_deviations`; phase 10 cho bộ câu tool tự sinh):

```powershell
python parity\parity.py run --private "$B" --adapter e11 --packs capture --no-aliases --edge-keys links --body-edges --spec-dir C:\src\spec-223c9c3
```

`--private` chỉ in số đơn vị và số lệch theo bảng, op, tool, file MCP, lệnh CLI; index dựng vào thư mục tạm, không ghi gì vào vault. Đạt khi dòng tổng báo 0 lệch chưa có tên và mã thoát 0. Ghi bảng tóm tắt và mã thoát của từng lệnh; `parity\report.json` cũng chỉ có số, nhưng đừng commit nó. Nếu `parity.py` hỏng vì lý do riêng của Windows (đường dẫn, mã hoá, tiến trình con), ghi loại lỗi và dòng cuối của thông báo, không ghi traceback (traceback có thể chứa tên note).

Thứ hai (phase 05): xin user vài tên note của bản B, chạy `git -C C:\src\obsidian-pkm grep -n -F "<tên>"` cho từng tên, chỉ ghi "N tên, tất cả rỗng" hoặc số tên có kết quả. Không ghi tên.

Nên chạy thêm (không bắt buộc): `python parity\parity.py run --spec-dir C:\src\spec-223c9c3` trên kg-solo để có số so Python với Rust chạy trên Windows.

### B3. `related` chỉ có câu hỏi so với Python (phase 11)

Chỉ làm trên kg-solo (vault giả, ghi được kết quả), không làm trên bản B. Dựng kg-solo trong thư mục tên đúng `kg-solo`:

```powershell
npx --yes "github:vantt/obsidian-test-fixture#v0.7.0" generate --preset kg-solo --seed 11 --out C:\pkm-test\kg-solo
```

(Tham số này lấy từ `cargo run -q --locked -p pkm-parity -- fixture-args`; chạy lệnh đó để chắc chúng chưa đổi.)

Với mỗi câu trong `khách hàng`, `hợp đồng`, `báo giá`, `quy trình`, `Đà Nẵng`:

1. Phía `pkm` (OHS chạy một lần cho lệnh, chờ tối đa `--ohs-wait` giây). Chạy lại tới khi không còn `ohs_not_ready`:
   ```powershell
   pkm C:\pkm-test\kg-solo related "khách hàng" --packs business --db C:\pkm-test\kg-solo.db --ohs "$env:LOCALAPPDATA\pkm\ohs" --ohs-wait 120
   ```
   Lấy danh sách `seeds[].id` theo `ohs_rank`.
2. Phía Python ở sha ghim, cùng gói OHS, cùng DB OHS trong vault (`.obsidian-hybrid-search.db`, chỉ có sau khi bước 1 đã index xong):
   `pkm` chạy OHS với `OBSIDIAN_IGNORE_PATTERNS` của gói tiếng Việt (theo `24f193a`, sau sha ghim `223c9c3`). Python ở `223c9c3` tự đặt `OHS_LANGUAGE_PACK` nhưng chưa đặt biến này, và để tiến trình con thừa hưởng môi trường; vì vậy đặt biến trước, không thì OHS báo lệch content policy và phép so không hợp lệ:
   ```powershell
   $env:OBSIDIAN_IGNORE_PATTERNS = ".obsidian/**,templates/**,*.canvas,_ohs/**"
   python -c "import sys; sys.path.insert(0, r'C:\src\spec-223c9c3\scripts'); from kg.related import ohs_seeds; print(ohs_seeds(r'C:\pkm-test\kg-solo', 'khách hàng', cli=sys.argv[1], node=sys.argv[2]))" "$env:LOCALAPPDATA\pkm\ohs\app\dist\src\cli.js" "$env:LOCALAPPDATA\pkm\ohs\node\node.exe"
   ```
   Kết quả là `([(đường dẫn, điểm), …], 'ohs-cli')`. Nguồn khác `ohs-cli` (ví dụ `ohs-fts`) nghĩa là Python không chạy được OHS: ghi lại, phép so không hợp lệ.
3. So tập note và thứ tự giữa hai bên. Đạt khi trùng ở mọi câu. Vì `related` có `hits` đã khớp Python từng bit (bộ so khớp kg-solo), trùng hạt giống nghĩa là `related` chỉ có câu hỏi khớp.

Sau mỗi lệnh `pkm … related`, `Get-Process node -ErrorAction SilentlyContinue` không được còn `node` do lệnh đó khởi động.

## 5. Danh sách nghiệm thu

Mỗi dòng ứng với một ô còn mở trong file phase. Ghi `pass`, `fail` hay `skip` (kèm lý do) và đúng bằng chứng nêu ở cột cuối.

| Phase | Ô còn mở | Làm ở | Bằng chứng cần ghi |
|---|---|---|---|
| 02 | Bài thử S1 trên bản B, chỉ in số lệch theo loại | B1 | Bảng số của lệnh `compare`; cột `unsafe-libyaml+layer` |
| 03 | Claude Desktop trên Windows gọi được tool | A6 | Phiên bản Claude Desktop, phiên bản Windows; `kg` chạy trong Developer; câu 2 trả đúng id có dấu. Server `pkm` dùng JSON-RPC tự viết, đúng cách đã chốt ở Q4, nên phép này thay cho chạy lại hai server thử của `probes/s2-mcp`. Chỉ khi `kg` không bắt tay được mới chạy hai server thử theo mục 6 của [báo cáo probe S2](../reports/probe-s2-mcp-261102-1317.md#6-việc-của-user-claude-desktop-trên-windows) để tách lỗi |
| 05 | Python so với chính nó trên bản B: 0 lệch | B2 | Bảng tóm tắt và mã thoát của lệnh `--against python` |
| 05 | `git grep` tên note bản B rỗng | B2 | Số tên đã thử, số tên có kết quả (phải là 0) |
| 08 | `parity --private` bản B: 0 lệch chưa có tên ở bảng vault và schema | B2 | Các dòng của bảng `nodes`, `props`, `links`, `raw_links`, `blocks`, `names`, `schema_*` |
| 09 | Như trên cho mọi bảng và `v3_deviations` | B2 | Các dòng bảng còn lại, `v3_deviations`, mã thoát |
| 10 | Bộ câu tool tự sinh trên bản B: 0 lệch chưa có tên | B2 | Các dòng tầng `tools`, MCP, CLI |
| 11 | `related` chỉ có câu hỏi khớp Python trên máy có gói OHS; OHS chạy trên Windows thật | A5, A6 câu 6, B3 | Thời gian tới `ready`; kết quả `doctor --deep`; dòng `ohs_package` (phiên bản, gói tiếng Việt); danh sách hạt giống hai bên cho từng câu (kg-solo); không còn `node` mồ côi |
| 12 | (Không có việc cho bạn: ô còn mở là quyết định của lead về cờ `--limit`, `--system`) | — | — |
| 13 | `pkm mcp install` trên Windows sạch, Claude Desktop gọi được `kg`, `doctor` không lỗi | A0–A7 | Có Python trên máy không; dòng `installed:` của bộ cài; `--check` báo 11 tool; `doctor --json` (`ok` và `status` theo mã mục); kết quả bốn phép kiểm ở A3; A6; trạng thái `kg` trên bản B ở A7 |

## 6. Rủi ro và giới hạn đã biết

- **SmartScreen, Defender:** `pkm.exe` chưa ký mã. Cài bằng bộ cài thì không bị chặn (bộ cài gỡ dấu tải về). Mở file tải bằng trình duyệt có thể gặp "Windows protected your PC". Ghi lại nếu Defender chặn hay cách ly `pkm.exe` hoặc `node.exe` của gói OHS.
- **Nhận diện Claude Code cài bằng winget chưa kiểm trên máy thật:** `pkm` tách Claude Code khỏi Claude Desktop theo đường dẫn exe; các thư mục `WinGet\Links` và `WinGet\Packages\Anthropic.ClaudeCode_*` suy từ cách winget đặt gói, chưa thử trên Windows. Nếu `pkm` nhận nhầm, ghi đường dẫn exe nó in ra.
- **Gói OHS cho Windows:** `pkm` ghim gói win32-x64 của Release `v0.17.2-vantt.4`; smoke win32-x64 của fork qua ở bản này, nhưng `pkm` chưa chạy gói này trên Windows thật lần nào. OHS báo `ready` ngay khi mở DB, **trước** khi index xong; `pkm` vì vậy đọc `status` và trả `ohs_not_ready` (`starting`, `indexing`) tới khi index xong. Câu tìm ngay sau `ready` có thể rỗng mà không báo lỗi. OHS trả đường dẫn ở dạng NFD; `pkm` chuẩn hoá sang NFC trước khi tra index. Nếu thấy note có dấu bị báo "không có", đó là lỗi cần ghi.
- **Artifact `release-assets` của `workflow_dispatch`** giữ theo hạn mặc định của repo; hết hạn thì chạy lại workflow.
- **Trần `max_len` và hạn giờ:** `paths`, `loops`, `all_loops` từ chối `max_len` lớn hơn 12 và dừng sau 10 giây với lỗi `query too expensive`. Biến `PKM_QUERY_TIMEOUT` (giây, `0` là tắt) đổi hạn giờ. Đây là lệch có chủ ý so với Python, ghi trong `spec/PORTING.md`.
- **`--force` khác `--allow-any-location`:** xem A3. Dùng nhầm `--force` khi Claude Desktop đang chạy thì app ghi đè cấu hình lúc thoát và mục `kg` mất.
- **Bộ so khớp chưa từng chạy trên Windows:** `parity.py` mới chạy trên Linux; CI chỉ chạy phần Rust trên Windows. Lỗi riêng của Windows trong `parity.py` là phát hiện cần báo, không phải lý do bỏ qua bản B.
- **Một mục `kg` duy nhất:** `pkm mcp install` thay mục `kg` đang có, kể cả MCP `kg` Python của user. Bản sao lưu `.bak-*` nằm cạnh file cấu hình.

## 7. Không làm

- Không sửa mã, workflow, `docs/`, `spec/`, file phase hay `plan.md`; lead cập nhật sau khi đọc báo cáo.
- Không gắn tag, không tạo Release, không mở issue.
- Không để nội dung bản B rời máy dưới bất kỳ dạng nào.

## 8. Ghi kết quả

Viết báo cáo ở `plans/reports/windows-tester-<YYMMDD>-<HHMM>-windows-install.md` trong bản clone `C:\src\obsidian-pkm` (giờ Asia/Saigon), bằng tiếng Việt câu đủ ý như `docs/PRD.md`. Nội dung:

1. Máy: phiên bản Windows, CPU, có Python không (lúc phần A), phiên bản Claude Desktop, `gh --version`.
2. Binary: nguồn (Release hay `RUN_ID`), `headSha`, dòng `pkm --version`.
3. Bảng nghiệm thu của mục 5, điền đủ cột.
4. Lỗi và chỗ lệch kỳ vọng, mỗi mục kèm lệnh đã chạy và thông báo (trừ mọi thứ thuộc bản B).
5. Trạng thái cuối của máy: mục `kg` giữ hay trả lại, `pkm` còn cài không.

Commit báo cáo trên nhánh mới `test/windows-install-results` (`git switch -c test/windows-install-results`). Chỉ push khi user cho phép; không thì đưa user đường dẫn file. Trước khi commit, đọc lại báo cáo một lượt để chắc không có tên note, đường dẫn hay nội dung nào của bản B.

## 9. Báo lại

Kết thúc bằng đúng khối sau, mỗi phase một dòng:

```text
Status: DONE | DONE_WITH_CONCERNS | BLOCKED | NEEDS_CONTEXT
Summary: một hai câu
P02: pass|fail|skip - <bằng chứng ngắn>
P03: pass|fail|skip - <bằng chứng ngắn>
P05: pass|fail|skip - <bằng chứng ngắn>
P08: pass|fail|skip - <bằng chứng ngắn>
P09: pass|fail|skip - <bằng chứng ngắn>
P10: pass|fail|skip - <bằng chứng ngắn>
P11: pass|fail|skip - <bằng chứng ngắn>
P13: pass|fail|skip - <bằng chứng ngắn>
Report: plans/reports/windows-tester-<YYMMDD>-<HHMM>-windows-install.md (nhánh, commit sha)
Concerns/Blockers: tuỳ chọn
```

`DONE` khi mọi dòng là `pass`. `DONE_WITH_CONCERNS` khi xong hết nhưng có `fail` hoặc `skip` có lý do. `BLOCKED` khi không lấy được binary, CI `main` đỏ, hoặc user không cho chạy trên bản B.
