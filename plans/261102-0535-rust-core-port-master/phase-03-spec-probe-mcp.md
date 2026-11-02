---
title: "Phase 3: Bài thử S2, MCP"
status: done
priority: P1
effort: "0,5 ngày"
prd: R02
wave: W1
dependencies: [1]
needs_user: true
---

# Phase 3: Bài thử S2, MCP Rust bắt tay với Claude (R02)

## Mục tiêu
Một MCP server Rust nhỏ, một tool, bắt tay được với Claude Desktop trên Windows và với Claude Code. Chốt Q4: crate `rmcp` hay JSON-RPC stdio tự viết.

## Đọc trước
- `docs/PRD.md` mục 3 (đoạn crate gợi ý), 8.
- Repo đặc tả ở sha ghim: `scripts/kg/mcp_server.py` (khai báo tool, `instructions`, phiên bản giao thức).

## File được sửa
- Tạo: `probes/s2-mcp/` (crate riêng, không là member của workspace), báo cáo `plans/reports/probe-s2-mcp-*.md`.
- Thêm mục Q4 vào `docs/DECISIONS.md`.

## Việc cần làm
- [x] Viết server bằng `rmcp`. Nếu `rmcp` kéo async runtime nặng hoặc không cho kiểm soát đủ câu chữ `instructions`, mô tả tool, schema tham số, viết thêm bản JSON-RPC tự viết để so.
- [x] Bắt tay với Claude Code: `initialize`, `tools/list`, gọi tool.
- [x] Ghi phiên bản giao thức Python đang trả và làm bản Rust trả cùng giá trị.
- [x] Đưa user đoạn JSON cấu hình để thêm server thử vào Claude Desktop trên Windows; chờ user báo kết quả.

## Nghiệm thu
- Claude Code gọi được tool của server thử (bản ghi phiên trong báo cáo).
- User xác nhận Claude Desktop trên Windows gọi được tool.
- `docs/DECISIONS.md` có Q4 kèm lý do (kích thước binary, phụ thuộc async, kiểm soát câu chữ).

## Cần user
Chạy thử trên Claude Desktop Windows.

## Tình trạng (2026-11-02)
Xong phần máy: hai server thử ở `probes/s2-mcp/`, Claude Code 2.1.316 gọi được tool của cả hai, Q4 đã chốt (JSON-RPC tự viết). Còn chờ user chạy thử Claude Desktop trên Windows theo hướng dẫn ở [báo cáo](../reports/probe-s2-mcp-261102-1317.md#6-việc-của-user-claude-desktop-trên-windows).

Cổng đã qua (2026-11-02, [báo cáo Windows](../reports/windows-tester-261102-1816-windows-install.md)): Claude Desktop Store 4.4212 trên Windows 11 26200 gọi được tool của `kg` (chính `pkm`), `find` trả đúng id có dấu; không cần chạy lại hai server thử.
