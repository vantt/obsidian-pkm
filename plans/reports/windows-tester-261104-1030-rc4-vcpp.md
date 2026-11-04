# Thử `v0.1.0-rc.4` trên Windows sạch: cảnh báo VC++ Redistributable

Ngày 2026-11-04. Windows Sandbox (máy sạch, không Python, không VC++ runtime), cùng cách với phase 13 của rc.3 ([báo cáo rc.3](windows-tester-261103-1430-rc3-windows.md)). Vault thử hai note tự tạo, cấu hình Claude Desktop giả qua `--config`. Không có dữ liệu của bản B.

## Kết quả

```text
Status: DONE
1. Trước VC++: doctor báo ohs_runtime warn kèm lệnh winget - pass
2. Trước VC++: lỗi của related nêu DLL thiếu trước câu của OHS - pass
3. Sau vc_redist.x64.exe: ohs_runtime ok, related chạy - pass
```

| Bước | Kết quả |
|---|---|
| `irm …/v0.1.0-rc.4/install.ps1 \| iex`, `PKM_ALLOW_ROOT=1` | checksum khớp, `pkm 0.1.0-rc.4 (spec da62ecb)`, 9 giây |
| `pkm mcp install VAULT --config …` | Mã 0, 33 giây: tải gói OHS `0.17.2-vantt.4` win32-x64, sha256 khớp ghim, dựng index, bắt tay 13 tool. Bước làm ấm báo `model_missing` và in câu mới: thiếu `vcruntime140.dll`, `vcruntime140_1.dll`, `msvcp140.dll`, kèm lệnh `winget` |
| Máy trước VC++ | `vcruntime140.dll`, `msvcp140.dll` không có trong System32 |
| `pkm doctor --config …` trước VC++ | 0 lỗi, 2 cảnh báo: `[warn] ohs_runtime` nêu ba DLL thiếu, "`search` và `related` theo câu hỏi sẽ báo model_missing, các tool graph vẫn chạy", lệnh `winget install --id Microsoft.VCRedist.2015+.x64`; `[warn] long_paths` như rc.3 |
| `pkm VAULT related "thói quen"` trước VC++ | Mã 1, `ohs_not_ready`, `state: model_missing`; `action` mở đầu bằng "Windows thiếu Microsoft Visual C++ Redistributable (x64) (vcruntime140.dll, vcruntime140_1.dll, msvcp140.dll)…", lệnh `winget`, rồi mới tới "OHS báo: [embedder] @huggingface/transformers is not installed…" |
| `vc_redist.x64.exe /install /quiet /norestart` | Mã 0, `vcruntime140.dll` có mặt |
| `pkm doctor --config …` sau VC++ | 0 lỗi, 1 cảnh báo (`long_paths`); `[ok] ohs_runtime có Visual C++ Redistributable (x64) cho tìm theo nghĩa` |
| `pkm VAULT related "thói quen"` sau VC++ | Mã 0 sau 21 giây (OHS tải model, index), 2 hạt giống `found_by: ohs`, note đúng xếp đầu |

## Ghi nhận khác

- **Link trong thân note không thành cạnh khi chạy mặc định.** Một note thử có `[[Thói quen buổi sáng]]` trong thân, nhưng `mcp install` mặc định báo `cạnh theo lớp: {}` và `degree` của cả hai note là 0. Bản B không gặp vì chạy với `--body-edges`. Với vault Obsidian thường (link nằm trong thân, không trong frontmatter), graph mặc định sẽ rỗng; lead xem có nên bật `--body-edges` mặc định cho `mcp install`, hoặc `doctor` cảnh báo khi index có note mà 0 cạnh.
- Windows Sandbox có lần không khởi động được ("failed to initialize", `wsb list` hết giờ ở `localhost:80`) sau khi dừng cứng tiến trình của nó; `Restart-Service CmService` (quyền admin) gỡ được. Không liên quan tới `pkm`; ghi để người thử sau đỡ mất thời gian.
- Phía fork OHS vẫn còn việc: chép runtime VC++ vào gói win32-x64 để người dùng không phải cài.
