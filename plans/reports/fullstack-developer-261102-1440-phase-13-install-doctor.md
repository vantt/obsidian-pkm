# Báo cáo: `pkm mcp install` và `pkm doctor` (phần không phụ thuộc index và MCP server)

- Phase: `plans/261102-0535-rust-core-port-master/phase-13-install-and-doctor.md`. Trạng thái phase: **in-progress**.
- Ngày: 2026-11-02. Nhánh: `worktree-agent-aee0656d37cda0d6d` (chưa push, chưa merge).
- Phần đã làm: thư viện `install` và `doctor` trong `crates/pkm`, có điểm nối cho `cli`. Phần còn lại: `cli` đăng ký hai lệnh con, lệnh `build` cho lần dựng index đầu (phase 09, 12), và user thử trên Windows sạch.

## File

| File | Nội dung |
|---|---|
| `crates/pkm/src/install.rs` | `InstallArgs` (clap), trait `Hooks`, `SystemHooks`, `Install` (thư mục pkm, binary, gói ghim), `run`/`run_with`, `server_entry`, `entry_paths`, `sqlite_header`, `parse_long_paths` |
| `crates/pkm/src/install/config.rs` | Tìm file cấu hình (Windows `%APPDATA%\Claude` và bản Store, macOS, Linux), `ConfigDoc` (đọc, đặt hoặc xoá mục, in lại), `ServerEntry`, diff, ghi nguyên tử kèm sao lưu |
| `crates/pkm/src/install/desktop.rs` | Claude Desktop có đang chạy không (Windows PowerShell rồi lùi về `tasklist`, macOS `pgrep -x Claude`, Linux `pgrep -x claude-desktop`) |
| `crates/pkm/src/install/handshake.rs` | Bắt tay MCP qua stdio: `initialize`, chờ trả lời, `notifications/initialized`, `tools/list`, có hạn giờ, kèm stderr cuối khi lỗi |
| `crates/pkm/src/install/ohs_bundle.rs` | Gói OHS ghim (run `41398112909`), tải bằng `gh`, kiểm sha256 hai lớp, giải nén `.tar.gz` và `.zip`, thay gói cũ an toàn |
| `crates/pkm/src/install/tests.rs` | Test của bộ cài |
| `crates/pkm/src/doctor.rs`, `doctor/tests.rs` | `DoctorArgs`, `Report`, `Check`, `diagnose`, `run` (in chữ hoặc `--json`) |
| `crates/pkm/src/main.rs` | Chỉ thêm `mod doctor; mod install;` |
| `crates/pkm/Cargo.toml`, `Cargo.lock` | Thêm `flate2`, `serde_json` (`preserve_order`), `sha2`, `tar`, `zip` (chỉ deflate); dev: `tempfile` |
| `spec/PORTING.md` | Mục "Bộ cài `pkm mcp install` và `pkm doctor`": chỗ giống và khác `desktop_config.py` |

## Điểm nối cho `cli` (phase 12)

- `pkm mcp install`: `#[command(flatten)]` hoặc lệnh con với `install::InstallArgs`, gọi `install::run(&args, &install::SystemHooks, &mut stdout)`. Kết quả `Err(String)` thì in ra stderr và thoát mã 1.
- `pkm doctor`: lệnh con với `doctor::DoctorArgs`, gọi `doctor::run(&args, &install::SystemHooks, &mut stdout)`. Kết quả `Ok(false)` (có mục `fail`) thì thoát mã 1.
- Khi đã đăng ký, bỏ dòng `#![allow(dead_code)]` ở đầu `install.rs` và `doctor.rs`.
- `Hooks` có mặc định chạy được ngay:
  - `build_index` chạy `<pkm> VAULT build --db DB <cờ đọc>`, nên tự chạy được khi CLI có lệnh `build`.
  - `handshake` dùng được với mọi MCP server stdio.
  - `check_db` kiểm file có header SQLite. Phase 09 có thể ghi đè để mở DB và kiểm phiên bản lược đồ.
  - `desktop_running`, `ohs_state` (chạy `pkm_core::ohs`, chờ có giới hạn), `long_paths_enabled` (`reg query`).
- Mục `kg` có dạng `{"command": "<đường dẫn đủ của pkm>", "args": ["VAULT", "mcp", "--db", "DB", <cờ đọc>, ("--about", TEXT)]}`. Đây là dạng `pkm VAULT mcp` của PRD mục 10. Nếu phase 12 đổi dạng lệnh, chỉ cần sửa `install::server_entry` và `install::entry_paths`.
- Cờ đọc của `InstallArgs` (`--packs`, `--no-aliases`, `--edge-keys`, `--body-edges`, `--adapter`) đang lặp lại tuỳ chọn chung của CLI. Phase 12 có thể thay bằng struct tuỳ chọn chung của mình, miễn là giữ `read_args()`.

## Hành vi

- `--dry-run` không ghi file nào, không dựng index, không tải gì, không tạo thư mục pkm. Lệnh in kế hoạch (file cấu hình, gói OHS, index) và phần khác của mục `kg` (`-` là bản cũ, `+` là bản mới). Dùng được cùng `--remove`.
- `--remove` chỉ xoá `kg`. Nếu không có `kg` thì không ghi gì.
- `--check` đọc mục `kg` đã cấu hình, bắt tay và in `command: tên phiên bản, N tool: …`. Lệnh không đổi gì.
- Thứ tự khi cài: đọc và kiểm JSON (lỗi thì dừng, không bao giờ ghi đè file hỏng), từ chối nếu Claude Desktop đang chạy (`--force` để bỏ qua), cài gói OHS, dựng index (lỗi thì cấu hình giữ nguyên), ghi nguyên tử kèm `<file>.bak-<UTC>`, rồi bắt tay với server vừa ghi.
- Mục `kg` không đổi thì không ghi lại. Các mục khác, thứ tự khoá, CRLF và ký tự tiếng Việt giữ nguyên. BOM bị bỏ, như Python.
- Không có file cấu hình nhưng có đúng một thư mục Claude thì tạo file mới. Đây là chỗ khác Python, đã ghi trong PORTING.
- `pkm doctor` có 6 mục: `claude_config`, `kg_entry` (command là đường dẫn đủ và có thật, vault có thật, có `mcp`), `db`, `ohs_package`, `ohs_state` (`ready` là ok, `starting`/`indexing` là warn, `model_missing`/`broken` là fail), `long_paths` (chỉ Windows, tắt thì warn). Mỗi mục warn hoặc fail đều có "việc cần làm". `--json` in `{"healthy", "checks": [{"id", "status", "detail", "action"}]}`.

## Gói OHS

- Ghim cả bốn artifact của run `41398112909`: id và digest sha256 của zip artifact lấy từ API GitHub. Sha256 của gói bên trong được ghim cho `linux-x64` (`9160d45e…`) và `win32-x64` (`02c277df…`). Tôi đã tải hai artifact này và kiểm digest khớp (`3df23d8b…`, `f7c2ed05…`). Gói Windows là `.zip`, gốc là `ohs-0.17.2-vantt.1-win32-x64/`, có `node/node.exe` và `app/dist/src/cli.js`.
- Kiểm hai lớp: digest của zip artifact phải khớp bản ghim, rồi sha256 của gói phải khớp file `.sha256` nằm trong artifact. Gói trần (không bọc artifact) được nhận khi biết sha256 ghim của nó. Thư mục đã giải nén được chép vào mà không kiểm sha256, và lệnh in rõ điều này. File của nền tảng khác thì báo nhầm nền tảng.
- Cài vào thư mục tạm cạnh `<thư mục pkm>/ohs`, kiểm bằng `OhsPackage::from_dir`, rồi đổi tên gói cũ sang một bên và đặt gói mới vào. Lỗi giữa chừng thì gói cũ được trả về chỗ. Nếu OHS đang giữ file trên Windows, lệnh báo cần thoát Claude Desktop.
- Khi không có `--ohs-package` và chưa có gói, bộ cài tải bằng `gh`. Tải lỗi chỉ là cảnh báo, vì `kg` vẫn chạy được và tool cần OHS sẽ báo `ohs_not_ready`. Có `--ohs-package` mà lỗi thì dừng.

### Gap: tải artifact cần đăng nhập GitHub

GitHub bắt đăng nhập khi tải artifact của Actions, kể cả với repo công khai. Người dùng cuối không có `gh` đã đăng nhập thì không tự tải được. Lệnh khi đó báo việc cần làm: đăng nhập GitHub, tải `bundle-<nền tảng>` ở trang run, rồi chạy lại với `--ohs-package <file zip>`. Muốn hết gap thì fork phải đưa gói lên GitHub Release, khi đó bộ cài tải qua URL công khai. Việc này đã ghi trong DECISIONS: chuyển sang Release khi có. Thêm nữa, artifact của run này **hết hạn ngày 2027-01-31**.

## Kiểm

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` và `cargo test --workspace` đều qua. Riêng `pkm`: 56 test qua, 2 test `#[ignore]`.
- `cargo clippy -p pkm --all-targets -- -D warnings` cũng qua với target `x86_64-pc-windows-msvc` và `aarch64-apple-darwin` (chỉ biên dịch, chưa chạy test trên hai nền này).
- Test bắt tay chạy một MCP server giả bằng cách gọi lại chính binary test. Cách này không cần script shell nên chạy được trên Windows. Các trường hợp đã thử: thành công, server im lặng (quá hạn giờ), server trả lỗi kèm stderr, server thoát sớm, không có lệnh.
- Test của bộ cài dùng `Hooks` giả, nên tiến trình kiểm Claude Desktop thay được. Đã thử: file tạm giữ nguyên mục khác (gồm `ohs`) và thứ tự khoá; `--dry-run` không ghi; `--remove` chỉ xoá `kg`; từ chối khi Desktop đang chạy và khi không kiểm được; `--force`; dựng index lỗi; JSON hỏng; giữ CRLF và BOM.
- `installs_real_pinned_artifact` (`#[ignore]`, `PKM_TEST_OHS_ARTIFACT=<bundle-linux-x64.zip>`) qua: cài artifact thật và `node --version` của gói chạy được.
- `downloads_and_installs_pinned_artifact_with_gh` (`#[ignore]`) qua: tải artifact thật bằng `gh` đã đăng nhập (khoảng 250 giây, 99 MB), digest khớp, `.sha256` khớp, cài xong.
- Không đụng tới cấu hình Claude Desktop thật trên máy này: mọi test truyền `--config` tới file tạm.

## Câu hỏi còn mở

1. Phase 12 có giữ dạng `pkm VAULT mcp --db DB …` cho mục `kg` không? Nếu đổi, cần sửa `server_entry` và `entry_paths`.
2. `pkm doctor` mặc định chạy thử OHS trên vault của `kg` (tối đa 15 giây, `--no-ohs-start` để bỏ). OHS không có `--db` nên dùng DB mặc định trong vault, chung với MCP `ohs` của người dùng. Cách này có chấp nhận được không, hay để mặc định không chạy thử?
3. Fork khi nào đưa gói lên Release? Artifact hết hạn ngày 2027-01-31.
