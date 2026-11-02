---
title: "Phase 4: Bài thử S3, OHS stdio"
status: todo
priority: P2
effort: "0,5 ngày"
prd: R03
wave: W1
dependencies: [1]
needs_user: false
---

# Phase 4: Bài thử S3, gọi `ohs serve --stdio` (R03)

## Mục tiêu
Chứng minh Rust giữ được một tiến trình OHS upstream qua stdio: chờ `{"ready":true}`, gửi nhiều request `search` đồng thời theo `id`, chuẩn hoá đường dẫn trả về sang NFC. Kết quả là mẫu cho module `ohs` ở phase 11.

## Đọc trước
- `docs/PRD.md` mục 3.1, 9, quyết định D5–D7.
- Fork `vantt/obsidian-hybrid-search`: `vantt/PRD.md` và tài liệu giao thức stdio của upstream. Không sửa fork.

## File được sửa
- Tạo: `probes/s3-ohs/` (crate riêng, không là member của workspace), báo cáo `plans/reports/probe-s3-ohs-*.md`.

## Việc cần làm
- [ ] Lấy hoặc dựng gói OHS dựng sẵn (thư mục có `node/` và thư mục build của app), chỉ đường bằng cờ hoặc `PKM_OHS_DIR`.
- [ ] Khởi động lệnh `serve --stdio` của OHS như mục 9 PRD, với `OBSIDIAN_VAULT_PATH` trỏ vào kg-solo; chờ ready.
- [ ] Gửi ít nhất 5 request `search` đồng thời, ghép đúng câu trả lời theo `id`.
- [ ] Chuẩn hoá đường dẫn trả về sang NFC; thử với tên file tiếng Việt dạng NFD.
- [ ] Đóng stdin khi thoát, chờ có giới hạn, không để tiến trình mồ côi.
- [ ] Ghi cách đọc trạng thái (`starting`, `indexing` kèm tiến độ, `model_missing`) từ OHS để phase 11 dựng lỗi `ohs_not_ready` (D7).

## Nghiệm thu
- Kết quả `search` của bài thử trùng với cùng câu hỏi gọi qua CLI OHS (so tập note).
- Sau khi chương trình thoát, `ps` không còn tiến trình `node` của bài thử.

## Rủi ro
Giao thức stdio của upstream đổi: ghi phiên bản OHS đã thử vào báo cáo.
