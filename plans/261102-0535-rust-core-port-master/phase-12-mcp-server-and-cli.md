---
title: "Phase 12: MCP server và CLI"
status: in-progress
priority: P1
effort: "2 ngày"
prd: R10
wave: W5
dependencies: [10, 3]
needs_user: false
---

# Phase 12: `pkm::mcp` và `pkm::cli` (R10)

## Mục tiêu
MCP stdio thay được MCP `kg` Python: bắt tay khớp từng chữ, refresh nền, log, giới hạn ký tự; CLI giống `scripts/kg/cli.py`. 13 test MCP Python chuyển sang đều qua. Chạy song song với phase 11.

## Đọc trước
- `docs/PRD.md` mục 3.1 (hai cửa), 6 (tầng MCP, CLI), 7 (dòng `mcp`), 8, 10.
- Báo cáo phase 03 và mục Q4 trong `docs/DECISIONS.md`.
- Repo đặc tả ở sha ghim: `scripts/kg/mcp_server.py`, `scripts/kg/cli.py`, `scripts/kg/__main__.py`, `scripts/kg/tests/test_mcp.py`, `docs/knowledge-graph/install-guide-kg-mcp.md`.

## File được sửa
- `crates/pkm/src/{main,cli,mcp}*` và test; `crates/pkm/Cargo.toml`.
- Phần cắm Rust của bộ so khớp cho tầng MCP và CLI.

## Việc cần làm
- [ ] MCP theo Q4: tên server, tên tool, mô tả, schema tham số, dạng kết quả giống Python từng chữ (11 tool, gồm `related` qua điểm vào chung của `tools`).
- [ ] Cờ khởi động đủ như mục 8 PRD; cờ OHS đổi nghĩa thành đường dẫn gói OHS.
- [x] `instructions` ghép như Python: mô tả vault (`--about`), mô tả từng loại trong schema, số note và quan hệ, đoạn của adapter, khối cố định.
- [x] Refresh nền mặc định 5 s bằng `std::thread`; log JSONL (`--log`); giới hạn ký tự (`--max-chars`).
- [ ] CLI bằng `clap`: `build [--full]`, `stats`, `validate`, `find`, `note`, `neighbors`, `reach`, `paths`, `loops`, `provenance`, `select`, `valid`, `mcp`, cùng tuỳ chọn chung của mục 10.
- [x] `pkm --version` in phiên bản và sha đặc tả đã khớp.
- [ ] Chuyển 13 test của `test_mcp.py`.

## Trạng thái (2026-11-02)

Xong phần không cần tầng tool: giao thức MCP, `instructions`, dựng nền và refresh, thu gọn, cắt, log; CLI `build`, `stats`, `validate`, `pkm mcp install`, `pkm doctor`. Bộ so khớp trên kg-solo: `initialize`, `instructions` và 8 lệnh CLI giống golden từng byte. Còn: nối `pkm_core::tools` vào `installed()` trong `crates/pkm/src/mcp/tool_layer.rs` (danh mục tool, `Server.call`, `Tools.<method>` cho CLI), rồi `tools/list`, 13 lệnh CLI hỏi graph và 9 test `test_mcp.py` còn lại (`spec/PORTING.md`). Cờ: có đủ cờ của `parser()` Python; `--limit`, `--system` mà PRD mục 8 kể không có trong parser MCP của Python nên chưa thêm (chờ lead quyết); `--ohs-cli`, `--ohs-node` chuyển nguyên cho tầng tool. Báo cáo: `plans/reports/fullstack-developer-261102-1500-phase-12-cli-mcp.md`.

## Nghiệm thu
- Bộ so khớp tầng MCP (`initialize`, `tools/list`, `instructions`) và tầng CLI trên kg-solo: 0 lệch chưa có tên (CI).
- 13 test MCP đã chuyển qua; `spec/PORTING.md` có bảng 13 dòng.
- Claude Code dùng `pkm` thay MCP `kg` gọi được mọi tool trên kg-solo (bản ghi phiên trong báo cáo).
- Tool `related` qua MCP chỉ nghiệm thu sau khi phase 11 đã merge; trước đó lead nghiệm thu 10 tool còn lại.
