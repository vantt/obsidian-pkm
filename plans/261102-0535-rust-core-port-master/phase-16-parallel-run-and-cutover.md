---
title: "Phase 16: Chạy song song một tuần và chuyển hẳn"
status: todo
priority: P1
effort: "0,5 ngày + 1 tuần dùng thật"
prd: R13
wave: W7
dependencies: [13, 14, 15]
needs_user: true
---

# Phase 16: Chạy song song và chuyển hẳn (R13)

## Mục tiêu
Đủ bốn điều kiện chuyển hẳn ở mục 13 PRD, rồi bỏ bản Python cho lõi. Phase này phần lớn là việc của user; lead tự làm, không giao agent con, vì cần phối hợp và quyết định.

## Đọc trước
- `docs/PRD.md` mục 5.7 (quy tắc hội tụ), 13.
- Repo đặc tả: `docs/capture-to-producer/playbook/07-kg-challenges.md`.

## Việc của lead
- [ ] Kiểm điều kiện 1: bộ so khớp sạch trên kg-solo (CI) và bản B (máy user) ở sha ghim mới nhất.
- [ ] Kiểm điều kiện 2: `follow` báo 0 (phase 15, điểm C3).
- [ ] Tạo bản phát hành thử qua `release.yml` (cần user đồng ý tag), hướng dẫn user cài bằng `pkm mcp install`.
- [ ] Ghi điểm của nhóm A–C, F playbook khi dùng Python (nếu chưa có) để làm mốc so.
- [ ] Sau một tuần: thu điểm khi dùng `pkm`, so với mốc. Điều kiện 3 đạt khi không thấp hơn.
- [ ] Hỏi user đồng ý (điều kiện 4).
- [ ] Sau khi user đồng ý và cho phép sửa repo đặc tả: `setup_vault.py` và hướng dẫn cài trỏ sang `pkm`; ghi mục GATES về việc `scripts/kg` đóng băng (chỉ sửa lỗi) và quy tắc hội tụ (mục 5.7).
- [ ] Cập nhật `README.md` repo này: trạng thái, cách cài.

## Việc của user
- Dùng Claude Desktop và chat trong Obsidian trỏ vào `pkm` một tuần.
- Chạy lại nhóm A–C, F playbook; báo điểm.
- Quyết định chuyển hẳn.

## Nghiệm thu
- Bốn điều kiện mục 13 PRD đều có bằng chứng ghi trong báo cáo `plans/reports/cutover-*.md` (link CI, số lệch bản B, đầu ra `follow`, bảng điểm hai bên, câu đồng ý của user kèm ngày).
