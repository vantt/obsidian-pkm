---
title: "Phase 15: Đuổi theo commit Python"
status: in-progress
priority: P1
effort: "10–20% tổng công, chia theo các điểm C0–C3"
prd: R12
wave: "C0–C3 (lặp)"
dependencies: [6]
needs_user: true
---

# Phase 15: Đuổi theo commit Python (R12)

## Mục tiêu
Đưa khoảng cách giữa Rust và Python về 0 trước khi chuyển hẳn: mọi commit của repo đặc tả chạm đường dẫn theo dõi đều đã chuyển hoặc đánh dấu "không áp dụng", và `spec.lock` ghim sha mới nhất với bộ so khớp sạch.

## Đọc trước
- `docs/PRD.md` mục 4, 5 (toàn bộ), 6 (đoạn golden).
- `spec/PORTING.md`, `parity/README.md`.

## Khi nào chạy
Chỉ khi user bảo (mục 5.3 PRD). Lead đề xuất ở bốn điểm, kèm số commit mà `follow` đang báo:

| Điểm | Lúc | Cách làm |
|---|---|---|
| C0 | Ngay sau cổng G0, trước phase 08 | Chưa có lõi Rust, nên chỉ cần: nâng `spec.lock` lên sha mới, dựng lại golden, chạy Python với chính nó ra 0 lệch. Rẻ nhất, nên làm |
| C1 | Sau phase 10 | Chuyển commit chạm `vault`, `schema`, `index`, `validate`, `query`, `tools`, adapter |
| C2 | Sau phase 13 | Chuyển commit chạm `related`, `mcp`, CLI, `desktop_config`, `platform` |
| C3 | Trước phase 16 | Lượt cuối, `follow` phải về 0 |

Commit chạm module chưa chuyển thì để lại; phase chuyển module đó đọc đặc tả ở sha ghim mới nhất.

## Cách chuyển một lượt
Một agent con cho mỗi lượt (không chia commit cho nhiều agent song song, vì phải theo thứ tự thời gian).
- [ ] Chạy `follow`, lấy danh sách commit theo thứ tự thời gian.
- [ ] Mỗi commit: đọc diff và test đi kèm → sửa Rust → chạy bộ so khớp với Python **ở sha của commit đó** → ghi `spec/PORTING.md` (sha Python, commit Rust, lệch có chủ ý nếu có). Commit chỉ đổi phần WSL hoặc phần Python thuần (ví dụ `utf8_stdio`) thì ghi "không áp dụng" kèm lý do.
- [ ] Commit chỉ đổi `kg_adapter.py` chuyển riêng, chỉ đụng `adapter::e11`.
- [ ] Khi mọi commit tới sha X đã xử lý và so khớp ở X sạch: dựng golden ở X vào `parity/golden/<X>/`, đặt `spec.lock` = X, cùng một PR.
- [ ] Cập nhật số đo khoảng cách vào báo cáo.

## Các lượt đã chạy
- [x] Lượt C (2026-11-02, user bảo chạy): chuyển `223c9c3` (OHS chạy từ gói dựng sẵn của fork với gói tiếng Việt) sang `pkm_core::ohs` (commit Rust `ae2ffa1`); nâng `spec.lock` từ `5a3eb2f` lên `223c9c3`, golden dựng lại ở `parity/golden/223c9c3…/` (chỉ đổi `spec_sha`). Python với chính nó và Rust với golden mới (chạy như CI): 64 mục, 0 lệch chưa có tên, 0 mục chưa có. `follow`: 0 commit chưa chuyển. Khoảng cách trước lượt: 1 commit, dưới 1 ngày. Báo cáo: `plans/reports/fullstack-developer-261102-1615-chase-223c9c3.md`.
- [x] Lượt 2 (2026-11-02, user bảo chạy): `follow` báo 5 commit, khoảng cách dưới 1 ngày. `24f193a` (mẫu bỏ qua `_ohs/**` và `kg_adapter` coi `_ohs/` là thư mục hệ thống; phần adapter chuyển ở commit Rust `3211180`, phần mẫu bỏ qua đã có từ `2fd131b`), `58010ad` và `8f89403` (công tắc `PACK_IGNORE_ON`, đã có từ `328716f`, đối chiếu lại), `6af65b1` (chỉ gói của fork: Rust đã vậy), `9f0700b` (bỏ `desktop_config.py`: không áp dụng, `pkm mcp install` đã thay). Nâng `spec.lock` từ `223c9c3` lên `da62ecb` (đầu `main` của repo đặc tả; các commit còn lại ngoài đường dẫn theo dõi), golden dựng lại ở `parity/golden/da62ecb…/` (chỉ đổi `spec_sha`). Python với chính nó và Rust với golden mới (chạy như CI): 64 mục, 0 lệch chưa có tên, 0 mục chưa có. `follow`: 0 commit chưa chuyển. Báo cáo: `plans/reports/fullstack-developer-261102-2217-phase-15-round-2.md`.
- [ ] Lượt sau: khi `follow` lại báo commit mới chạm đường dẫn theo dõi.

## Nghiệm thu (mỗi lượt)
- `follow` báo 0 commit chưa xử lý tới sha X.
- CI xanh với golden mới; bản B sạch trên máy user ở sha X.
- `spec/PORTING.md` có một dòng cho mỗi commit đã xử lý.

## Nghiệm thu (cả phase, trước phase 16)
- `follow` báo 0 commit chưa chuyển so với `main` của repo đặc tả.

## Rủi ro
Python sửa nhanh hơn Rust đuổi. Khi khoảng cách vượt mục tiêu (5 commit hoặc 7 ngày), lead báo user thay vì cố đuổi trong im lặng.
