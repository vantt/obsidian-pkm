---
title: "Phase 6: Lệnh follow"
status: todo
priority: P2
effort: "0,25 ngày"
prd: "R05 (follow)"
wave: W1
dependencies: [1]
needs_user: false
---

# Phase 6: Lệnh `follow` (phần follow của R05)

## Mục tiêu
Một lệnh chạy trên máy (`parity/follow.py` hoặc `cargo xtask follow`, chọn một và ghi vào `docs/DECISIONS.md`) liệt kê commit của repo đặc tả từ sha trong `spec.lock` tới `main` có chạm đường dẫn theo dõi, và đo khoảng cách.

## Đọc trước
- `docs/PRD.md` mục 5 (toàn bộ).
- `spec/spec.lock`.

## File được sửa
- Tạo: `parity/follow.py` hoặc `xtask/` (chỉ một), thêm một mục nhỏ vào `docs/DECISIONS.md`.

## Việc cần làm
- [ ] Fetch repo đặc tả qua `gh` hoặc `git` dùng quyền sẵn có trên máy; không dùng token hay secret mới.
- [ ] Đường dẫn theo dõi đúng như mục 5.1 PRD, giữ trong một danh sách duy nhất trong code.
- [ ] Mỗi commit một dòng: sha, ngày, tiêu đề, file chạm, số dòng đổi; theo thứ tự thời gian.
- [ ] Dòng cuối: số commit chưa chuyển và số ngày từ commit cũ nhất chưa chuyển, so với mục tiêu (dưới 5 commit, dưới 7 ngày).
- [ ] Đánh dấu commit đã ghi trong `spec/PORTING.md` là đã xử lý (đã chuyển hoặc không áp dụng).
- [ ] Không có workflow CI nào gọi lệnh này (mục 5.3 PRD).

## Nghiệm thu
- Chạy lệnh in đúng danh sách commit từ `5a3eb2f`; đối chiếu bằng `git log 5a3eb2f..origin/main -- <đường dẫn theo dõi>` trong bản clone repo đặc tả cho cùng tập sha.
- Lệnh chạy được hai lần liên tiếp không lỗi (dùng lại bản clone cũ).
