---
title: "Phase 17: Một cửa"
status: todo
priority: P2
effort: "1,5 ngày"
prd: R14
wave: W8
dependencies: [16]
needs_user: true
---

# Phase 17: Một cửa (R14, Q1)

## Mục tiêu
Agent chỉ cần nói với `pkm`: thêm tool `search`, `read` chuyển tiếp sang OHS qua stdio, `related` nhận câu hỏi làm đường chính, bộ cài thôi đăng ký MCP `ohs`. Số lượt gọi tool và token mỗi câu không tăng so với hai cửa.

## Đọc trước
- `docs/PRD.md` mục 3.1 (bước 2, 3), 8 (đoạn cuối), 9, R14, Q2.
- Fork OHS: tên tham số của `search`, `read` ở giao thức stdio; `read` cần bản vá P04 của fork (kiểm fork đã có chưa, chưa có thì báo `BLOCKED`).

## Quyết định cần user trước khi làm
- Q2: đã chốt (2026-11-02): đổi thành `pkm` ở đợt này, xem `docs/DECISIONS.md`.
- Cho phép sửa `CLAUDE.md` mẫu của vault và hướng dẫn cài ở repo đặc tả.

## File được sửa
- `crates/pkm/src/mcp*`, `crates/pkm/src/install*`, `crates/pkm-core/src/ohs*`, `crates/pkm-core/src/related*`.
- Tạo `docs/MCP-CHANGELOG.md`.
- Từ đây bộ so khớp tầng MCP ghi các tool mới là lệch có chủ ý so với Python.

## Thiết kế đã chốt (user, 2026-11-02)
Chữ ký tool chốt sau hai báo cáo thiết kế (brainstormer, kongming); chi tiết ở `docs/MCP-CHANGELOG.md`, lý do ở `docs/DECISIONS.md`.
- 13 tool theo thứ tự `find`, `search`, `note`, `read`, `neighbors`, `reach`, `paths`, `loops`, `provenance`, `select`, `valid`, `related`, `more`; mười tool của Python giữ nguyên chữ.
- `search`: tham số chọn lọc theo tên MCP của OHS (`query`, `queries`, `path`, `mode`, `scope`, `tag`, `limit`, `snippet_length`, `system`), kết quả `{count, mode, items: [{id, title, type, score, snippet}]}`.
- `read`: **đọc thẳng từ vault, không qua OHS** (lệch câu "chuyển tiếp mỏng" ở dưới và PRD 3.1).
- `related`: `query` | `queries` | `node` | `hits` (ưu tiên `hits` > `node` > câu hỏi), `snippet` cho hạt giống, `seed_source` = `query` | `node` | `hits`.
- Bộ cài làm ấm index tìm kiếm khi cài (`--no-warm` để bỏ); CLI chỉ thêm `read`.

## Việc cần làm
- [x] Tool `search`, `read`: chuyển tiếp mỏng, giữ tên tham số của OHS. Đã làm theo thiết kế đã chốt: `search` chuyển tiếp với tham số chọn lọc (tên MCP của OHS, tự đổi sang stdio); `read` đọc thẳng từ vault.
- [x] `related` nhận câu hỏi làm đường chính (gộp tìm và đi graph trong một lần gọi); vẫn nhận `hits`.
- [x] `instructions` hướng dẫn agent chỉ dùng `pkm`.
- [x] `pkm mcp install` thôi đăng ký MCP `ohs`; không xoá MCP `ohs` user tự thêm, không cấm dùng song song. Mục `kg` cũ do pkm ghi đổi tên thành `pkm`; `pkm doctor` cảnh báo khi còn MCP `ohs` riêng.
- [x] Ghi mọi thay đổi hợp đồng vào `docs/MCP-CHANGELOG.md`.
- [ ] Sau khi user cho phép: cập nhật `CLAUDE.md` mẫu của vault và hướng dẫn cài ở repo đặc tả.
- [ ] Đo: chạy nhóm A–D playbook với hai cửa và với một cửa, ghi số lượt gọi tool và token mỗi câu. Cần dùng thật (Claude Desktop hoặc `claude -p` với `pkm VAULT mcp --log FILE.jsonl` trên bản B); chưa làm.

## Điều kiện xem lại code mode
Đợt này không làm code mode (docs/DECISIONS.md, "Một cửa: chưa làm code mode"). Nếu bảng số đo ở việc "Đo" cho thấy từ 2 câu trở lên vượt giới hạn số lượt gọi tool (H4: nhóm B dưới 3 lượt, nhóm C dưới 5; thường là B4, C4, D5), mở một phase riêng cho tool `run` dạng hybrid (giữ 13 tool, thêm `run`); trước đó xét tham số hay tool nhỏ cho đúng phép còn thiếu.

## Nghiệm thu
- Agent làm đủ nhóm A–D playbook chỉ với `pkm`.
- Bảng số đo: lượt gọi tool và token mỗi câu của một cửa không cao hơn hai cửa.
- `docs/MCP-CHANGELOG.md` có mục cho mỗi tool mới và mỗi thay đổi `related`.
