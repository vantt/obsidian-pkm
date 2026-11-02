---
title: "Phase 13: pkm mcp install và pkm doctor"
status: todo
priority: P1
effort: "1 ngày"
prd: R11
wave: W6
dependencies: [12, 7]
needs_user: true
---

# Phase 13: `pkm mcp install` và `pkm doctor` (R11, trừ `platform` đã làm ở phase 07)

## Mục tiêu
Thay `desktop_config.py`: cài MCP `kg` trỏ vào binary `pkm` cho Claude Desktop, không cần Python; `pkm doctor` chẩn đoán máy khách. Sau phase này lead chạy điểm đuổi theo C2 (phase 15).

## Đọc trước
- `docs/PRD.md` mục 10 (đoạn "Thêm"), 11, Q2 (giữ tên `kg`).
- Repo đặc tả ở sha ghim: `scripts/kg/desktop_config.py`.

## File được sửa
- `crates/pkm/src/{install,doctor}*` và test; thêm lệnh con vào `cli` (chỉ đăng ký lệnh).

## Việc cần làm
- [ ] Tìm file cấu hình Claude Desktop: `%APPDATA%\Claude\claude_desktop_config.json`, bản Microsoft Store, macOS `~/Library/Application Support/Claude/`.
- [ ] Dựng index lần đầu vào thư mục dữ liệu của `pkm` (ngoài vault, dùng `platform`).
- [ ] Ghi mục `kg`: `command` là đường dẫn đủ của `pkm`, `args` là vault và cờ đọc của vault đó. Giữ nguyên các mục khác trong file cấu hình (gồm MCP `ohs`).
- [ ] `--check` (bắt tay lại như Claude Desktop), `--dry-run`, `--remove`. Từ chối ghi khi Claude Desktop đang chạy, báo rõ.
- [ ] `pkm doctor`: kiểm DB, gói OHS, cấu hình Claude Desktop, đường dẫn dài trên Windows; mỗi lỗi có việc cần làm.

## Nghiệm thu
- Test đơn vị: ghi cấu hình vào file tạm giữ nguyên mục khác; `--dry-run` không ghi; `--remove` chỉ xoá mục `kg`.
- User chạy `pkm mcp install` trên Windows sạch (không có Python), mở Claude Desktop và gọi được tool `kg`.
- `pkm doctor` trên máy đó không báo lỗi.

## Cần user
Thử trên Windows sạch.
