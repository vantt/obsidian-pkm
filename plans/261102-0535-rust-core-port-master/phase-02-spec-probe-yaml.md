---
title: "Phase 2: Bài thử S1, YAML"
status: done
priority: P1
effort: "1 ngày"
prd: R01
wave: W1
dependencies: [1]
needs_user: true
---

# Phase 2: Bài thử S1, crate YAML so với PyYAML (R01)

## Mục tiêu
Chọn crate YAML cho Rust và liệt kê mọi chỗ đọc khác PyYAML trên toàn bộ frontmatter của kg-solo và bản B, kèm cách xử lý. Chốt Q3. Đây là rủi ro cao nhất của dự án (mục 7, 16 PRD) và chặn phase 08.

## Đọc trước
- `docs/PRD.md` mục 7 (dòng `vault`), 16.
- Repo đặc tả ở sha ghim: `scripts/kg/vault.py` (cách tách và đọc frontmatter).

## File được sửa
- Tạo: `probes/s1-yaml/` (crate riêng, không là member của workspace), báo cáo `plans/reports/probe-s1-yaml-*.md`.
- Thêm mục Q3 vào `docs/DECISIONS.md`.

## Việc cần làm
- [x] Script Python nhỏ (chạy trên máy) đọc frontmatter của mọi note đúng như `vault.py`, xuất JSON có kiểu giá trị rõ (bool, int, float, date, datetime, str, null, list, map).
- [x] Chương trình Rust đọc cùng các note bằng hai hoặc ba crate ứng viên, xuất JSON cùng định dạng.
- [x] So từng note; phân loại lệch (`yes/no/on/off`, ngày và giờ, số bát phân hoặc có gạch dưới, `~`, khoá trùng, chuỗi nhiều dòng, tiếng Việt và NFC/NFD, CRLF).
- [x] Chạy trên kg-solo (dựng bằng `npx github:vantt/obsidian-test-fixture#v0.7.0` với preset và seed trong `spec.lock`).
- [x] Chạy trên bản B ở máy user: chỉ in số lệch theo loại, không in nội dung.
- [x] Chọn crate; nếu cần, thiết kế lớp chuẩn hoá kiểu giá trị để ra đúng như Python. Ghi thiết kế trong báo cáo để phase 08 làm theo.

## Nghiệm thu
- `docs/DECISIONS.md` có Q3: crate đã chọn, lý do, bảng lệch.
- Báo cáo có bảng: loại lệch, số lần trên kg-solo, số lần trên bản B, cách xử lý. Với lớp chuẩn hoá dự kiến, số lệch còn lại là 0, hoặc mỗi lệch còn lại có tên và lý do.

## Cần user
Đường dẫn bản B và cho phép chạy script trên vault đó.

## Rủi ro
Bản B có dạng YAML mà kg-solo không có, nên phải chạy cả hai. Không crate nào gần PyYAML thì gọi `kongming` trước khi chốt.

## Trạng thái (2026-11-02)
Xong mọi việc trừ bản B. Q3 đã chốt theo kg-solo, 58 ca biên và 25.000 note ngẫu nhiên: `unsafe-libyaml` + lớp tương thích, 0 lệch ([báo cáo](../reports/probe-s1-yaml-261102-1317.md)). **Còn chờ user** chạy lệnh ở mục 6 của báo cáo trên bản B; user tạm hoãn việc này. Phase 08 có thể bắt đầu theo thiết kế ở mục 5 của báo cáo; nếu bản B ra lệch khác 0 thì bổ sung lớp tương thích.

Cổng đã qua (2026-11-02, [báo cáo Windows](../reports/windows-tester-261102-1816-windows-install.md)): bài thử S1 trên bản B, 2.079 note, cột `unsafe-libyaml+layer` 0 lệch ở mọi loại.
