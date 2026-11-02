---
title: "Phase 10: query, tools, more"
status: todo
priority: P1
effort: "2,5 ngày"
prd: R08
wave: W4
dependencies: [9]
needs_user: true
---

# Phase 10: `query`, `tools`, `more` (R08)

## Mục tiêu
JSON của mọi tool trừ `related` khớp Python: `find`, `note`, `neighbors`, `reach`, `paths`, `loops`, `provenance`, `select`, `valid`, `more`. Sau phase này lead chạy điểm đuổi theo C1 (phase 15).

## Đọc trước
- `docs/PRD.md` mục 6 (tầng Tool), 7 (dòng `validate, query`, `tools`), 8.
- Repo đặc tả ở sha ghim: `scripts/kg/query.py`, `scripts/kg/tools.py`, `scripts/kg/tests/fixture_check.py`, phần còn lại của `test_kg.py`.

## File được sửa
- `crates/pkm-core/src/{query,tools}*` và test.
- Phần cắm Rust của bộ so khớp cho tầng Tool.
- Thêm dòng vào `spec/PORTING.md`.

## Việc cần làm
- [ ] Recursive CTE chép nguyên; phần Rust chỉ ghép câu và lọc (`as_of`, `statuses`, `accepted_only`, `system`, `limit`).
- [ ] Mỗi tool trả JSON đúng từng khoá như Python, gồm tóm tắt kết quả dài (`count`, `by_type`, `handle`) và `more` để lấy tiếp.
- [ ] Link `obsidian://open` trong kết quả như Python.
- [ ] Một điểm vào chung cho mọi tool (ví dụ `tools::call(name, args)`) để MCP (phase 12) và bộ so khớp dùng. `related` thêm vào điểm vào này ở phase 11.
- [ ] Chuyển `fixture_check.py` (70 phép graph cộng các phép bảng) và đủ 15 test của `test_kg.py` (các test chưa chuyển ở phase 08, 09).

## Nghiệm thu
- Bộ so khớp tầng Tool trên kg-solo: mọi câu của `.kg-answers.json` khớp golden, 0 lệch chưa có tên (CI).
- Bộ câu sinh tự động trên bản B (máy user): 0 lệch chưa có tên.
- `spec/PORTING.md` có bảng 15 test `test_kg.py` ↔ test Rust, đủ 15 dòng.

## Cần user
Chạy bản B.
