---
title: "Phase 11: related và module OHS"
status: todo
priority: P1
effort: "1,5 ngày"
prd: R09
wave: W5
dependencies: [10, 4]
needs_user: false
---

# Phase 11: `pkm-core::related` và `pkm-core::ohs` (R09)

## Mục tiêu
`related` khớp Python cả khi có `hits` và khi chỉ có câu hỏi; module OHS duy nhất (D6) giữ một tiến trình `serve --stdio` sống lâu và báo trạng thái đúng D7. Chạy song song với phase 12.

## Đọc trước
- `docs/PRD.md` mục 3.1 (giai đoạn so khớp: hai cửa), 7 (dòng `related`), 8 (lỗi `ohs_not_ready`), 9 (toàn bộ), D5–D7.
- Báo cáo phase 04.
- Repo đặc tả ở sha ghim: `scripts/kg/related.py`, chỗ `mcp_server.py` gọi OHS.

## File được sửa
- `crates/pkm-core/src/{related,ohs}*` và test; thêm nhánh `related` vào điểm vào chung của `tools` (một chỗ, không đổi tool khác).
- Phần cắm Rust của bộ so khớp cho `related`.

## Việc cần làm
- [ ] PageRank cá nhân hoá: trọng số theo lớp (`WEIGHT`), `alpha`, `eps`, `k` như Python.
- [ ] Module `ohs`: tìm gói (cờ `--ohs`, `PKM_OHS_DIR`, thư mục mặc định); khởi động với `OBSIDIAN_VAULT_PATH` (và `OBSIDIAN_DB_PATH` khi fork có P02); chờ ready; request có `id`, đồng thời; khởi động lại có giới hạn rồi `broken`; tắt khi `pkm` thoát.
- [ ] Trạng thái `starting`, `indexing` (tiến độ từ `status` của OHS), `model_missing`, `broken`, `ready`.
- [ ] Tool cần OHS khi chưa ready trả đúng `{"error":"ohs_not_ready","state":…,"progress":…,"eta_s":…,"action":…}`; không có đường lùi cho kết quả kém (D7). Tool không cần OHS chạy bình thường.
- [ ] Chuẩn hoá đường dẫn OHS trả về sang NFC trước khi tra index.
- [ ] Không đọc thẳng DB hay `node_modules` của OHS.

## Nghiệm thu
- `related` có `hits` trên kg-solo: tập note và thứ hạng khớp golden, điểm lệch tối đa 1e-6 (CI).
- `related` chỉ có câu hỏi: khớp Python trên máy có gói OHS (CI không có OHS; ghi rõ điều này trong `parity/README.md`).
- Test: OHS thiếu gói trả `state: broken` kèm `action` cài đặt; tiến trình OHS bị giết thì được khởi động lại; sau khi `pkm` thoát không còn tiến trình `node` con.
