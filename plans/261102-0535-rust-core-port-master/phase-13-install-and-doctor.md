---
title: "Phase 13: pkm mcp install và pkm doctor"
status: in-progress
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
- [x] Tìm file cấu hình Claude Desktop: `%APPDATA%\Claude\claude_desktop_config.json`, bản Microsoft Store, macOS `~/Library/Application Support/Claude/`.
- [x] Dựng index lần đầu vào thư mục dữ liệu của `pkm` (ngoài vault, dùng `platform`). Bộ cài gọi `Hooks::build_index`, mặc định chạy `pkm VAULT build --db <thư mục pkm>/kg/<vault>-<hash>.db <cờ đọc>`; CLI đã có lệnh `build`.
- [x] Ghi mục `kg`: `command` là đường dẫn đủ của `pkm`, `args` là vault và cờ đọc của vault đó. Giữ nguyên các mục khác trong file cấu hình (gồm MCP `ohs`).
- [x] `--check` (bắt tay lại như Claude Desktop), `--dry-run`, `--remove`. Từ chối ghi khi Claude Desktop đang chạy, báo rõ.
- [x] `pkm doctor`: kiểm DB, gói OHS, cấu hình Claude Desktop, đường dẫn dài trên Windows; mỗi lỗi có việc cần làm.

## Trạng thái (2026-11-02)
Thư viện `install` và `doctor` xong, có test (`cargo test -p pkm`); gói OHS cài từ Release ghim của fork (https, hiện `v0.17.2-vantt.4`, nâng gói cũ) hoặc `--ohs-package`; OHS chạy với gói tiếng Việt của gói (DECISIONS, mục "Gói OHS: ghim GitHub Release của fork"). Sau đó `cli` đã đăng ký `pkm mcp install`, `pkm doctor` và có lệnh `build`; chỉ còn thử trên Windows sạch. Báo cáo: `plans/reports/fullstack-developer-261102-1440-phase-13-install-doctor.md`.

## Nghiệm thu
- Test đơn vị: ghi cấu hình vào file tạm giữ nguyên mục khác; `--dry-run` không ghi; `--remove` chỉ xoá mục `kg`.
- User chạy `pkm mcp install` trên Windows sạch (không có Python), mở Claude Desktop và gọi được tool `kg`.
- `pkm doctor` trên máy đó không báo lỗi.

## Cần user
Thử trên Windows sạch.

Cổng còn chờ: mã đã xong (`cli` đã đăng ký `pkm mcp install`, `pkm doctor` và có lệnh `build`). Trên Windows thật ([báo cáo Windows](../reports/windows-tester-261102-1816-windows-install.md)) bộ cài, `mcp install` (bốn phép kiểm), `--check` 11 tool, `doctor` `ok`, Claude Desktop 7/7 câu và bản B đều qua; nhưng máy có sẵn Python, nên "Windows sạch" mới đạt một phần. Còn thử trên một máy Windows không có Python.
