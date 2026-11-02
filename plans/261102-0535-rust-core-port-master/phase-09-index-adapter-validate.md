---
title: "Phase 9: index, adapter e11, validate"
status: todo
priority: P1
effort: "3 ngày"
prd: R07
wave: W3
dependencies: [8]
needs_user: true
---

# Phase 9: `index`, `adapter::e11`, `validate` (R07)

## Mục tiêu
Dựng index đủ và tăng dần như Python, cùng adapter bản B và validator. Mọi bảng dữ liệu và view `v3_deviations` khớp.

## Đọc trước
- `docs/PRD.md` mục 2.1, 3 (D3: SQL giữ nguyên), 7 (dòng `index`, `validate`, `adapter::e11`), 16 (rủi ro `kg_adapter.py`).
- Repo đặc tả ở sha ghim: `scripts/kg/index.py`, `scripts/kg/validate.py`, `scripts/e11/kg_adapter.py`, phần còn người gọi của `scripts/e11/graph_index.py`, test liên quan trong `test_kg.py`.

## File được sửa
- `crates/pkm-core/src/{index,validate,adapter}*` và test; `crates/pkm-core/Cargo.toml`.
- Phần cắm Rust của bộ so khớp cho `edges`, `meta`, `v3_deviations`.
- Thêm dòng vào `spec/PORTING.md`.

## Việc cần làm
- [ ] SQLite qua `rusqlite` (feature `bundled`); DDL, view validator chép nguyên từ Python.
- [ ] Build đủ (`--full`) và tăng dần theo mtime và hash; thứ tự ghi và chuẩn hoá giá trị như Python.
- [ ] Hai lớp cạnh; cửa sổ hiệu lực; nguồn gốc (`origin`, `proposed_by`, `extracted_by`); `--edge-keys`, `--body-edges`.
- [ ] Trait `Adapter` (impl đầu tiên là `e11`): cạnh `topic` theo trang chủ đề (`toc`, tên `(chủ đề).md`), cạnh từ hàng chờ đề xuất, cạnh OHS từ file cặp gần nghĩa, danh sách trang hệ thống. Giữ phần e11 nhỏ, tách riêng để commit chỉ đổi adapter chuyển được độc lập.
- [ ] Nhớ cách đọc vault trong index sau lần build đầu, như Python.
- [ ] `validate` với `--rule`, `--limit`, `--json`.
- [ ] Chuyển các test Python liên quan.

## Nghiệm thu
- Bộ so khớp tầng bảng: **mọi** bảng của mục 6 PRD và `v3_deviations` khớp golden kg-solo (CI) và khớp bản B (máy user), 0 lệch chưa có tên.
- Build tăng dần hai lần liên tiếp không đổi gì cho DB giống hệt build đủ (so bằng bộ so khớp).
- Số đo hiệu năng build đủ và tăng dần đã ghi; Rust không chậm hơn Python.

## Cần user
Chạy bản B.
