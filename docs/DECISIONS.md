# Quyết định kỹ thuật

Nhật ký các lựa chọn PRD để agent tự chốt khi làm (crate, công cụ, cách làm). Quyết định đã chốt từ trước (D1–D10) và câu hỏi còn mở cho user nằm ở [PRD.md](PRD.md) mục 1 và 15; ở đây chỉ ghi phần chốt trong lúc làm.

Mỗi quyết định một mục, theo khung:

```markdown
## <tiêu đề>
- **Ngày:** YYYY-MM-DD
- **Lựa chọn:** chọn gì
- **Lý do:** vì sao, đã cân nhắc gì
- **Bằng chứng:** số đo, bài thử, file, commit
```

Đổi một quyết định thì thêm mục mới trỏ về mục cũ, không sửa đè.

## Q3: Crate YAML
- **Ngày:** 2026-11-02
- **Lựa chọn:** `unsafe-libyaml` 0.2.11, chỉ dùng luồng sự kiện của parser, cộng một lớp tương thích PyYAML tự viết trong `pkm-core::vault`: composer như `CParser`, resolver YAML 1.1 của PyYAML (tắt timestamp cho frontmatter như `vault.LOADER`, bật cho `schema.yaml`), constructor như `SafeConstructor`, và `str()`/`repr()` của Python để ra `props`. Phần `unsafe` nằm gọn trong một module. Không dùng thẳng crate YAML nào ở mức giá trị.
- **Lý do:** `vault.py` đọc bằng `CSafeLoader`, tức libyaml 0.2.5, và lưu dòng đầu thông báo lỗi YAML vào `nodes.error`; muốn khớp cả giá trị lẫn câu lỗi thì parser phải là libyaml. Crate dùng thẳng (`serde_norway`, `saphyr`) theo YAML 1.2 nên lệch ở bool `yes/no/on/off`, số `010`, `1_000`, `10:30`, `0912…`, `1e3`, khoá trùng, `<<`, tag tường minh và câu lỗi. `saphyr-parser` kèm cùng lớp vẫn lệch vì parser khác libyaml. `libyaml-safer` 0.3.0 (bản an toàn của libyaml) panic khi block scalar là giá trị cuối của frontmatter (issue #6, #7 ở upstream, còn mở). `unsafe-libyaml` đã đóng băng (archived 2024-03) nhưng libyaml 0.2.5 cũng đóng băng và là đúng bản PyYAML dùng. Khi `libyaml-safer` sửa hai issue thì chạy lại bài thử; 0 lệch thì đổi sang để bỏ `unsafe`.
- **Bằng chứng:** bài thử S1 (PRD mục 14 R01), `probes/s1-yaml/`, báo cáo `plans/reports/probe-s1-yaml-261102-1317.md`. Số note mà index thấy khác Python:

  | Ứng viên | kg-solo (746) | 58 ca biên | 25.000 note ngẫu nhiên | Bản B |
  |---|---|---|---|---|
  | `unsafe-libyaml` + lớp | 0 | 0 | 0 | chờ user |
  | `libyaml-safer` + lớp | 0 | 1 | 242 (đều là panic) | |
  | `saphyr-parser` + lớp | 0 | 9 | 6.510 | |
  | `serde_norway` dùng thẳng | 0 | 35 | 20.916 | |
  | `saphyr` dùng thẳng | 0 | 36 | 21.317 | |

  kg-solo không có dạng YAML rủi ro nào (chỉ chuỗi, số nguyên thường, danh sách; ngày được cả hai bên giữ là chuỗi), nên mọi ứng viên đều 0 lệch ở đó; bảng lệch theo loại và cách xử lý ở mục 3 của báo cáo. Bốn dạng làm bản Python dừng hẳn (`!!bool` chữ lạ, `!!timestamp` sai dạng, `!!int ""`, alias tự lồng) để phase 08 chốt cách xử lý.

## Q4: `rmcp` hay JSON-RPC tự viết
- **Ngày:** 2026-11-02
- **Lựa chọn:** JSON-RPC 2.0 stdio tự viết (mỗi dòng một bản tin), đồng bộ, chỉ dùng `serde_json` (bật `preserve_order`). Không dùng `rmcp`, không dùng async runtime.
- **Lý do:**
  - Kích thước và phụ thuộc: cùng một tool, bản tự viết 0,68 MB, 12 crate, build release từ sạch 2,5 s; bản `rmcp` 3.8.0 2,79 MB, 56 crate, 16,5 s, và bắt buộc `tokio` (cần cả `time`, thiếu là panic lúc chạy). PRD mục 3 nói không dùng async runtime nếu không cần.
  - Câu chữ: cả hai đều cho kiểm soát đủ `instructions`, mô tả tool, schema tham số (mục `find` giống hệt Python, kể cả thứ tự khoá), và phiên bản giao thức (`rmcp` phải ghi đè `supported_protocol_versions`).
  - Hợp đồng MCP phải giống Python (PRD mục 8), mà ở tầng giao thức `rmcp` khác Python ở năm chỗ: câu báo phương thức lạ (`nope/x` thay vì `method not found: nope/x`), dòng không phải JSON bị bỏ im lặng (Python trả `-32700`), batch bị trả `-32600` không có `id` (Python trả lời từng phần), request trước `initialize` bị từ chối, `server/discover` trả `-32022` (Python `-32601`). Bản tự viết khớp Python ở cả năm chỗ với khoảng 100 dòng, chép thẳng `Server.handle` của Python.
  - `rmcp` đổi nhanh (bản 3.x, đã theo giao thức 2026-07-28); bám nó là thêm một nguồn thay đổi ngoài Python cần đuổi theo.
  - Xem lại khi Python chuyển sang giao thức 2026-07-28 (bỏ `initialize`, thêm `server/discover`) hoặc khi cần MCP qua HTTP.
- **Bằng chứng:** `probes/s2-mcp/` (hai binary `s2-hand`, `s2-rmcp`, test `tests/stdio_session.rs` so mục `find` với bản Python); bản ghi phiên Claude Code 2.1.316 và số đo ở `plans/reports/probe-s2-mcp-261102-1317.md`.

## Ghim toolchain Rust
- **Ngày:** 2026-11-02
- **Lựa chọn:** `rust-toolchain.toml` ghim `1.97.1` (kèm `rustfmt`, `clippy`); edition 2024. CI cài đúng bản đó bằng `rustup toolchain install`, không dùng action cài toolchain riêng.
- **Lý do:** CI chạy `clippy -D warnings`. Theo `stable` thì mỗi bản Rust mới có thể thêm lint và làm CI đỏ dù mã không đổi. Nâng toolchain là một commit riêng, có chủ ý.
- **Bằng chứng:** `rust-toolchain.toml`, `.github/workflows/ci.yml`.

## Thư viện CLI: `clap`
- **Ngày:** 2026-11-02
- **Lựa chọn:** `clap` 4, feature `derive`.
- **Lý do:** PRD mục 3 và 10 đã gợi ý. CLI có nhiều lệnh con và tuỳ chọn chung (giống `scripts/kg/cli.py` dùng `argparse`), viết tay không đáng.
- **Bằng chứng:** `crates/pkm/Cargo.toml`.

## `pkm --version` lấy sha đặc tả lúc build
- **Ngày:** 2026-11-02
- **Lựa chọn:** `crates/pkm/build.rs` đọc `[spec].sha` trong `spec/spec.lock` và đưa vào binary. Kết quả in ra có dạng `pkm 0.1.0 (spec <sha đủ 40 ký tự>)`. Build dừng nếu thiếu sha hoặc sha không phải 40 ký tự hex. Parser chỉ đọc dạng `key = "value"`, không thêm crate TOML vào build-dependency.
- **Lý do:** `spec.lock` là nguồn duy nhất của sha đang ghim (PRD mục 4), nên nâng ghim thì `--version` tự đổi theo. In sha đủ 40 ký tự để không nhầm khi so.
- **Bằng chứng:** `crates/pkm/tests/version.rs` so kết quả `pkm --version` với sha đọc từ `spec.lock`.

## Seed của fixture kg-solo: 11
- **Ngày:** 2026-11-02
- **Lựa chọn:** `spec.lock` ghi `[fixture].seed = 11`.
- **Lý do:** Test của repo đặc tả sinh vault mà không truyền `--seed`, nên dùng giá trị mặc định của preset. Cỡ vault cũng là mặc định (`--clients 30 --days 365`, 746 note).
- **Bằng chứng:** repo đặc tả ở `5a3eb2f`: `scripts/kg/tests/test_kg.py` (lớp `Fixture`, `setUpClass`) chạy `otf.mjs generate --preset kg-solo --out …`, không có `--seed`. Fixture `v0.7.0` (`2a514a8`): `presets/kg-solo.mjs` dòng 14 `seed: { default: 11 }` và dòng 97 `build({ seed = 11, … })`. `docs/knowledge-graph/bench/r2/README.md` mục 1 của repo đặc tả cũng ghi "hạt giống 11".

## Lệnh `follow`: `parity/follow.py`
- **Ngày:** 2026-11-02
- **Lựa chọn:** `python3 parity/follow.py` (Python chuẩn, không phụ thuộc ngoài), không dùng `cargo xtask`. Bản clone repo đặc tả nằm ngoài repo này: `$PKM_FOLLOW_CLONE`, `--clone-dir`, hoặc `~/.cache/pkm-follow/spec-repo`. Quyền đọc lấy từ `gh auth git-credential` chỉ cho lệnh git đó, không đổi cấu hình máy.
- **Lý do:** Bộ so khớp cũng cần Python để chạy bản đặc tả, nên không thêm crate hay thành viên workspace chỉ để chạy một lệnh làm tay. Không đụng `Cargo.lock`, không làm CI build thêm.
- **Bằng chứng:** `parity/follow.py`; chạy hai lần liên tiếp, tập sha khớp `git log <sha ghim>..origin/main -- <đường dẫn theo dõi>`.

## Module `platform`: `caseless`, `std::env::home_dir`
- **Ngày:** 2026-11-02
- **Lựa chọn:** so tên không phân biệt hoa thường bằng `caseless` (khớp `str.casefold` của Python), chuẩn hoá NFC/NFD bằng `unicode-normalization`; thư mục nhà lấy từ `std::env::home_dir` thay vì crate `dirs`; Windows dùng `MoveFileExW` không cờ thay thế (`windows-sys`).
- **Lý do:** `to_lowercase` không giống `casefold` với một số ký tự; `home_dir` đã đúng trên ba nền từ Rust 1.86 nên không cần thêm crate; `std::fs::rename` trên Windows đè đích, khác `os.rename`.
- **Bằng chứng:** `crates/pkm-core/src/platform.rs`, bảng test platform trong `spec/PORTING.md`.

## Thứ tự liệt kê file khi so khớp: sắp theo đường dẫn, so chặt
- **Ngày:** 2026-11-02
- **Lựa chọn:** phía Python của bộ so khớp liệt kê note theo đường dẫn đã sắp (thứ tự NTFS và APFS trả về). `edges.id` và `related` vẫn so chặt như PRD (điểm lệch tối đa 1e-6). Lõi Rust liệt kê note theo đường dẫn đã sắp, chèn cạnh và chạy PageRank đúng thứ tự Python.
- **Lý do:** đầu ra Python phụ thuộc thứ tự hệ điều hành liệt kê file: đảo thứ tự trên kg-solo đổi 593/726 `edges.id` và đổi điểm `related` cỡ 1e-5. Ghim thứ tự giúp golden dựng trên máy này khớp trên máy khác mà không phải nới luật so. Nếu phase index hoặc related không giữ được thứ tự này thì phần đó ghi ngoại lệ có tên, không nới luật chung.
- **Bằng chứng:** báo cáo `plans/reports/fullstack-developer-261102-1317-phase-05-parity.md`, `parity/dump-python.py`.

## Gói OHS: ghim artifact của một run CI của fork
- **Ngày:** 2026-11-02
- **Lựa chọn:** khi GitHub Release `v0.17.2-vantt.1` của fork chưa có gói, phase `ohs` và bộ cài lấy gói từ artifact của run `41398112909` (repo `vantt/obsidian-hybrid-search`), kiểm sha256. Fork có gói trên Release thì chuyển sang Release.
- **Lý do:** user chọn (2026-11-02). Smoke trên darwin-arm64 và win32-x64 của fork đang hỏng nên Release chưa có file.
- **Bằng chứng:** `plans/reports/probe-s3-ohs-261102-1317.md`.
- **Thay bởi:** mục "Gói OHS: ghim GitHub Release `v0.17.2-vantt.3` của fork" bên dưới.

## Bốn dạng YAML làm Python crash: Rust ghi lỗi cho note
- **Ngày:** 2026-11-02
- **Lựa chọn:** với `!!bool maybe`, `!!timestamp soon`, `!!int ""` và alias vòng trong list, Rust không dừng build mà ghi lỗi YAML cho note đó; ghi "Lệch có chủ ý" trong `spec/PORTING.md` và ngoại lệ có tên trong `parity/exceptions.toml`. Chưa mở issue ở repo đặc tả.
- **Lý do:** user chọn (2026-11-02).
- **Bằng chứng:** `plans/reports/probe-s1-yaml-261102-1317.md`.

## Binary Windows dùng CRT tĩnh
- **Ngày:** 2026-11-02
- **Lựa chọn:** `.cargo/config.toml` bật `target-feature=+crt-static` cho `x86_64-pc-windows-msvc`. Mỗi lần đóng gói, `scripts/release-assets.sh` chạy `dumpbin -dependents` và dừng nếu binary còn nhập `vcruntime*`, `msvcp*`, `ucrtbase*` hay `api-ms-win-crt-*`. CI và release không đặt `RUSTFLAGS`, vì biến này ghi đè cờ trong file cấu hình.
- **Lý do:** mặc định target MSVC liên kết động với `VCRUNTIME140.dll`, file không có sẵn trên Windows mới cài hay Windows Sandbox. Claude Desktop khởi chạy `pkm.exe` sẽ lỗi mà không báo gì. SQLite (bundled, qua `cc`) đọc cùng cờ và biên dịch bằng `/MT`. Binary lớn hơn một chút, đổi lại không phải cài thêm gì.
- **Bằng chứng:** audit `plans/reports/packaging-audit-261102-1505-cs-packaging-distribution.md` (B1); bước "Đóng gói và chạy thử" của `.github/workflows/release-assets.yml` trên `windows-2025`.

## Repo private: cài qua gh, nguồn phát hành công khai tính sau
- **Ngày:** 2026-11-02
- **Lựa chọn:** `vantt/obsidian-pkm` giữ private. Bộ cài thử tải công khai (`<base>/download/<tag>/<asset>`) trước; gặp HTTP 401/403/404 thì chuyển sang `gh release download`, và báo rõ phải cài `gh` và `gh auth login` nếu thiếu. Nguồn đổi được không cần sửa script: `PKM_RELEASE_REPO` / `--repo` / `-Repo`, `PKM_RELEASE_BASE_URL` / `--release-base-url` / `-ReleaseBaseUrl`. README hướng dẫn tải bộ cài bằng `gh release download … --output - | Out-String | iex` (Windows) và `… | sh` (macOS, Linux).
- **Lý do:** user chọn (2026-11-02). Phát hành công khai (có thể là một repo công khai chỉ chứa release) quyết định sau. Khi đó chỉ cần đổi repo mặc định hoặc đặt biến môi trường, và lệnh `irm …/releases/latest/download/install.ps1 | iex` chạy được không cần `gh`.
- **Bằng chứng:** `gh api repos/vantt/obsidian-pkm` trả `private=true`. Chạy thử `install.sh` với `--version v0.1.0`: tải công khai bị 404 rồi chuyển sang `gh`; khi bỏ `gh` khỏi PATH thì dừng với câu hướng dẫn cài và đăng nhập.

## Bộ cài: `scripts/install.sh`, `scripts/install.ps1`
- **Ngày:** 2026-11-02
- **Lựa chọn:**
  - Asset là binary trần `pkm-<target>[.exe]` (giữ cách đặt tên cũ), cộng một `checksums.txt` gộp; bỏ các file `.sha256` riêng. Tên asset chỉ định nghĩa ở `scripts/release-assets.sh asset_name`.
  - Cài vào `%LOCALAPPDATA%\pkm\bin` (cùng gốc với `platform::pkm_dir()`) và `~/.local/bin`. Không cần quyền admin, từ chối chạy với quyền admin/root trừ khi `PKM_ALLOW_ROOT=1`.
  - PATH theo chính sách B của skill `cs-packaging-distribution`: Windows sửa `HKCU\Environment\Path` (giữ kiểu `REG_EXPAND_SZ`, phát `WM_SETTINGCHANGE`); macOS/Linux ghi khối `# >>> pkm >>>` vào `~/.profile`, `~/.bashrc`, `~/.zshrc` và `conf.d` của fish. Gỡ cài xoá đúng phần đã thêm. `--no-modify-path` chỉ in hướng dẫn.
  - Kiểm sha256 theo đúng một dòng có tên file trùng hẳn, chạy thử `--version` trước khi thay, đổi tên rồi mới thay. Trên Windows, `pkm.exe` đang chạy (Claude Desktop) được đổi tên thành `pkm.exe.old-*` và xoá ở lần chạy sau. Marker `.pkm-managed` ghi phiên bản: cùng phiên bản thì không làm gì, không đè binary không do bộ cài đặt.
  - `--uninstall` hỏi qua terminal, không có terminal thì cần `--yes`; giữ index và dữ liệu, in đường dẫn và lệnh gỡ mục `kg`.
  - `--mcp-install <vault>` chạy `pkm mcp install <vault>` sau khi cài, chỉ khi người dùng truyền cờ.
  - Bộ cài chỉ dùng ký tự ASCII, câu báo bằng tiếng Anh. `release-assets.sh stamp` từ chối file có ký tự ngoài ASCII.
  - Phiên bản đóng dấu vào bộ cài lúc phát hành (`@PKM_VERSION@`); bản trong repo đòi `--version`.
- **Lý do:** theo `INS-*`, `SEC-01/04` của skill, phỏng theo `templates/install.sh` của skill, `forgent/scripts/install.ps1` và phần PATH của `skillstation/scripts/install.ps1`. Binary trần đơn giản hơn tar/zip trên Windows (không cần `tar.exe`). ASCII vì Windows PowerShell 5.1 đọc kết quả `irm` (GitHub trả `application/octet-stream`) và file `.ps1` không BOM theo bảng mã cũ, chữ có dấu sẽ vỡ.
- **Bằng chứng:** `scripts/test-installer.sh` (chạy được trên máy, đã qua trên Linux), `scripts/test-installer.ps1` (Windows PowerShell 5.1 trên CI), shellcheck 0.11.0 sạch.

## Pipeline phát hành
- **Ngày:** 2026-11-02
- **Lựa chọn:**
  - `.github/workflows/release-assets.yml` (`workflow_call`): dựng bốn đích ở chế độ release, đóng gói bằng `scripts/release-assets.sh`, chạy thử `pkm --version` trên runner chạy được đích đó (x86_64-apple-darwin chạy qua Rosetta nếu có, không thì cảnh báo), kiểm CRT tĩnh, thử vòng đời bộ cài, lưu artifact. `ci.yml` gọi nó ở mọi PR.
  - `release.yml`: `verify` (tag phải bằng `v` + phiên bản crate) → gọi lại cả `ci.yml` → `package` (đóng dấu bộ cài, gộp `checksums.txt`, kiểm đủ 6 file) → `publish` chỉ khi có tag. Tag có dấu `-` thành pre-release. `workflow_dispatch` dừng ở artifact `release-assets`.
  - Action ghim theo SHA của bản mới nhất (đã kiểm bằng `gh api`): `actions/checkout` v7.0.1, `upload-artifact` v7.0.2, `download-artifact` v8.0.2, `Swatinem/rust-cache` v2.9.3. Runner ghim: `ubuntu-24.04` (trần glibc 2.39 cho binary Linux), `macos-15`, `windows-2025`. Không động vào các bước có sẵn của `ci.yml`; chỉ thêm `workflow_call`, đổi nhóm `concurrency` thành `${{ github.workflow }}-${{ github.ref }}` (lần gọi từ release không huỷ CI của main), và thêm job `assets`.
  - Chưa làm: release nháp rồi chạy thử bộ cài trên release thật trước khi công khai (REL-07), ký mã và provenance (SEC-02, PRD mục 12).
- **Lý do:** REL-03/04/05/06/10, VER-02/07, TST-02 của skill. Gọi lại `ci.yml` thay vì chép bước để release luôn chạy đúng bộ kiểm của CI (gồm so khớp).
- **Bằng chứng:** audit (I6, I7); `gh api repos/<action>/commits/<tag>`.

## `pkm doctor`: chỉ đọc, OHS chỉ là cảnh báo, envelope `--json` cố định
- **Ngày:** 2026-11-02
- **Lựa chọn:** `doctor` mặc định không ghi gì và không chạy OHS: chỉ kiểm gói có đủ file và `node --version`. Chạy thử OHS cần `--deep` (bỏ `--no-ohs-start`). Mục OHS không ổn (thiếu gói, Node hỏng, mọi trạng thái của `--deep`) chỉ là `warn`. Mục `kg` cài với `--no-ohs` (`env.PKM_NO_OHS = "1"`) thì thiếu gói là `skip`. `--json` in `{"schema": 1, "ok", "data": {"checks": [{"id", "status", "detail", "action", "fix"}]}, "error", "next"}`. `fix` là lệnh chạy được, không hỏi gì, cho PowerShell trên Windows và `sh` nơi khác; `next` là `fix` của các mục `fail`. Hợp đồng chi tiết nằm ở `spec/PORTING.md`, mục bộ cài.
- **Lý do:** chẩn đoán không được ghi. Chạy OHS có thể bắt đầu index và nạp model trên DB dùng chung với MCP `ohs`. User chốt OHS là tuỳ chọn lúc này (tích hợp OHS do việc khác lo): không có OHS thì chỉ `related` khi không có `hits` bị ảnh hưởng, nên thiếu OHS không được làm `doctor` báo hỏng. Chốt khoá JSON ngay vì chưa ai gọi `doctor --json`, theo chuẩn envelope của skill `cs-packaging-distribution` (CMD-11, DOC-01/03). Chưa có `--fix`, vì `fix` đã là lệnh chạy được.
- **Bằng chứng:** `crates/pkm/src/doctor.rs`, `crates/pkm/src/doctor/tests.rs`, `plans/reports/fullstack-developer-261102-1520-doctor-install-fixes.md`.

## `pkm mcp install`: binary phải ở chỗ cài ổn định, so rồi mới ghi, nguồn OHS đổi được bằng biến
- **Ngày:** 2026-11-02
- **Lựa chọn:**
  - Binary ở chỗ cài ổn định là binary nằm ở một trong ba chỗ: thư mục chuẩn (hằng `STANDARD_BIN_DIR`: `%LOCALAPPDATA%\pkm\bin` trên Windows, `~/.local/bin` nơi khác, cùng mặc định với `install.ps1`/`install.sh`), `PKM_INSTALL_DIR`, hoặc thư mục có `.pkm-managed` của bộ cài. Binary ở chỗ khác thì `mcp install` từ chối trừ khi có `--force`. Lời từ chối nói rõ khi file nằm trong Downloads hay thư mục tạm, và in lệnh bộ cài (`gh release download --repo vantt/obsidian-pkm --pattern install.ps1|install.sh`, vì repo riêng tư) cùng lệnh chạy lại từ chỗ chuẩn.
  - Trước khi ghi cấu hình: hỏi lại Claude Desktop, đọc lại file và so sha256 với bản đã đọc. Khác thì đặt lại mục `kg` trên bản mới.
  - `PKM_OHS_PACKAGE` thay nguồn tự tải gói OHS. Giá trị là đường dẫn, hoặc URL https của file gói kèm `<URL>.sha256`.
- **Lý do:**
  - Claude Desktop gọi đúng đường dẫn ghi trong cấu hình, nên binary trong Downloads bị xoá là `kg` hỏng.
  - Giữ bề mặt lệnh của PRD, nên dùng lại `--force` thay vì thêm cờ.
  - Dựng index có thể mất vài phút; trong lúc đó, nếu Claude Desktop mở hoặc app khác sửa file mà ta vẫn ghi thì thay đổi đó mất.
  - Artifact ghim hết hạn ngày 2027-01-31, và máy khách không có `gh`. Đổi nguồn bằng biến môi trường thì không cần ra bản `pkm` mới. Nguồn URL kiểm bằng `.sha256` đi kèm, như asset của Release.
- **Bằng chứng:** `crates/pkm/src/install.rs`, `crates/pkm/src/install/{ohs_bundle,shell,tests}.rs`.

## `mcp install`: tách `--allow-any-location` khỏi `--force`; trần và hạn giờ cho liệt kê đường, vòng
- **Ngày:** 2026-11-02
- **Lựa chọn:**
  - `--force` chỉ bỏ qua kiểm Claude Desktop đang chạy (hoặc không kiểm được). Binary ngoài chỗ cài ổn định cần cờ riêng `--allow-any-location`, cờ này không bỏ qua kiểm Desktop. Lời từ chối vì Desktop đang chạy in đường dẫn exe bị coi là Claude Desktop. Thay cho lựa chọn "dùng lại `--force` thay vì thêm cờ" ở mục `pkm mcp install` phía trên.
  - Claude Code cài qua winget, scoop, `%LOCALAPPDATA%\Programs` không còn bị coi là Claude Desktop (danh sách `CLAUDE_CODE_DIRS` trong `crates/pkm/src/install/desktop.rs`). Claude Desktop (`AnthropicClaude\app-*`, gói MS Store `WindowsApps\Claude_*`) và đường dẫn không đọc được vẫn tính là Desktop.
  - `paths`, `loops`, `all_loops` từ chối `max_len` > 12 và dừng sau 10 s (`PKM_QUERY_TIMEOUT` đổi được, `0` tắt) với lỗi `query too expensive`. Chi tiết: `spec/PORTING.md`, mục "Lệch có chủ ý".
- **Lý do:** review mã ngày 2026-11-02. Gộp hai ý vào `--force` khiến người qua lỗi Claude Code bị nhận nhầm là Desktop cũng mất luôn cảnh báo binary nằm trong Downloads, và ngược lại. Một lời gọi tool liệt kê đường trên vault dày khoá cả MCP server một luồng.
- **Bằng chứng:** `crates/pkm/src/install.rs`, `crates/pkm/src/install/{desktop,tests}.rs`, `crates/pkm-core/src/query.rs`, `crates/pkm-core/src/query/tests.rs`; bộ so khớp kg-solo 0 lệch chưa có tên.

## Gói tiếng Việt của OHS: luôn bật khi gói có, giữ gói ghim `0.17.2-vantt.1`
- **Ngày:** 2026-11-02
- **Lựa chọn:** mọi tiến trình OHS mà `pkm` chạy có `OHS_LANGUAGE_PACK=<gói>/lang/ohs-lang-vi` khi thư mục đó có; không thêm cờ tắt. Không nâng gói ghim của bộ cài lên `0.17.2-vantt.2` mà bản Python dùng từ `223c9c3` (nay đã nâng lên `vantt.3`, xem mục "Gói OHS: ghim GitHub Release").
- **Lý do:** theo bản Python (`platform_paths.ohs_env` ở `223c9c3`): mọi bên dùng chung một index phải cùng gói, nếu không OHS dựng lại bảng view qua lại. Bộ cài của Python cũng không có núm tắt. Artifact ghim của `0.17.2-vantt.1` đã có `lang/ohs-lang-vi` (`vi 0.2.0`, cùng Node 22.23.3 và chín bản vá như `vantt.2`), nên nâng gói không cần cho việc này. Release `v0.17.2-vantt.1`…`vantt.3` của fork nay đã có file gói; chuyển bộ cài từ artifact sang Release (mục "Gói OHS: ghim artifact của một run CI của fork") là việc riêng, chưa làm.
- **Bằng chứng:** artifact `bundle-linux-x64` của run `41398112909` (sha256 gói `9160d45e…`, khớp bản ghim) có `lang/ohs-lang-vi/package.json`; `bin/ohs` của gói không đặt biến này. Chạy gói đó trên kg-solo với biến: `status` báo `language_pack.loaded: true`, `id: vi`, `version: 0.2.0`; `tests/ohs_real.rs` qua. Test: `ohs::package::tests::vietnamese_pack_is_found_only_as_a_folder`, `tests/ohs_fake.rs::ohs_runs_with_the_vietnamese_pack_of_its_package`.

## Q2: đổi tên MCP thành `pkm` ở đợt một cửa
- **Ngày:** 2026-11-02
- **Lựa chọn:** khi làm phase một cửa (R14), tên MCP server trong cấu hình Claude Desktop đổi từ `kg` thành `pkm`. Tới lúc đó vẫn giữ `kg` để chạy song song và so khớp với bản Python.
- **Lý do:** user chọn (2026-11-02), cho khớp tên binary.
- **Việc kèm theo:** `pkm mcp install` chuyển mục `kg` cũ sang `pkm` (xoá `kg` do chính nó tạo, giữ mục người dùng tự thêm); cập nhật `CLAUDE.md` mẫu và hướng dẫn ở repo đặc tả khi user cho phép; ghi vào `docs/MCP-CHANGELOG.md`.

## Gói OHS: ghim GitHub Release `v0.17.2-vantt.3` của fork
- **Ngày:** 2026-11-02
- **Lựa chọn:** thay mục "Gói OHS: ghim artifact của một run CI của fork" và phần "giữ gói ghim `0.17.2-vantt.1`" của mục "Gói tiếng Việt của OHS". `pkm mcp install` tự tải asset `ohs-0.17.2-vantt.3-<nền tảng>.tar.gz` (Windows: `.zip`) của Release `v0.17.2-vantt.3` ở repo công khai `vantt/obsidian-hybrid-search` bằng `curl` qua HTTPS, không cần đăng nhập, rồi kiểm sha256 với bản ghim trong binary (`BUNDLES` của `crates/pkm/src/install/ohs_bundle.rs`, một sha cho mỗi nền tảng win32-x64, darwin-arm64, darwin-x64, linux-x64). Bỏ hẳn đường tải bằng `gh` và dạng zip artifact. `--ohs-package` nhận file gói đã tải (phải khớp sha ghim), thư mục gói đã giải nén, hoặc URL https kèm `<URL>.sha256`; `PKM_OHS_PACKAGE` giữ nguyên. Nền tảng không có asset thì báo rõ nền tảng nào có. Hạn 2027-01-31 của artifact không còn liên quan.
- **Lý do:** user chọn (2026-11-02): ghim `vantt.3`. Release của fork công khai nên máy khách không cần `gh` cho OHS (vẫn cần `gh` cho chính `pkm` vì `vantt/obsidian-pkm` riêng tư). `curl` có sẵn trên Windows 10 trở lên, macOS, Linux; không thêm crate HTTP. Smoke của fork qua trên cả bốn nền tảng ở bản này.
- **Bằng chứng:** `gh release view v0.17.2-vantt.3 -R vantt/obsidian-hybrid-search`: bốn gói, mỗi gói có `.sha256` và `.bundle.json`. Tải ẩn danh bằng `curl`, sha256 đo được khớp file `.sha256` của fork: win32-x64 `ad63f1a5…`, darwin-arm64 `8c32b8aa…`, darwin-x64 `f67d90f7…`, linux-x64 `a4178475…`. Gói linux-x64 có `node/`, `app/`, `bin/`, `lang/ohs-lang-vi` (vi 0.2.0), Node 22.23.3. Chạy gói đó trên kg-solo: `tests/ohs_real.rs` và `tests/related_ohs_real.rs` qua; `status` với `OHS_LANGUAGE_PACK` báo `language_pack.loaded: true`, `id: vi`, `version: 0.2.0`; không còn tiến trình `node` nào sau đó. Test tải thật `install::ohs_bundle::tests::downloads_and_installs_pinned_release_asset` (`#[ignore]`) qua.

## Phiên bản crate theo tag pre-release: `0.1.0-rc.1`
- **Ngày:** 2026-11-02
- **Lựa chọn:** `[workspace.package] version = "0.1.0-rc.1"` cho tag `v0.1.0-rc.1`; giữ nguyên kiểm "tag == `v` + phiên bản crate" của `release.yml`, không cắt hậu tố pre-release. Lệnh bộ cài mà `pkm mcp install` in khi binary nằm ngoài chỗ cài ổn định ghim tag của chính bản đang chạy (`gh release download v<phiên bản> --repo vantt/obsidian-pkm …`), thay cho lệnh không tag ở mục `pkm mcp install` phía trên; README và handoff ghi tag `v0.1.0-rc.1` cho tới khi có bản chính thức.
- **Lý do:** user chọn tag `v0.1.0-rc.1` (2026-11-02). Bộ cài (`install.sh` `runs_as_version`, `install.ps1`) kiểm `pkm --version` bắt đầu bằng `pkm <tag bỏ v> `, nên cắt hậu tố ở bước kiểm tag sẽ làm bộ cài từ chối binary. Đổi phiên bản crate không đụng so khớp: `serverInfo.version` của MCP là hằng `SERVER_VERSION = "0.1.0"` (theo Python) trong `crates/pkm/src/mcp.rs`; chỉ `pkm --version` và `clientInfo` khi `mcp install` bắt tay dùng phiên bản crate, cả hai không có trong golden. Tag có dấu `-` đã thành pre-release ở bước `publish`. GitHub không coi pre-release là "latest", nên `gh release download` không tag sẽ không tìm thấy bản `v0.1.0-rc.1`.
- **Bằng chứng:** `grep CARGO_PKG_VERSION crates`: `cli.rs` (`--version`), `install/handshake.rs` (`clientInfo`). `scripts/release-assets.sh version` in `0.1.0-rc.1`; `package` chạy thử in `pkm 0.1.0-rc.1 (spec 223c9c3…)`; `scripts/test-installer.sh <out> v0.1.0-rc.1` qua mọi bước. So khớp kg-solo như CI: 0 lệch chưa có tên.

## OHS: một chỗ cài mỗi máy, nâng gói cũ, gói vi không nạp là hỏng, mẫu bỏ qua `_ohs/**`
- **Ngày:** 2026-11-02
- **Lựa chọn:**
  - `pkm mcp install … --ohs DIR` dùng gói OHS có sẵn ở `DIR`: ghi `--ohs DIR` vào `args` của mục `kg`, không cài gói vào `<thư mục pkm>/ohs`. `doctor` đọc `--ohs` từ mục `kg`; lệnh dựng lại index bỏ cờ này. Bản B dùng `--ohs C:\tools\ohs`. Về sau `pkm` làm chủ chỗ cài và script dự án trỏ theo.
  - Gói đã cài ở `<thư mục pkm>/ohs` cũ hơn bản ghim (`version` trong `MANIFEST.json`, `OhsPackage::version`; không có phiên bản cũng tính là cũ) thì `pkm mcp install` nâng; tải lỗi thì giữ gói cũ và cảnh báo. Gói bằng hay mới hơn bản ghim, hoặc phiên bản không so được (`ohs_bundle::older_than_pin`), thì giữ nguyên: bộ cài không bao giờ hạ cấp. Gói chỉ bằng `--ohs DIR` hay `PKM_OHS_PACKAGE` thì không bao giờ bị ghi đè hay tự nâng. `doctor` báo phiên bản và gói tiếng Việt ở `ohs_package`; cũ hơn bản ghim hay thiếu gói tiếng Việt là `warn` (lệnh sửa `mcp install` chỉ gợi ý khi gói ở thư mục mặc định), mới hơn là `ok`.
  - `status` có `language_pack.loaded: false` thì trạng thái là `broken` (không trả kết quả tìm thiếu xử lý tiếng Việt). Tín hiệu index xong giữ `pending == 0`: OHS tính `notes_without_chunks = total - indexed`, nên `indexed + notes_without_chunks == total` luôn đúng; `failed_indexing > 0` không chặn.
  - Có gói tiếng Việt và bật `PACK_IGNORE_ON` thì OHS chạy thêm `OBSIDIAN_IGNORE_PATTERNS=.obsidian/**,templates/**,*.canvas,_ohs/**`, như `platform_paths.ohs_settings` của Python ở `24f193a`. Công tắc tắt từ `58010ad` tới khi bản B index lại với mẫu này; bật từ 2026-11-02 cùng `OHS_IGNORE_ON = True` ở `8f89403` (E12.9.0: bản B lên `vantt.4`, có `_ohs/lexicon.md`, `reindex --force`). Phần còn lại của commit đó (`kg_adapter` coi `_ohs/` là thư mục hệ thống) đã chuyển ở lượt đuổi theo thứ hai: `adapter::e11::SYSTEM` có `_ohs/`.
- **Lý do:** agent cài fork trên bản B (2026-11-02): hai tiến trình khác cấu hình gói ngôn ngữ trên một DB OHS làm bên làm chủ index dựng lại bảng view qua lại; khác mẫu bỏ qua thì OHS báo lệch content policy. Gói vi `vantt.4` đọc từ điển ở `VAULT/_ohs/lexicon.md`, không phải note. Bản B chạy `0.17.2-vantt.3` ở `C:\tools\ohs` với gói vi, index 2.081 note, 0 lỗi.
- **Còn mở:** `related` chỉ có câu hỏi vẫn tìm ở chế độ mặc định (`hybrid`) như Python; trên bản B đáp án câu hỏi theo ý đứng hạng 7 ở `semantic`, 41 ở `hybrid`. Đổi ở repo đặc tả trước. Ghim đã nâng lên `v0.17.2-vantt.4` (mục "Gói OHS: nâng ghim lên GitHub Release `v0.17.2-vantt.4`").
- **Bằng chứng:** mã fork `src/storage/db.ts` (`withoutChunks: total - indexed`), bản vá `0001-language-pack-api.patch` (`language_pack` trong `status`); gói thật `vantt.3` trên kg-solo: `language_pack.loaded: true`, `pkm related "khach hang"` ra "Chân dung khách hàng". Test: `install::tests::{ohs_flag_points_kg_at_an_existing_package_without_installing_one, ohs_older_than_the_pin_is_upgraded_and_the_pin_or_newer_is_kept}`, `install::ohs_bundle::tests::older_than_pin_never_counts_newer_or_unknown_forms_as_old`, `doctor::tests::ohs_package_older_than_the_pin_or_without_vietnamese_pack_warns`, `ohs::state::tests::{language_pack_that_did_not_load_is_broken, ready_with_notes_without_chunks_and_failed_notes}`, `tests/ohs_fake.rs::ohs_runs_with_the_vietnamese_pack_of_its_package`.

## Gói OHS: nâng ghim lên GitHub Release `v0.17.2-vantt.4`
- **Ngày:** 2026-11-02
- **Lựa chọn:** thay bản ghim của mục "Gói OHS: ghim GitHub Release `v0.17.2-vantt.3` của fork"; cách tải, kiểm và nguồn thay thế giữ nguyên. `VERSION`/`TAG` và sha256 bốn nền tảng trong `crates/pkm/src/install/ohs_bundle.rs`: win32-x64 `5cb480f8…`, darwin-arm64 `ac2c3175…`, darwin-x64 `37eed430…`, linux-x64 `e6972863…`.
- **Lý do:** phase một cửa (R14) cần action `read` của giao thức stdio (bản vá `stdio-parity` của fork) để `pkm` chuyển tiếp `read` sang OHS qua tiến trình đang chạy. User chỉ định bản `vantt.4`. Bản này dựng lại cả chín bản vá trên upstream `v0.17.2`, Node 22.23.3, `vi 0.2.0`; smoke của fork qua trên cả bốn nền tảng.
- **Bằng chứng:** `gh release view v0.17.2-vantt.4 -R vantt/obsidian-hybrid-search`; tải bốn gói, sha256 đo được khớp file `.sha256` đăng kèm và `digest` của asset. Gói linux-x64 trên kg-solo: `tests/ohs_real.rs` (`search` và `read`), `tests/related_ohs_real.rs` và `install::ohs_bundle::tests::downloads_and_installs_pinned_release_asset` (`#[ignore]`) qua; `status` báo `version: 0.17.2-vantt.4`, `language_pack.loaded: true`; action `read` trả `{id, notes}`, `paths` rỗng trả `{id, error}`.

## Một cửa: `read` đọc thẳng file trong vault, không qua OHS
- **Ngày:** 2026-11-02
- **Lựa chọn:** tool `read` của MCP `pkm` đọc thân note từ file trong vault (bỏ frontmatter như tool `note`), không chuyển tiếp sang action `read` của OHS. Tham số `nodes` (1–20 note id, giải như `note`: NFC, id sai trả `did_you_mean`), `max_chars` (mặc định 4000, 0 là cả note), `offset`. Kết quả `{items: [{id, title, type, url, chars, truncated, body}]}`; id sai là lỗi trong mục của nó. Cả kết quả tự giữ dưới `--max-chars` (cắt đều thân note). Lệch PRD mục 3.1 ("`read` chuyển tiếp mỏng sang OHS") và câu chữ của phase 17.
- **Lý do:** user chọn (2026-11-02) theo hai báo cáo thiết kế (brainstormer, kongming). Đọc một file không cần index theo nghĩa; chuyển tiếp thì trong lúc OHS khởi động, index lần đầu (khoảng 10 phút với vài nghìn note) hay thiếu model, agent không đọc được note nào dù tool graph vẫn chạy, trái D7. `pkm` đã có đủ thứ cần (thư mục vault, `did_you_mean`, `url`, `title`/`type` trong index). `Ohs::read` vẫn giữ trong `pkm-core::ohs` cho plugin hay CLI về sau.
- **Đổi ý khi:** plugin hoặc máy chủ điện thoại cần `read` cho ra đúng dạng `readNotes` của OHS (`links`, `backlinks`, `markdownLinks`) để dùng chung một hợp đồng.
- **Bằng chứng:** `crates/pkm-core/src/tools.rs` (hàm `read`, `fit_bodies`), `tools::tests::read_*`, `crates/pkm/tests/mcp_tools.rs::read_fits_max_chars_without_ohs` (server chạy `PKM_NO_OHS=1`, `--max-chars 2000`, kết quả vẫn là JSON nguyên vẹn).

## Một cửa: chưa làm "code mode"; điều kiện xem lại
- **Ngày:** 2026-11-02
- **Lựa chọn:** không thêm tool chạy script hay bộ tìm tool lười (`tools_search`/`tool_describe`/`tool_call` kiểu `vantt/browser-worker`). Giữ 13 tool rời; phần "gộp lời gọi" làm bằng tham số phía server (`related` với `query` hoặc `node` = tìm + đi graph trong một lượt).
- **Lý do:** user chọn (2026-11-02) theo hai báo cáo thiết kế. Claude Desktop không có shell, nên code mode phải nhúng trình thông dịch (QuickJS hay Rhai) vào binary trên bốn đích, trái tinh thần Q4; tìm tool lười thêm hai lượt mỗi phiên và làm model không còn thấy `find` cạnh `search` để chọn. Sau khi thiết kế lại, các câu nhóm A–D cần tối đa 2–3 lượt.
- **Xem lại khi:** đo playbook (việc còn mở của phase 17) cho thấy từ 2 câu trở lên vượt giới hạn số lượt gọi tool (H4: nhóm B dưới 3 lượt, nhóm C dưới 5; thường là các câu "phép tập hợp" B4, C4, D5). Khi đó mở một phase riêng cho tool `run` dạng hybrid (giữ 13 tool, thêm `run`), và trước đó xét tham số hay tool nhỏ cho đúng phép còn thiếu (bậc của note, giao hai `reach`, lọc `created <`).
- **Bằng chứng:** phase 17 (`plans/261102-0535-rust-core-port-master/phase-17-one-door-mcp.md`, mục "Điều kiện xem lại code mode").

## Một cửa: mặt tool `search` và cỡ `tools/list`
- **Ngày:** 2026-11-02
- **Lựa chọn:** `search` chỉ phơi `query`, `queries`, `path`, `mode`, `scope`, `tag`, `limit` (10), `snippet_length` (200) và `system` (khi vault có trang hệ thống, như các tool khác), đúng tên MCP của OHS và tự đổi sang tên stdio (`snippetLength`, `notePath`, `queries` khi có từ hai câu). Không phơi `frontmatter` (trùng `select`), `threshold`, `paths`, `related`/`depth`/`direction`/`link_type` (trùng `neighbors`/`reach`), `rerank` (tải model khoảng 570 MB), `anchors`, `explain`. Kết quả chỉ có `id`, `title`, `type`, `score`, `snippet`. `search` và `related` chờ tối đa 8 giây khi OHS đang `starting`; lỗi OHS khác `ohs_not_ready` là `ohs_error`. Test ghim cỡ `tools/list`: đo được 17.664 ký tự (JSON gọn) không có trang hệ thống, 19.160 khi có (11 tool của Python: 13.403); ngân sách 18.500 và 20.000.
- **Lý do:** user chốt chữ ký (2026-11-02). Mỗi tham số thừa tốn token mỗi phiên và thêm một cách chọn sai; kết quả thô của OHS nặng 400–900 byte một hit (mảng link, bốn điểm thành phần). Ước lượng ban đầu "khoảng 17 KB" cho cả danh sách; số đo vượt khoảng 4% vì giữ nguyên câu mô tả đã chốt cho `search`, `read`, `related`.
- **Bằng chứng:** `crates/pkm-core/src/tools/defs.rs`, `tools::tests::tools_list_stays_within_its_size_budget`, `tests/related_ohs_fake.rs::search_maps_arguments_to_stdio_and_trims_results`.

## Một cửa: bộ cài đổi tên mục `kg`, không đụng `ohs`, làm ấm index tìm kiếm
- **Ngày:** 2026-11-02
- **Lựa chọn:** `pkm mcp install` ghi mục `pkm` (Q2). Mục `kg` do chính `pkm` ghi (`command` là binary `pkm`, `args` là `VAULT mcp …`) được đổi tên tại đúng chỗ; mục `kg` không do pkm ghi (bản Python chạy song song) giữ nguyên. Không ghi, sửa hay xoá mục `ohs`; có mục đó thì in câu báo là có thể bỏ, `pkm doctor` cảnh báo (`ohs_mcp`, `warn`, không có `fix`). `--remove` xoá `pkm` và mục `kg` cũ do pkm ghi. Sau khi ghi cấu hình và bắt tay, chạy gói OHS trên vault (cùng DB mà MCP `pkm` dùng) tới khi index xong, hỏng, thiếu model, hoặc 30 phút, in tiến độ; chưa xong chỉ là cảnh báo, không làm lệnh cài thất bại. `--no-warm` bỏ qua. Mã kiểm `kg_entry` của `doctor --json` giữ nguyên (hợp đồng mã ổn định), mục `kg` cũ do pkm ghi là `warn` kèm lệnh cài lại.
- **Lý do:** user chọn (2026-11-02): thôi đăng ký `ohs`, không xoá `ohs` user tự thêm, không cấm dùng song song, làm ấm index mặc định. Làm ấm sau khi ghi cấu hình để người dùng bấm Ctrl+C vẫn có cấu hình đúng; OHS index tiếp khi Claude Desktop mở. Đổi tên tại chỗ giữ thứ tự mục người dùng đã quen trong file.
- **Bằng chứng:** `crates/pkm/src/install.rs` (`put_entry`, `written_by_pkm`, `warm`), `install::tests::{install_renames_the_kg_entry_written_by_pkm_and_keeps_ohs, install_keeps_a_kg_entry_not_written_by_pkm, remove_also_drops_the_old_kg_entry_written_by_pkm, install_warms_the_ohs_index_unless_told_not_to}`, `doctor::tests::{legacy_kg_entry_written_by_pkm_warns_with_reinstall_fix, separate_ohs_mcp_only_warns}`.

## Bộ cài tự đọc link trong thân note khi graph theo schema rỗng
- **Ngày:** 2026-11-04
- **Lựa chọn:** lõi giữ mặc định của Python (`--edge-keys schema`, không `--body-edges`). `pkm mcp install` không có `--edge-keys`, `--body-edges` hay `--schema-edges-only` mà index dựng theo mặc định có note và 0 cạnh thì dựng lại với `--edge-keys links --body-edges`; có cạnh thì ghi hai cờ đó vào mục `pkm` và in ra, vẫn 0 cạnh thì dựng lại với `--edge-keys schema` và giữ mục như cũ. `--schema-edges-only` tắt việc này. `pkm doctor` có mục `edges`: `warn` khi index có note, 0 cạnh và không đọc thân note, `fix` là lệnh cài lại với hai cờ.
- **Lý do:** thử rc.4 trên Windows Sandbox (2026-11-04): vault thử có wikilink trong thân note mà `cạnh theo lớp: {}`, degree mọi note 0; bản B không gặp vì chạy với `--body-edges`. Vault Obsidian thường sẽ có graph rỗng. User chọn (2026-11-04) để bộ cài tự phát hiện thay vì đổi mặc định của lõi: không thêm lệch nào trong bộ so khớp, vault có schema không đổi.
- **Đổi ý khi:** repo đặc tả đổi mặc định cách đọc cạnh, hoặc người dùng cần graph chỉ theo schema dù vault có link (khi đó dùng `--schema-edges-only`).
- **Bằng chứng:** `crates/pkm/src/install.rs` (`body_edges_fallback`, `graph_size`, `BODY_EDGE_FLAGS`), `install::tests::{empty_graph_switches_to_body_edges_and_writes_the_flags, vault_without_links_keeps_the_default_reading, chosen_edge_reading_or_existing_edges_never_switch}`, `doctor::tests::empty_graph_without_body_edges_warns_with_reinstall_fix`; chạy tay trên vault hai note chỉ có wikilink trong thân: 0 cạnh, dựng lại, 2 cạnh, mục `pkm` có `--edge-keys links --body-edges`.

## Repo `vantt/obsidian-pkm` chuyển sang public (2026-11-03)

User tự chuyển repo sang public khi GitHub Actions của repo private bị chặn vì hạn mức thanh toán. Thay quyết định giữ private trước đó. Hệ quả: tải Release không còn cần `gh` đăng nhập; trước khi chuyển đã rà lịch sử git theo mẫu token GitHub, khoá Anthropic, khoá AWS, private key và không thấy gì. Không đưa nội dung vault thật (bản B) vào repo vẫn là ràng buộc.

## Tách vai hai repo; `obsidian-pkm` là nguồn sự thật của lõi từ 2026-11-03
- **Ngày:** 2026-11-03
- **Lựa chọn:**
  - Repo này là repo sản phẩm: mọi thứ để dựng một bản B hoàn chỉnh trên máy khách: code, preset bản B (`schema.yaml`, adapter e11, mẫu `CLAUDE.md`, template, theme), hướng dẫn người dùng (phần playbook dành cho khách), bộ cài, Release. Repo đặc tả `vantt/obsidian-pkm-automation` thành repo nghiên cứu: thử nghiệm, số đo, PRD và kế hoạch, chất liệu blog; code ở đó là nguyên mẫu, không giao khách. Fork OHS vẫn là repo riêng.
  - Từ 2026-11-03 repo này là nguồn sự thật của lõi (graph, tools, MCP, adapter, index). Spec ghim `da62ecb` là lần ghim cuối; phase 15 (đuổi theo commit Python) đóng, không còn lượt đuổi theo. Tính năng mới viết thẳng ở đây, có test và golden riêng.
  - Repo nghiên cứu không viết thêm Python, kể cả sửa tính năng. Script Python hiện có chạy nguyên trạng cho bản B tới khi được chuyển; muốn đổi hành vi script nào thì chuyển nó trước rồi đổi trên bản mới.
  - Quy tắc chuyển: lõi, tính nặng, đóng gói (đọc cả vault, index, `propose`, `material`, `zones`, setup) viết Rust ở đây; tích hợp Obsidian (giao diện, hành vi, `workshop_ui`, review log) viết TS trong plugin; script vốn là JS thì giữ JS. Python nội bộ của repo nghiên cứu (`normalize`, `highlight_colours`, `migrate_v3`, `selection_signals`) đóng băng, chuyển khi cần dùng lại.
  - Tuần chạy song song (phase 16) vẫn giữ để user duyệt chất lượng trước `v0.1.0`. Bản Python `kg` chỉ là đường lùi, không còn là đặc tả.
- **Lý do:** user chốt 2026-11-03. Rust đã bắt kịp Python: bộ so khớp trên bản B và kg-solo 0 lệch chưa có tên. Giữ Python làm đặc tả nữa là viết mỗi tính năng hai lần. Khách cần một chỗ lấy đủ đồ; đồ giao khách trộn với nguyên mẫu và số đo thì không rõ thứ gì là giao.
- **Bằng chứng:** phase 10 trên bản B (rc.3, spec `da62ecb`): 54 mục, 0 lệch chưa có tên; CI so khớp kg-solo 0 lệch. Mục quyết định cùng ngày trong `docs/GATES.md` của repo nghiên cứu.

## Cài đặt là `pkm setup`, không làm app cài riêng
- **Ngày:** 2026-11-03
- **Lựa chọn:** lệnh `pkm setup VAULT` của chính binary `pkm` thay `scripts/e11/setup_vault.py` của repo nghiên cứu. Không làm app cài riêng. Khách cần giao diện thì sau làm vỏ GUI mỏng bằng Tauri gọi `pkm setup --json`. Plugin Obsidian (TS) chỉ lo giao diện và hành vi, gọi `pkm … --json`, không tự tính nặng. Một tag ra cả binary và plugin cùng phiên bản.
- **Lý do:** phần lớn việc cài đã có trong `pkm`: `mcp install`, tải gói OHS ghim và kiểm sha256, `doctor`. Một binary thì một chỗ ký mã, một chỗ phát hành, không phải giữ khớp phiên bản giữa hai app.
- **Đổi ý khi:** cần app cài riêng thật sự, ví dụ khách không mở được terminal và vỏ Tauri không đủ.

## Bố cục `apps/` + `packages/`; đổi thư mục sau khi `phase-17-one-door` merge
- **Ngày:** 2026-11-03
- **Lựa chọn:**

  ```
  apps/
    pkm/               Rust bin: CLI, MCP, doctor, setup, serve
    obsidian-plugin/   TS: giao diện, hành vi trong Obsidian
    setup-gui/         (sau) Tauri, vỏ mỏng gọi `pkm setup --json`
  packages/
    pkm-core/          Rust lib: vault, index, graph, related, propose, zones, OHS client
    pkm-contract/      kiểu JSON chung, sinh từ Rust (schemars/ts-rs) ra TS cho plugin và GUI
  presets/
    b/                 schema, adapter, CLAUDE.md, template, theme (dữ liệu)
  tools/
    parity/, probes/   bộ so khớp, probe (không giao khách)
  Cargo.toml           workspace: apps/pkm, packages/pkm-core, tools/parity
  pnpm-workspace.yaml  apps/obsidian-plugin, packages/pkm-contract
  ```

  Đổi `crates/` (và `parity/`, `probes/`) sang bố cục này trong một PR, cùng lúc dựng khung plugin, khi `phase-17-one-door` đã merge, không còn nhánh chờ merge và CI xanh. Các việc khác (chuyển preset bản B và hướng dẫn người dùng sang đây, chuyển script E11) không đợi. Kế hoạch: `plans/261103-product-layout/plan.md`.
- **Lý do:** ba thứ chạy được (binary, plugin, GUI sau này) dùng chung lõi và hợp đồng JSON. `pkm-contract` sinh kiểu TS từ Rust nên lệch hợp đồng lộ ra lúc build. Cargo và pnpm workspace đều cho thành viên ở thư mục tuỳ ý. Đổi thư mục bây giờ đụng nhánh `phase-17-one-door` đang chờ merge.
- **Đổi ý khi:** plugin và GUI không thành hình sau `v0.1.0` thì giữ `crates/` và `plugin/` cũng được.
