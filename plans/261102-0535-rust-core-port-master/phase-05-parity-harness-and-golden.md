---
title: "Phase 5: Bộ so khớp và golden"
status: todo
priority: P1
effort: "2,5 ngày"
prd: "R04, R05 (golden)"
wave: W1
dependencies: [1]
needs_user: true
---

# Phase 5: Bộ so khớp và golden kg-solo (R04, phần golden của R05)

## Mục tiêu
Dựng bộ so khớp, tiêu chuẩn "khớp" duy nhất của dự án (mục 6 PRD). Khi xong phase này: chạy Python với chính nó ra 0 lệch trên kg-solo và bản B; `parity/golden/<sha ghim>/` có đủ dump bảng và JSON tool của kg-solo; CI có bước so Rust với golden.

## Đọc trước
- `docs/PRD.md` mục 6 (toàn bộ), 4, 12.
- Repo đặc tả ở sha ghim: `scripts/kg/tests/fixture_check.py`, `test_kg.py`, `test_mcp.py`, `cli.py`, `tools.py`.

## File được sửa
- Toàn bộ `parity/`, trừ lệnh follow (phase 06 giữ).
- Sửa `.github/workflows/ci.yml`: thêm bước dựng kg-solo và so với golden.

## Ràng buộc thiết kế
- **CI không có Python** (Q5). Phần so sánh chạy trong CI phải là Rust (gợi ý: một package Rust trong `parity/` là member của workspace, phụ thuộc `pkm-core` để gọi thẳng tool). Phần chạy Python chỉ chạy trên máy.
- Định dạng dump trung lập ngôn ngữ (JSONL hoặc TSV chuẩn hoá), sắp theo khoá, bỏ cột thời gian dựng, `mtime` cho lệch 1 ms.
- Tầng nào Rust chưa có thì báo "chưa có", không bao giờ tính là khớp.
- Ngoại lệ có tên nằm trong một file (gợi ý `parity/exceptions.toml`), mỗi mục trỏ tới mục "Lệch có chủ ý" của `spec/PORTING.md`.

## Việc cần làm
- [ ] Phía Python (chạy trên máy): clone repo đặc tả ở một sha bất kỳ, dựng index, dump mọi bảng của mục 6 (`nodes`, `props`, `links`, `raw_links`, `blocks`, `names`, `edges`, `meta`, mọi `schema_*`, view `v3_deviations`), chạy bộ câu tool, dump MCP handshake (`initialize`, `tools/list`, `instructions`), dump đầu ra CLI.
- [ ] Bộ câu: (1) mọi câu trong `.kg-answers.json` của kg-solo; (2) bộ sinh tự động cho bản B (mỗi loại note vài note, mỗi tool vài tham số, gồm `as_of`, `statuses`, `accepted_only`, `system`, `limit`).
- [ ] Phần so: so từng dòng bảng, so JSON tool sau khi chuẩn hoá thứ tự danh sách không thứ tự; `related` so tập note, thứ hạng, điểm lệch tối đa 1e-6; MCP và CLI so chuỗi.
- [ ] Tầng hiệu năng: đo `build` đủ, `build` tăng dần không đổi gì, một vòng refresh MCP, mỗi tool; ghi số, không chặn.
- [ ] Đầu ra `parity/report.json` và bảng tóm tắt; mã thoát khác 0 khi có lệch chưa có tên.
- [ ] Chế độ `--private <vault>`: chỉ in số dòng lệch theo bảng và theo tool, không in nội dung note.
- [ ] Xuất golden kg-solo ở `5a3eb2f` vào `parity/golden/5a3eb2f7990cd093b2ee3a1ccd5b46bd46e3ca4e/`, kèm lệnh dựng lại golden ở sha khác (phase 15 dùng khi nâng ghim).
- [ ] Bước CI: `npx github:vantt/obsidian-test-fixture#v0.7.0 generate …` với preset, seed của `spec.lock`, rồi chạy phần so với golden của sha ghim.
- [ ] `parity/README.md` ngắn: cách chạy trên máy, trên bản B, cách dựng golden.

## Nghiệm thu
- Python ở sha ghim so với chính nó: 0 lệch trên kg-solo và trên bản B (máy user, chỉ in số).
- Cố ý sửa một dòng trong một file golden thì lệnh so trả mã khác 0 và chỉ đúng dòng đó.
- CI xanh với bước so mới (mọi tầng báo "chưa có" ở thời điểm này là đúng).
- Không có nội dung bản B trong repo (`git grep` theo vài tên note bản B do user đưa ra cho kết quả rỗng).

## Cần user
Đường dẫn bản B, cho phép chạy trên máy user.
