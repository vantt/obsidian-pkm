# Thử `v0.1.0-rc.3` trên Windows: phase 10 trên bản B, phase 13 trên Windows sạch

Ngày 2026-11-03. Agent Windows (máy user, Windows 11 Pro N, có Python) và Windows Sandbox (Windows 11 Enterprise N 10.0.26100, máy sạch). Việc 5 và 6 trong danh sách tuần chạy song song của lead. Mọi lệnh trên bản B chỉ in số đếm; báo cáo không có tên note, đường dẫn hay nội dung của bản B.

## Kết quả

```text
Status: DONE_WITH_CONCERNS
P10: pass - bản B, rc.3 (535bc55), spec da62ecb: 54 mục, 0 lệch chưa có tên, 0 mục chưa có, mã 0; CLI build-json, stats nay khớp
P13: pass có điều kiện - Windows sạch không Python: cài, mcp install, --check 13 tool, doctor 0 lỗi; search/related báo model_missing tới khi cài VC++ Redistributable
```

## Phase 10: bộ so khớp riêng trên bản B

- Clone `vantt/obsidian-pkm` ở tag `v0.1.0-rc.3` và repo đặc tả ở `da62ecb` (đúng `spec/spec.lock`), cả hai `core.autocrlf=false`. `PKM_BIN` trỏ `pkm.exe` rc.3 do bộ cài đặt; Python là venv có PyYAML, chạy `-X utf8`.
- Lệnh: `parity.py run --private <B> --adapter e11 --packs capture --no-aliases --edge-keys links --body-edges --spec-dir <spec da62ecb>`.
- Kết quả: index 20 bảng khớp (nodes 2.081, edges 4.788, `v3_deviations` 1.888), tool 10 tool khớp (`related` 1 lệch có tên), MCP `initialize`, `tools_list`, `instructions` khớp với lệch có tên của một cửa (1, 118, 2), CLI 19 lệnh khớp. Tổng 54 mục, 0 lệch chưa có tên, 0 mục chưa có, mã 0, 44 giây.
- Hai lệch CLI của lần thử rc.1 (`build-json`, `stats`, đường dẫn vault chưa che trong JSON ở phía Python) đã hết nhờ `04811f4`.
- Lần chạy đầu thiếu `PKM_BIN` thì tầng MCP và CLI báo "chưa có" 24 mục mà mã vẫn 0. Handoff nên ghi `PKM_BIN` là bắt buộc cho phase 10, hoặc `parity.py` trả mã khác 0 khi còn mục chưa có.

## Phase 13: Windows Sandbox, không Python

Script chạy lúc đăng nhập Sandbox, ghi kết quả ra thư mục chia sẻ. Vault thử 2–4 note tự tạo (không phải bản B), cấu hình Claude Desktop giả qua `--config`.

| Bước | Kết quả |
|---|---|
| Máy | Không có `python`, `python3`, `py`; không có `vcruntime140.dll`, `msvcp140.dll` |
| `irm …/v0.1.0-rc.3/install.ps1 \| iex` | Tài khoản của Sandbox là admin nên bộ cài từ chối (đúng thiết kế); với `PKM_ALLOW_ROOT=1`: checksum khớp, cài vào `%LOCALAPPDATA%\pkm\bin`, thêm PATH, 8 giây |
| `pkm mcp install VAULT --config …` | Mã 0, 26 giây: tải gói OHS `vantt.4` win32-x64, sha256 khớp ghim, dựng index, ghi mục `pkm`, bắt tay 13 tool |
| `pkm mcp install --check` | 13 tool: find, search, note, read, neighbors, reach, paths, loops, provenance, select, valid, related, more |
| `pkm doctor --config …` | 0 lỗi, 1 cảnh báo (`long_paths`: LongPathsEnabled tắt, đúng với máy sạch); `ohs_package` báo `0.17.2-vantt.4, có gói tiếng Việt`, `ohs_mcp` ok |
| `pkm VAULT stats` | Mã 0 |
| Làm ấm và `pkm VAULT related "thói quen"` | **`ohs_not_ready`, `state: model_missing`**, câu của OHS: "`@huggingface/transformers` is not installed (optional dependency missing)" |

### Nguyên nhân của `model_missing`

Lần chạy thứ hai trong Sandbox mới:

1. Trong `%LOCALAPPDATA%\pkm\ohs\app`, `node -e "require('onnxruntime-node')"` báo `ERR_DLOPEN_FAILED` ("The specified module could not be found") ở `onnxruntime-node\bin\napi-v6\win32\x64\onnxruntime_binding.node`. `import('@huggingface/transformers')` hỏng cùng lỗi. Thư viện có đủ trong gói; thứ thiếu là Microsoft Visual C++ Redistributable (x64) mà `onnxruntime` cần.
2. Cài `vc_redist.x64.exe /install /quiet` (mã 0), `vcruntime140.dll` có mặt: `require('onnxruntime-node')` qua; `pkm VAULT related "thói quen"` mã 0 sau 21 giây (OHS tải model, index), hạt giống đúng note.

Máy user không gặp vì đã có VC++ Redistributable. Câu "transformers is not installed … reinstall without --no-optional … npm install -g" của OHS sai hướng người dùng: gói có đủ thư viện, và người dùng `pkm` không có npm.

### Đề xuất sửa

1. **Fork OHS (`vantt/obsidian-hybrid-search`), gói win32-x64:** chép runtime VC++ x64 (app-local, được phép theo giấy phép redistributable) cạnh `onnxruntime_binding.node`; smoke của gói chạy trên máy không có runtime (Windows Sandbox hoặc runner xoá runtime). Và: lỗi `dlopen` của `onnxruntime-node` không báo thành "not installed".
2. **`pkm` (repo này):** trên Windows, `doctor` thêm mục kiểm `vcruntime140.dll`, `msvcp140.dll` (System32 hoặc cạnh `onnxruntime_binding.node`), báo `warn` kèm lệnh `winget install --id Microsoft.VCRedist.2015+.x64`; `mcp install` in cùng câu khi làm ấm gặp `model_missing`. `action` của `ohs_not_ready` khi `state: model_missing` nên nêu runtime VC++ trước khi chép câu của OHS.
3. Tạm thời: README có mục "Windows mới cài: search báo `model_missing`" (cùng nhánh với báo cáo này).

## Ghi nhận khác

- `serverInfo.version` của MCP là `0.1.0` trong khi `pkm --version` là `0.1.0-rc.3`. Không chặn gì; ghi để lead xem có cố ý không.
- Cùng nhánh: README và lời dẫn trong `scripts/install.ps1`, `scripts/install.sh` bỏ phần "repo còn private, cần `gh`" (repo public từ 2026-11-03); dùng `irm … | iex` và `curl … | sh` với link ghi tag cho pre-release. Đường lùi `gh` cho fork private giữ nguyên.

## Bản B sau rc.3 (phía repo đặc tả)

- Chat trong Obsidian của bản B: hai MCP `kg` (Python qua WSL) và `ohs` đã thay bằng một mục `pkm` (`setup_vault.py` bước agent, commit `04eb84f` của repo đặc tả); `claude mcp list` báo `pkm` Connected; thử qua stdio: 13 tool, `search` 5 kết quả 2,5 giây, `related` theo câu hỏi 10 hạt giống, `read` 3 note 0 lỗi. `VAULT/.claude/CLAUDE.md` chép lại từ mẫu mới (bản cũ chưa bị sửa tay).
- Claude Desktop: `pkm.exe` đã lên rc.3; mục cấu hình vẫn tên `kg` tới khi user thoát Claude Desktop và chạy lại `pkm mcp install` (agent chạy trong Claude Desktop nên không tự làm được). Dry-run: đổi tên `kg` thành `pkm`, giữ mọi cờ và `--ohs C:\tools\ohs`, bỏ `env` (pkm tự đặt mẫu bỏ qua).
