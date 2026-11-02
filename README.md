# obsidian-pkm

Hỏi Claude Desktop về graph của vault Obsidian: note nào nối với note nào, đi từ ý này tới ý kia qua đâu, note nào lạc khỏi mạng. Một file `pkm`, không cần Python.

Trạng thái (2026-11-02): **chưa phát hành bản nào.** Mã đã xong tới bộ cài: lõi, CLI, MCP `pkm` (một cửa: tìm kiếm qua OHS và graph; trước tên là `kg`), `related` với OHS, `pkm mcp install` và `pkm doctor`, khớp bản Python trên vault thử kg-solo. Đang chờ thử trên Windows thật và trên vault thật. Bản thử đầu tiên là pre-release `v0.1.0-rc.1`; các bước dưới đây dùng bản đó. GitHub không coi pre-release là bản mới nhất, nên lệnh `gh release download` phải ghi tag (bỏ tag khi đã có bản chính thức).

## Bắt đầu nhanh (Windows)

Từ máy Windows trống tới Claude Desktop hỏi được graph vault trong khoảng 5 phút. Cần: Claude Desktop, một vault Obsidian, và GitHub CLI `gh` đã đăng nhập (repo `pkm` còn private nên tải `pkm` phải qua `gh`). Gói OHS (tìm kiếm theo nghĩa cho `search` và `related`) thì `pkm mcp install` tự tải từ bản phát hành công khai của fork, không cần `gh`.

1. Cài `gh` rồi đăng nhập (bỏ qua nếu đã có). Mở PowerShell mới sau khi cài:
   ```powershell
   winget install --id GitHub.cli
   gh auth login
   ```
2. Cài `pkm` vào `%LOCALAPPDATA%\pkm\bin` và thêm vào PATH (không cần quyền admin):
   ```powershell
   gh release download v0.1.0-rc.1 --repo vantt/obsidian-pkm --pattern install.ps1 --output - | Out-String | iex
   ```
3. Thoát hẳn Claude Desktop (cả biểu tượng ở khay hệ thống), rồi nối vault vào Claude Desktop:
   ```powershell
   pkm mcp install "D:\Duong\dan\toi\Vault"
   ```
   Lệnh này dựng index, tải gói OHS `0.17.2-vantt.4` (khoảng 100 MB, kiểm sha256) vào `%LOCALAPPDATA%\pkm\ohs`, ghi mục `pkm`, rồi làm ấm index tìm kiếm (vault lớn lần đầu mất vài phút; `--no-warm` để bỏ). Mục `kg` cũ do `pkm` ghi được đổi tên thành `pkm`; mục `ohs` bạn tự thêm thì giữ nguyên (không cần nữa, vì `pkm` đã có `search` và `read`). Máy không ra được GitHub thì tải file gói ở [bản phát hành của fork](https://github.com/vantt/obsidian-hybrid-search/releases/tag/v0.17.2-vantt.4) rồi thêm `--ohs-package <file vừa tải>`; không cần OHS thì thêm `--no-ohs`.
4. Mở Claude Desktop và hỏi: *"Dùng pkm: vault có bao nhiêu note, note nào được liên kết tới nhiều nhất?"*

Có lỗi thì chạy `pkm doctor`: lệnh này kiểm index, cấu hình Claude Desktop, gói OHS, đường dẫn dài, và in cách sửa.

macOS, Linux: bước 2 là `gh release download v0.1.0-rc.1 --repo vantt/obsidian-pkm --pattern install.sh --output - | sh` (cài vào `~/.local/bin`).

<details>
<summary>Cập nhật, gỡ cài, tuỳ chọn của bộ cài</summary>

- **Cập nhật:** chạy lại lệnh ở bước 2. Cùng phiên bản thì không làm gì. Claude Desktop đang chạy `pkm.exe` vẫn cập nhật được; khởi động lại Claude Desktop để dùng bản mới.
- **Gỡ cài:** gỡ mục `pkm` khỏi Claude Desktop trước, rồi gỡ `pkm`:
  ```powershell
  pkm mcp install --remove
  $env:PKM_UNINSTALL = '1'; gh release download v0.1.0-rc.1 --repo vantt/obsidian-pkm --pattern install.ps1 --output - | Out-String | iex
  ```
  Index và gói OHS vẫn nằm ở `%LOCALAPPDATA%\pkm`; xoá thư mục đó nếu muốn bỏ hẳn.
- **Cài và nối vault một lần:** `$env:PKM_MCP_INSTALL = 'D:\Vault'` trước lệnh ở bước 2.
- **Tuỳ chọn khác** (biến môi trường, hoặc tham số khi chạy file): `PKM_VERSION` (`-Version`), `PKM_INSTALL_DIR` (`-InstallDir`), `PKM_NO_MODIFY_PATH=1`, `PKM_DRY_RUN=1`, `PKM_RELEASE_REPO` (`-Repo`), `PKM_RELEASE_BASE_URL` (`-ReleaseBaseUrl`). Đầy đủ ở đầu [scripts/install.ps1](scripts/install.ps1) và `sh install.sh --help`.
- Bộ cài thử tải công khai trước; bị từ chối (repo private) thì chuyển sang `gh release download`. Mọi file đều được kiểm sha256 theo `checksums.txt` của bản phát hành.
</details>

<details>
<summary>Windows chặn pkm.exe (SmartScreen, Defender)</summary>

`pkm.exe` chưa ký mã. Cài bằng bộ cài ở trên thì không bị chặn. Nếu tự tải file bằng trình duyệt và Windows báo "Windows protected your PC", bấm **More info → Run anyway**, hoặc chạy `Unblock-File .\pkm.exe` sau khi đã so sha256 với `checksums.txt` (`Get-FileHash .\pkm.exe`).
</details>

## Cho người phát triển

Lõi Rust của sản phẩm PKM trên Obsidian: index graph của vault (SQLite), bộ tool hỏi graph và MCP server, thay cho bản Python trong [`vantt/obsidian-pkm-automation`](https://github.com/vantt/obsidian-pkm-automation).

Bản Python là đặc tả chạy được, ghim ở `spec/spec.lock`. Repo này chuyển phần lõi (`scripts/kg`, MCP `kg`, adapter bản B), so khớp kết quả với bản Python, đuổi theo thay đổi mới của Python cho tới khi khớp hẳn, rồi thay bản Python.

Đọc trước: [docs/PRD.md](docs/PRD.md). Quyết định kỹ thuật: [docs/DECISIONS.md](docs/DECISIONS.md). Phát hành: gắn tag `vX.Y.Z` trùng `version` trong `Cargo.toml`; `.github/workflows/release.yml` chạy lại CI, dựng bốn binary, gộp `checksums.txt`, đóng dấu bộ cài và đăng Release.
