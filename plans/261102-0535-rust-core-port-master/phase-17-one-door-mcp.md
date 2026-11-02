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

## Việc cần làm
- [ ] Tool `search`, `read`: chuyển tiếp mỏng, giữ tên tham số của OHS.
- [ ] `related` nhận câu hỏi làm đường chính (gộp tìm và đi graph trong một lần gọi); vẫn nhận `hits`.
- [ ] `instructions` hướng dẫn agent chỉ dùng `pkm`.
- [ ] `pkm mcp install` thôi đăng ký MCP `ohs`; không xoá MCP `ohs` user tự thêm, không cấm dùng song song.
- [ ] Ghi mọi thay đổi hợp đồng vào `docs/MCP-CHANGELOG.md`.
- [ ] Sau khi user cho phép: cập nhật `CLAUDE.md` mẫu của vault và hướng dẫn cài ở repo đặc tả.
- [ ] Đo: chạy nhóm A–D playbook với hai cửa và với một cửa, ghi số lượt gọi tool và token mỗi câu.

## Nghiệm thu
- Agent làm đủ nhóm A–D playbook chỉ với `pkm`.
- Bảng số đo: lượt gọi tool và token mỗi câu của một cửa không cao hơn hai cửa.
- `docs/MCP-CHANGELOG.md` có mục cho mỗi tool mới và mỗi thay đổi `related`.
