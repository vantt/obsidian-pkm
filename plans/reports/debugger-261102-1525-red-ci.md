# CI đỏ trên main: nguyên nhân và cách sửa

Ngày: 2026-11-02 (Asia/Saigon). Người làm: debugger. Các commit nằm trên `main` ở máy, chưa push.

## Kết luận

CI đỏ vì ba lỗi riêng biệt, mỗi lỗi đều đã chứng minh và đã sửa:

| # | Triệu chứng | Nguyên nhân gốc | Commit sửa |
|---|---|---|---|
| 1 | Bước parity đỏ trên cả 3 OS: `rust-dump có file nhưng không phải dump (thiếu manifest.json)` | `Swatinem/rust-cache` lưu `target/` nhưng xoá mọi *file* nằm ngoài thư mục profile. `target/parity/rust-dump` được khôi phục dưới dạng khung thư mục rỗng (`tables/`, `cli/`, `mcp/`) không có `manifest.json`, nên `Writer::create` từ chối ghi đè. | `cc8af51` ci: write the parity dump outside the cached target dir |
| 2 | Windows: `mcp::tests::handshake` và `tools_list_comes_from_the_tool_layer` | `instructions::basename` chỉ cắt ở `/`. Trên Windows đường dẫn dùng `\`, nên tên vault thành cả đường dẫn tuyệt đối `C:\Users\...\Vault thử`. Python dùng `ntpath.basename`, tách ở cả hai dấu và bỏ ổ đĩa. Đây là lỗi thật của sản phẩm (hướng dẫn MCP và link `obsidian://` sai trên Windows), không phải lỗi test. | `c7bf21e` fix(pkm): take the vault name from Windows paths like ntpath |
| 3 | Job `asset x86_64-unknown-linux-gnu`, bước `shellcheck các script` | `scripts/install.sh:77` dùng `A && B \|\| C`; shellcheck 0.9.0 trên ubuntu-24.04 báo SC2015 và trả mã 1. | `2f83462` fix(install): make the missing option value check a plain if |

Lead cần push `cc8af51`, `c7bf21e`, `2f83462` (cả ba, trên `0054429`) để CI chạy lại.

## Dòng thời gian

- `8f64929` (Merge vault and schema modules, run 41406769089): lần đầu bản Rust ghi dump vào thư mục con (`tables/*.jsonl`). Run xanh vì cache cũ (key `...-bda02514`) chưa có `target/parity`. Bước Post của rust-cache lưu cache mới key `v0-rust-check-Linux-x64-ff458983-64e06c25`, sau khi đã xoá file trong `target/parity/rust-dump/**`.
- `2661aa0` (run 41408080051): `Cache hit ... 64e06c25 full match: true`, khôi phục khung rỗng, bước parity đỏ trên cả 3 OS. Commit này chỉ sửa `vault.rs`/`yaml.rs`, không đụng parity; nó chỉ là run đầu tiên dùng cache bị hỏng.
- Mọi run sau (41409203491, 41409885877, 41410663494, 41412319781, 41412927033, release 41412937710) đỏ cùng lý do vì Cargo.lock không đổi, cache vẫn full match.
- `e9abb8a` (Merge MCP server and CLI shell, run 41412319781): thêm test MCP; Windows đỏ thêm ở `cargo test`.
- `77b82de`/`e0a4787` (install.sh, release-assets.yml): job asset Linux đỏ ở shellcheck (run 41412927033, 41412937710).

## Bằng chứng

### 1. Parity
- Mã rust-cache (`src/cleanup.ts`, `cleanTargetDir`): thư mục không có `build`, `.fingerprint`, `deps` thì được đệ quy và mọi file trừ `CACHEDIR.TAG` bị xoá. Thư mục rỗng được giữ.
- Dump ở máy có `target/parity/rust-dump/{manifest.json, perf.json, tables/, cli/, mcp/}`.
- Tái hiện ở máy: tạo `dump/{tables,cli,mcp}` rỗng rồi chạy `check --dump-out dump` cho ra đúng thông báo lỗi của CI, exit 2.
- Vì sao máy lead xanh: `target/` ở máy không bao giờ bị rust-cache dọn, `manifest.json` vẫn còn.

Cách sửa: CI truyền `--dump-out "$RUNNER_TEMP/rust-dump"`. `RUNNER_TEMP` mới cho mỗi job và nằm ngoài cache. Kiểm an toàn "không xoá thư mục lạ" của `Writer::create` vẫn giữ nguyên. Cache cũ còn khung rỗng nhưng không ảnh hưởng nữa.

### 2. Windows basename
Log run 41412319781:
```
left: Some("C:\\Users\\RUNNER~1\\AppData\\Local\\Temp\\.tmpo5jyw3\\Vault thử")
right: Some("Vault thử")
```
Hướng dẫn MCP ghi `vault "C:\Users\...\Vault thử"` và link `obsidian://open?vault=C%3A%5C...`.
Python (`scripts/kg/mcp_server.py:89,93,200` ở sha ghim): `self.vault = os.path.abspath(...)`, rồi `os.path.basename(self.vault.rstrip('/'))`. Trên Windows `os.path` là `ntpath`. Đã kiểm `ntpath.basename`: `C:\Users\a\Vault thử` cho `Vault thử`, `C:Vault` cho `Vault`, `C:\` cho chuỗi rỗng.
Cách sửa: trên Windows lấy `Path::file_name()` (tách ở cả `\` và `/`, bỏ prefix ổ đĩa), trên Unix giữ cách cũ. Thêm test `#[cfg(windows)] basename_splits_windows_paths_like_ntpath`. Cùng lỗi này cũng làm `tests/mcp_stdio.rs` (tên `kg-test`) đỏ trên Windows, nhưng test đó chưa từng chạy vì `cargo test` dừng ở binary đầu tiên bị lỗi.

### 3. shellcheck
Log job 125170080604: `In scripts/install.sh line 77 ... SC2015 (info)`. Tái hiện bằng shellcheck 0.9.0 (bản của ubuntu-24.04), còn 0.11.0 không báo. Sau khi sửa, cả 0.9.0 và 0.11.0 đều sạch; `install.sh --version` và `--version ""` vẫn báo `needs a value`, exit 2.

## Các giả thuyết đã loại
- Commit `2661aa0` làm hỏng dump: loại, vì commit không đụng `parity/` và lỗi xảy ra ở `Writer::create` trước khi chạy bản Rust.
- Một test trong `cargo test` ghi vào `target/parity/rust-dump`: loại, vì không test nào dùng đường mặc định (golden.rs dùng tempdir), và tái hiện chỉ cần khung rỗng.
- Windows hỏng do CRLF hoặc khác biệt khi spawn process: loại, vì giá trị lệch là đường dẫn tuyệt đối thay vì tên thư mục; 78 test khác trong cùng binary đều qua.
- Nhãn runner `macos-15` / `windows-2025` không tồn tại: loại, vì cả bốn job asset đều chạy được; macOS và Windows xanh.
- `install.ps1` lỗi: loại, vì job `asset x86_64-pc-windows-msvc` (PSScriptAnalyzer và vòng đời install.ps1) xanh ở cả run 41412927033 và 41412937710.

## Gate đã chạy ở máy (Linux)
- `cargo fmt --all --check`: sạch.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: sạch. Clippy cho đích Windows không chạy được ở máy vì thiếu `lib.exe` cho cc-rs.
- `cargo test --workspace --locked`: tất cả qua, 0 failed.
- Parity như CI (kg-solo copy mới, `--dump-out` mới): exit 0, `0 lệch chưa có tên, 34 mục chưa có`.
- Vòng đời `install.sh` như job asset Linux (`release-assets.sh package` rồi `test-installer.sh`): `OK: install.sh qua mọi bước`. Bước này chưa từng chạy trên CI vì shellcheck đỏ trước nó.

## Phòng tái diễn
- Không để artifact của CI trong `target/` khi dùng rust-cache. Nếu cần giữ mặc định `target/parity/rust-dump` cho dev, có thể ghi chú điều này trong `parity/src/main.rs`.
- Đề xuất (chưa làm, cần lead quyết): thêm `--no-fail-fast` vào `cargo test` trong ci.yml. Hiện binary `pkm` đỏ trên Windows che mất 10 binary test còn lại (`cli_commands`, `mcp_stdio`, `version`, `pkm_core`, `ohs_*`, `yaml_cases`, `parity`, `golden`). Ba binary `cli_commands`, `mcp_stdio`, `version` chưa từng chạy trên Windows.
- Đề xuất: ghim phiên bản shellcheck (hoặc chạy shellcheck ở máy bằng cùng bản 0.9.0) để kết quả ở máy và trên CI giống nhau.

## Câu hỏi còn mở
- Windows: `tests/cli_commands.rs`, `tests/mcp_stdio.rs`, `tests/version.rs` chưa từng chạy trên Windows. Chỉ biết chắc sau khi push. Lỗi basename trong `mcp_stdio` đã được sửa; các test còn lại (ví dụ so khớp chuỗi đường dẫn db trong `build_stats_validate_like_python_cli`, `doctor`, `mcp install`) có thể lộ lỗi mới. Phần `install`/`doctor` thuộc agent khác.
- Với vault là gốc UNC (`\\srv\share`), `ntpath.basename` cho chuỗi rỗng và `Path::file_name` cũng cho `None`, nên vẫn khớp. Chưa thử trên máy Windows thật.
