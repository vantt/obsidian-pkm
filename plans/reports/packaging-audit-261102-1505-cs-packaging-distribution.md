# Audit đóng gói và phát hành `pkm` theo `cs-packaging-distribution`

- **Ngày:** 2026-11-02 15:05 (Asia/Saigon)
- **Phạm vi:** nhánh `main` ở `7d14601`. Chỉ đọc, không sửa gì.
- **Đã đọc:** `README.md`, `docs/PRD.md` (mục 1, 2, 3, 9–16), `docs/DECISIONS.md`, `.github/workflows/{ci,release}.yml`, `Cargo.toml` (gốc và hai crate), `rust-toolchain.toml`, `crates/pkm/build.rs`, `crates/pkm/src/{main,install,doctor}.rs`, `crates/pkm/src/install/{config,desktop,handshake,ohs_bundle}.rs`, `crates/pkm-core/src/ohs/package.rs`, `crates/pkm-core/src/platform.rs`, cùng plan phase 13 và 14.
- **Ngoài phạm vi (đang làm ở worktree khác):** lệnh `cli`, `mcp`, `query`, `tools`. `pkm` hiện chưa đăng ký `mcp install`, `doctor`, `build`; theo yêu cầu, việc này không tính là phát hiện.
- **Giữ nguyên:** D1–D10, Q3, Q4 và quyết định ghim artifact OHS. Chỗ nào thật sự va với một quyết định này thì ghi ở cuối, dưới dạng câu hỏi.

## 1. Hồ sơ dự án và archetype

| | |
|---|---|
| Ngôn ngữ | Rust (overlay `languages/rust.md`). `parity/follow.py` là công cụ của dev, không phát hành, nên không tính là polyglot |
| Loại | CLI, cộng MCP server stdio (`kg`) do Claude Desktop khởi chạy. Không phải daemon: SVC n/a |
| Nền tảng | Chính là Windows x64. Có thêm macOS arm64/x64 và Linux x64 (gnu) |
| Cấu hình, bí mật | Không có file cấu hình riêng, không có bí mật. Có sửa cấu hình của bên thứ ba (`claude_desktop_config.json`) và có dữ liệu suy ra (index SQLite, gói OHS kèm Node) trong `platform::pkm_dir()` |
| Phụ thuộc lúc chạy | Gói OHS (Node đi kèm) là tuỳ chọn ở giai đoạn so khớp. **Tải gói tự động cần `gh` đã đăng nhập** |
| Archetype | **A, một binary.** Dự án tham chiếu gần nhất: **mdview** (Rust CLI + MCP, update, cặp doctor/uninstall) |

Bảng quyết định (template checklist; tầng 1 trước):

| Quyết định | Hiện trạng |
|---|---|
| Tagline (đau hoặc lợi của người dùng) | Chưa có. README mở đầu bằng kiến trúc ("Lõi Rust… index graph… thay cho bản Python") |
| Lời hứa quick start | Chưa có |
| Động từ | PRD mục 10–11: `pkm VAULT <lệnh>` như Python, cộng `--version`, `mcp install [--check/--dry-run/--remove]`, `doctor`. Không có `init`, `update`, `uninstall` |
| `init --yes` một mình cho ra máy chạy được? | Tương đương gần nhất là `pkm mcp install VAULT`: không hỏi gì và từ chối thay vì chờ, tức đạt CMD-17 |
| Lệnh cài một dòng | Không có. Người dùng tải tay `pkm-x86_64-pc-windows-msvc.exe` |
| Ngân sách phụ thuộc | Binary tĩnh trừ CRT của MSVC (xem B1). OHS cần `gh` (xem I1) |
| Định dạng cấu hình | Không có file cấu hình (zero-config), đạt UX-10 |
| Nguồn phiên bản, đích, tên asset | `[workspace.package] version` + `env!`, đạt RS-01. Bốn đích. Asset là binary trần `pkm-<target>[.exe]` + `.sha256` |
| Chính sách PATH, ký, host | Chưa chọn chính sách PATH. Chưa ký mã (PRD mục 12, có chủ ý). Host: Claude Desktop |

## 2. Verify (bảng 12 bước của skill)

Hook `scout-block` của repo chặn `cargo build` và mọi đường dẫn `target/`, nên các bước chạy binary được suy ra từ mã nguồn, không chạy thật. Kết luận vẫn chắc, vì `main.rs` chỉ có `Args {}` với `version`.

| # | Rule | Kết quả | Bằng chứng |
|---|---|---|---|
| 1 | ONB-01/04/06 | **gap** | `README.md:1-9`: không có tagline nói về người dùng, quick start hay lời hứa. Dòng trạng thái đã cũ ("Chưa có lõi; chờ cổng G0" trong khi index/validator đã merge ở `d97040b`) |
| 2 | INS-02/04 | **gap** | Không có `install.sh` hay `install.ps1` |
| 3 | CMD-06 | một phần | `--version` chạy (`main.rs:12-20`, `tests/version.rs`). Không có `version --json` |
| 4 | CMD-15/19 | gap (đang làm) | `main.rs:19-25`: chạy không tham số thì thoát 0 mà không in gì. Thuộc phần CLI đang làm, chỉ ghi để không quên |
| 5 | CMD-16 | n/a lúc này | Chưa đăng ký lệnh con |
| 6 | CMD-08/12 | pass (theo mặc định của clap) | clap trả mã 2 kèm gợi ý |
| 7–9 | CMD-03/09/11/17, DOC-02 | gap về thiết kế | `doctor --json` không theo envelope chuẩn, không có `--fix`, `fix` không phải lệnh chạy được (xem I4) |
| 10 | CMD §1 | **gap** | Không có `update` |
| 11 | REL-06/10, VER-02 | **gap** | Không có `checksums.txt` gộp, không có cổng tag == version, action chưa ghim SHA (`release.yml:84,88,110,124`) |
| 12 | TST-04 | chưa chạy được | Cần CLI xong mới chạy được |

## 3. Phát hiện, xếp theo mức

### Blocker: chặn bản cài được đầu tiên trên Windows sạch (nghiệm thu R11, phase 13)

**B1. Binary Windows phụ thuộc `VCRUNTIME140.dll`, máy sạch có thể không chạy được.**
- *Bằng chứng:* `release.yml:91` build `x86_64-pc-windows-msvc` với cấu hình mặc định. Repo không có `.cargo/config.toml` hay `RUSTFLAGS` nào bật `crt-static`. Target MSVC của Rust mặc định liên kết động với `vcruntime140.dll`, mà file này không thuộc Windows (Windows Sandbox và nhiều máy mới cài không có). Claude Desktop khởi chạy `pkm.exe` thì lỗi ngay, không có thông báo nào dễ hiểu.
- *Rule:* UX-07 MUST (không bắt cài thư viện hệ thống), UX-03, ONB-14.
- *Sửa:* thêm `.cargo/config.toml` với `[target.x86_64-pc-windows-msvc] rustflags = ["-C", "target-feature=+crt-static"]`, hoặc đặt `RUSTFLAGS` cho job Windows trong `release.yml`. Thêm bước CI chạy `dumpbin /dependents pkm.exe` (hoặc thử chạy `pkm --version` trong Windows Sandbox hay runner sạch) và fail nếu còn `VCRUNTIME`.

**B2. Không có đường cài: người dùng tự tải, tự đổi tên, tự đặt chỗ cho `.exe`, và `mcp install` ghi đúng cái đường dẫn tạm đó vào cấu hình Claude Desktop.**
- *Bằng chứng:* release chỉ có binary trần `pkm-x86_64-pc-windows-msvc.exe` (`release.yml:100`), không có `install.ps1`. `install.rs:182` lấy `std::env::current_exe()` làm `command` của mục `kg`. Ai chạy `pkm mcp install` từ `Downloads\pkm-x86_64-pc-windows-msvc.exe` thì Claude Desktop sẽ gọi đúng file trong Downloads; xoá hay dời file là `kg` hỏng. `doctor` chỉ phát hiện sau khi đã hỏng (`doctor.rs:278-280`).
- *Rule:* INS-01/09/10/13/17 MUST, UX-02/03 MUST, ONB-07 MUST, AGT-04 (đường dẫn tuyệt đối *ổn định*).
- *Sửa:*
  1. Viết `install.ps1` (phỏng theo `skillstation/scripts/install.ps1`; bản gọn: `forgent/scripts/install.ps1`). Nó: dùng TLS 1.2, tải asset qua `releases/latest/download/`, kiểm `Get-FileHash` khớp đúng tên file, đặt vào `%LOCALAPPDATA%\pkm\bin\pkm.exe` (cùng gốc với `platform::pkm_dir()`), in dòng PATH (chính sách A) và bước kế tiếp `"<đường dẫn đủ>\pkm.exe" mcp install "<vault>"`. Đính `install.ps1` vào release, đóng dấu phiên bản (REL-08).
  2. Trong `mcp install`, cảnh báo (hoặc từ chối nếu không có `--force`) khi `current_exe()` nằm ngoài thư mục cài chuẩn, ví dụ trong `Downloads` hay `%TEMP%`, và in lệnh cài đúng chỗ.

**B3. Chưa có tài liệu "từ không có gì tới Claude Desktop gọi được `kg`".**
- *Bằng chứng:* `README.md:1-9` chỉ có bối cảnh dự án. Nghiệm thu phase 13 là "User chạy `pkm mcp install` trên Windows sạch", nhưng không có trang nào ghi các bước. Bộ cài Python (`desktop_config.py`) có hướng dẫn ở repo đặc tả, còn `pkm` thì chưa.
- *Rule:* ONB-01/04/06/07/08/12 MUST.
- *Sửa:* thêm phần Quick start cho người dùng lên đầu README (hoặc `docs/installation.md` có link ngay dòng đầu README). Gồm: lời hứa ("Từ máy Windows trống tới Claude Desktop hỏi được graph vault trong khoảng 5 phút. Cần: Claude Desktop, vault Obsidian."); ba bước (`irm …/install.ps1 | iex`, đóng hẳn Claude Desktop, `pkm mcp install "<vault>"`); câu cần hỏi Claude để thấy kết quả; và "nếu lỗi: `pkm doctor`". Chuyển bối cảnh port Rust xuống dưới. Sửa dòng trạng thái đã cũ. README là tiếng Việt, đúng đối tượng khách (ONB-05 đạt).

### Important: nên xong trước khi phát cho người khác ngoài user

**I1. Tải OHS tự động cần `gh` đã đăng nhập, và artifact ghim hết hạn ngày 2027-01-31 (còn khoảng 3 tháng).**
- *Bằng chứng:* `ohs_bundle.rs:8-10,183-211` gọi `gh api repos/…/actions/artifacts/<id>/zip`. `ohs_bundle.rs:40` ghi hạn 2027-01-31. `RUN_ID`, `BUNDLES` là hằng số biên dịch vào binary (`ohs_bundle.rs:23,41`), nên sau ngày đó mọi bản `pkm` đã phát hành đều không tự tải được OHS nữa. Trên máy khách sạch thì không có `gh`.
- *Rule:* UX-07/08 MUST (không bắt cài công cụ phụ), ONB-08. Phần kiểm hai lớp sha256 thì đạt SEC-01.
- *Sửa (giữ quyết định ghim artifact):* ở giai đoạn so khớp, `kg` vẫn chạy không cần OHS (`install.rs:488-519` chỉ cảnh báo), nên giữ nguyên hành vi đó. Thêm vào README và `doctor` một dòng nói rõ: không có OHS thì chỉ `related` khi không có `hits` bị ảnh hưởng. Đặt một mốc trong plan để chuyển sang Release của fork trước 2027-01-31. Cho URL hoặc nguồn gói ghi đè được bằng biến môi trường (`PKM_OHS_RELEASE_BASE`), để không phải ra bản `pkm` mới chỉ vì đổi nguồn. Xem câu hỏi Q-a.

**I2. `doctor` đánh `fail` khi chưa có gói OHS, trong khi `mcp install` coi đó là cảnh báo. Máy cài đúng hướng dẫn vẫn bị `doctor` báo hỏng.**
- *Bằng chứng:* `doctor.rs:318-335` (`ohs_package` → `Status::Fail`), `doctor.rs:116-118` (`healthy` = không có `fail`). Ngược lại, `install.rs:503-517` và `--no-ohs` (`install.rs:76-78`) coi OHS là tuỳ chọn. Nghiệm thu phase 13 đòi "`pkm doctor` trên máy đó không báo lỗi". Máy không có `gh` sẽ trượt nghiệm thu dù `kg` chạy tốt.
- *Rule:* DOC-02 MUST (mã thoát phản ánh đúng trạng thái), CMD-01 (doctor và install chung một định nghĩa "đúng").
- *Sửa:* khi giai đoạn so khớp chưa xong, `ohs_package` thiếu là `warn` (giống `install`). Hoặc đọc từ mục `kg` xem lần cài có `--no-ohs` không, và khi đó trả `skip`.

**I3. `doctor` chạy thử OHS mặc định, tức có thể ghi dữ liệu (index của OHS) và nạp model.**
- *Bằng chứng:* `doctor.rs:337-364` gọi `hooks.ohs_state` → `Ohs::start` (`install.rs:149-154`). OHS khởi động với `OBSIDIAN_VAULT_PATH` và không đặt `OBSIDIAN_DB_PATH` (`ohs.rs:47,519-524`), nên dùng chung DB với MCP `ohs` của người dùng và có thể bắt đầu index. Muốn tắt thì phải tự nhớ `--no-ohs-start`.
- *Rule:* DOC-03 MUST (chẩn đoán không ghi gì), CMD-02.
- *Sửa:* mặc định chỉ kiểm gói (file đủ, `node --version`). Chỉ chạy thử OHS khi có cờ rõ ràng (`--deep` hoặc `--ohs-start`), hoặc hỏi trạng thái qua lệnh chỉ đọc của OHS nếu fork có.

**I4. Đầu ra máy đọc của `doctor` chưa đủ cho agent tự sửa.**
- *Bằng chứng:* `doctor.rs:124-135` xuất `{"healthy","checks":[{id,status,detail,action}]}`. `action` là câu văn (ví dụ `doctor.rs:368-370`, `239-240`), không phải lệnh chép chạy được. Không có `--fix`, `schema`, `next`.
- *Rule:* CMD-11 MUST (envelope `{schema,ok,data,error,next}`, `fix` chạy được), CMD-18, DOC-01 (`fix` là lệnh), DOC-02 (`--fix`).
- *Sửa:* giữ khoá `checks` trong `data`, thêm `schema: 1`, đổi `healthy` thành `ok`. Mỗi check có hai trường: `fix` là lệnh không tương tác (ví dụ `pkm mcp install "<vault>" --force`, hay lệnh PowerShell bật long path), còn `action` giữ câu giải thích. Thêm `next`. `--fix` có thể để sau, nhưng khoá thì nên chốt ngay vì CMD-11 đòi giữ ổn định qua các bản. Đây là lúc rẻ nhất để chốt, vì `doctor` chưa có người gọi.

**I5. `mcp install` đọc cấu hình trước, dựng index (có thể mất vài phút), rồi mới ghi. Không kiểm lại file, không kiểm lại Claude Desktop.**
- *Bằng chứng:* `install.rs:231` (`ConfigDoc::load`), `289` (kiểm Desktop đang chạy), `295-297` (dựng index), `299` (`write_atomic`). Nếu trong lúc dựng index người dùng mở Claude Desktop, hoặc app hay công cụ khác sửa file, thì bản ghi sẽ đè mất thay đổi đó. Có bản `.bak-*`, nhưng người dùng không biết.
- *Rule:* AGT-03 MUST (compare-and-swap theo digest của bản đọc), PRN-03.
- *Sửa:* lưu sha256 của bytes đã đọc. Ngay trước `write_atomic`, gọi lại `refuse_if_desktop_running` và đọc lại file; digest khác thì nạp lại, áp `set_server` lại, và báo cho người dùng. Một cách khác: dựng index trước, đọc cấu hình sau.

**I6. Release thiếu các cổng an toàn.**
- *Bằng chứng:* `release.yml`:
  - Không chạy lại test hay clippy và không `needs:` CI (REL-03).
  - Không kiểm tag == `version` trong `Cargo.toml` (VER-02).
  - Mỗi file một `.sha256`, không có `checksums.txt` gộp và không kiểm số asset (REL-06).
  - Không chạy thử binary đã build, dù chỉ `pkm --version` (TST-01/02).
  - Action chưa ghim theo SHA, và `checkout@v7` ở release lệch `@v5` ở CI (`release.yml:84` và `ci.yml:28`) (REL-10).
  - `gh release create` không có `--prerelease`, nên tag kiểu `v0.1.0-rc.1` sẽ thành "latest" (VER-07).
- *Rule:* REL-03/06 MUST, VER-02 MUST, TST-02 MUST. REL-10 và VER-07 là SHOULD.
- *Sửa:*
  - Job `verify` so `${GITHUB_REF_NAME#v}` với `cargo metadata … .packages[]|select(.name=="pkm").version`.
  - `build` dùng `needs: verify`, và gọi lại CI qua `workflow_call` (hoặc ít nhất `cargo test --locked`).
  - Mỗi đích chạy `./pkm --version` trước khi upload.
  - Job cuối gộp thành `checksums.txt` (sắp xếp), kiểm đủ 4 asset (5 khi có `install.ps1`).
  - Ghim action theo SHA.
  - Thêm `--prerelease` khi tag có `-`.
  - Tham chiếu: `skillstation/.github/workflows/release.yml`, `herdr-go/scripts/generate-checksums.sh`.

**I7. Lỗi đóng gói chỉ lộ ra lúc gắn tag: CI không build release, không build đủ bốn đích.**
- *Bằng chứng:* `ci.yml` chỉ chạy `cargo test` ở chế độ debug trên ba runner. Không build `--release --target` nào, không build `x86_64-apple-darwin` chéo. Lỗi B1 hay lỗi build chéo sẽ chỉ hiện khi đẩy tag.
- *Rule:* REL-04 MUST (build asset trên mọi PR, không upload).
- *Sửa:* tách phần build của `release.yml` thành workflow tái dùng (`workflow_call`). CI gọi nó trên PR (không đăng), release gọi rồi đăng. Đưa logic đặt tên và đóng gói từ YAML ra một script (REL-05).

**I8. Chưa có đường cập nhật và gỡ cài.**
- *Bằng chứng:* không có `update`, `uninstall`. `--remove` chỉ xoá mục `kg` (`install.rs:67-69,400-447`). Index trong `<pkm_dir>/kg` và gói OHS kèm Node trong `<pkm_dir>/ohs` (hàng trăm MB) ở lại mà không có chỗ nào nói. Cập nhật lúc này tức là tải tay rồi đè `.exe`, mà trên Windows thì không đè được khi Claude Desktop đang chạy `pkm.exe`.
- *Rule:* UX-04 MUST, UNI-01/05 MUST, INS-06 (đổi tên exe đang chạy thành `.old-<pid>`).
- *Sửa (theo thứ tự rẻ trước):*
  1. Cho `install.ps1` chạy lại được: phát hiện bản cũ, đổi tên `pkm.exe` đang chạy thành `pkm.exe.old-<pid>`, rồi tự dọn ở lần chạy sau. Như vậy `irm …/install.ps1 | iex` vừa là lệnh cài vừa là lệnh cập nhật.
  2. Thêm `install.ps1 -Uninstall`: xoá binary và in đường dẫn dữ liệu.
  3. Ghi trong README hai dòng: cập nhật và gỡ (gồm `pkm mcp install --remove` và thư mục `%LOCALAPPDATA%\pkm`).
  4. Sau này mới thêm `pkm update` (phỏng theo `mdview/crates/mdview/src/update.rs`).

**I9. Chưa ký mã, và chưa có hướng dẫn vượt SmartScreen/Defender.**
- *Bằng chứng:* PRD mục 12 và 16 đã chấp nhận "chưa ký mã". Nhưng không chỗ nào hướng dẫn người dùng khi Windows chặn file `.exe` tải bằng trình duyệt (Mark-of-the-Web).
- *Rule:* SEC-08 MAY ("nếu không ký thì ghi cách vượt"). Không trái quyết định.
- *Sửa:* `install.ps1` tải bằng `Invoke-WebRequest` và chạy `Unblock-File` sau khi kiểm hash. README thêm mục `<details>` "Windows chặn pkm.exe" để hướng dẫn.

### Nice-to-have

- **N1. Chưa có `[profile.release]`** (`Cargo.toml:1-9`). Thêm `strip = true`, `lto = "thin"`, `codegen-units = 1` để binary nhỏ và nhanh hơn. Rule: RS-03 (MUST trong overlay, REL-12). Mức thấp vì không chặn việc cài.
- **N2. Linux build `-gnu`, không phải musl** (`release.yml:71`, PRD mục 12 đã chọn). Linux không phải đích chính nên giữ được, nhưng cần ghi trần glibc (runner `ubuntu-latest` là 24.04) hoặc build trên image cũ hơn. Rule: REL-01, RS-04.
- **N3. Không có `version --json` hay fingerprint git** (`build.rs` chỉ đưa vào sha đặc tả). Thêm sha commit và thời điểm commit (phỏng theo `herdr-go/build.rs`). Rule: VER-04, CMD-06.
- **N4. Không có biến ghi đè thư mục gốc** (`PKM_DIR`) **và không có `pkm paths --json`.** Chỉ có `PKM_OHS_DIR` (`package.rs:11`). Installer, uninstall và test sẽ cần. Rule: CFG-02/03 MUST, nhưng chỉ cấp khi có installer hoặc uninstall.
- **N5. Trên Linux, index và gói OHS nằm trong `$XDG_CACHE_HOME`** (`platform.rs:15-18,56`). Dữ liệu suy ra được thì chấp nhận được, nhưng gói OHS (cài bằng tay qua `gh`) bị dọn cache là mất. Cân nhắc đưa `ohs/` sang `XDG_DATA_HOME`. `XDG_*` cũng nên chỉ nhận đường dẫn tuyệt đối (CFG-01).
- **N6. `cargo deny check advisories`** trong release (SEC-06, RS-06). Hiện chưa có.
- **N7. Thêm `.gitattributes`** để giữ LF cho các file mẫu sau này, và `docs/release-runbook.md` (REL-13).
- **N8. Lệnh "cài bằng agent"** cho quick start (ONB-13), và bài thử agent-native TST-04 sau khi CLI xong.

## 4. Phần đã làm tốt (giữ nguyên)

- Một nguồn phiên bản (`[workspace.package]`, `version.workspace = true`), toolchain ghim (RS-01, RS-05).
- `rusqlite` có `bundled`. Không dùng OpenSSL, không có async runtime. MCP tự viết nhỏ (Q4).
- Sửa cấu hình Claude Desktop: giữ thứ tự khoá (`preserve_order`), giữ CRLF, bỏ BOM; từ chối file JSON hỏng thay vì ghi đè thành `{}`; ghi nguyên tử với `fsync`, `.bak-<UTC>` và dọn file tạm; chỉ đụng mục `kg` (AGT-03 phần lớn, PRN-03, PRN-05).
- Từ chối ghi khi Claude Desktop đang chạy, phân biệt được Claude Code với Claude Desktop trên Windows (`desktop.rs:36-73`).
- `--dry-run` in kế hoạch và diff, `--check` bắt tay như Claude Desktop (PRN-04).
- Gói OHS kiểm hai lớp sha256, giải nén zip chặn đường dẫn thoát ra ngoài, staging cạnh đích, đổi chỗ có hoàn tác (SEC-01, SEC-05 phần lớn, PRN-05).
- `doctor` có mã check ổn định, bốn trạng thái, mỗi lỗi có việc cần làm. Test cài và doctor đều đi qua `Hooks`, không đụng máy thật (TST-08).
- CI chạy test trên Windows (RS-06).

## 5. Không áp dụng

- `85-service-daemon`: không có daemon (MCP stdio do client khởi chạy).
- `channels/agent-host-plugin`: chưa phát hành dạng plugin (plugin Obsidian là D4, giai đoạn sau).
- Config UX (UX-11..17, CFG-05..10, CFG-13): không có file cấu hình riêng hay bí mật.
- AGT-06/07/08: không cài skill, hook hay khối hướng dẫn. Host duy nhất là Claude Desktop, chưa cần bảng adapter AGT-02.
- SEC-02/03 (cosign, provenance): PRD hoãn ký sang giai đoạn sau.

## 6. Thứ tự đề xuất (tầng 1 trước)

1. B1 `crt-static` và kiểm phụ thuộc DLL trong CI. Rẻ, làm ngay.
2. B2 `install.ps1` (cài, cập nhật, gỡ) + cảnh báo `current_exe` ngoài chỗ chuẩn. Cần release có asset trước nên làm cùng I6.
3. B3 quick start trong README, cùng ba bước với dòng "Next:" của `install.ps1` (ONB-12).
4. I2, I3, I4 trên `doctor` trước khi `cli` đăng ký lệnh, để chốt hợp đồng JSON khi còn chưa ai gọi.
5. I5 compare-and-swap trong `mcp install`.
6. I6, I7 cổng release và build trên PR. I9 hướng dẫn SmartScreen.
7. I1 mốc chuyển OHS sang Release của fork trước 2027-01-31.
8. Phần nice-to-have.

## 7. Câu hỏi về quyết định đã ghi (không đòi đổi)

- **Q-a (ghim artifact OHS, DECISIONS "Gói OHS: ghim artifact…").** Quyết định này hợp lý cho máy của user (có `gh`). Với máy khách sạch thì nó trái UX-07/08, và hết hạn ngày 2027-01-31. Bản `pkm` đầu tiên có chấp nhận rằng khách không có `gh` sẽ chạy thiếu OHS (chỉ `related` không có `hits` bị ảnh hưởng) không? Hay chờ fork có Release rồi mới phát cho khách?
- **Q-b (bề mặt lệnh, PRD mục 10–11).** PRD chọn `pkm mcp install` / `--remove` / `pkm doctor` và cú pháp `pkm VAULT <lệnh>` như Python. Skill chuẩn hoá `init` / `doctor` / `update` / `uninstall`. Có muốn thêm `pkm init` làm bí danh của `pkm mcp install` và `pkm update` / `pkm uninstall` sau khi chuyển hẳn (cùng đợt với Q1/Q2), hay giữ đúng bề mặt Python tới hết giai đoạn so khớp? Đề xuất của audit: giữ PRD lúc này, xét lại ở R14.
- **Q-c (Linux `-gnu`, PRD mục 12).** Giữ `-gnu` (Linux không phải đích khách), hay đổi sang `-musl` để binary chạy được trên mọi distro? `rusqlite` có `bundled` và không có OpenSSL nên đổi không tốn nhiều.

## Câu hỏi chưa giải quyết

- Chưa chạy được bước Verify bằng binary thật vì hook `scout-block` chặn `cargo build` và `target/`. Nếu cần bảng Verify chạy thật thì phải nới `.claude/.ckignore`, hoặc chạy trong worktree khác.
- Chưa xác minh `actions/checkout@v7`, `upload-artifact@v7`, `download-artifact@v8` có tồn tại và chạy đúng. Plan phase 14 ghi nghiệm thu bằng `workflow_dispatch`, nhưng không có bằng chứng đã chạy.
