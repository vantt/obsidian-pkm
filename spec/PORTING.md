# Nhật ký chuyển từ bản Python

Ghi mỗi commit của repo đặc tả ([`vantt/obsidian-pkm-automation`](https://github.com/vantt/obsidian-pkm-automation)) chạm đường dẫn theo dõi (PRD mục 5.1) và đã được xử lý ở repo này. Cách chuyển một commit và khi nào nâng ghim: PRD mục 5.4 và 5.5. Sha đang ghim: `spec/spec.lock`.

Mốc ban đầu: bản Rust chuyển từ đặc tả ở `5a3eb2f7990cd093b2ee3a1ccd5b46bd46e3ca4e` (2026-11-02). Bảng chỉ ghi các commit **sau** mốc đó.

Trạng thái là một trong:
- **đã chuyển:** Rust làm giống, bộ so khớp ở sha đó sạch.
- **không áp dụng:** commit chỉ đổi phần WSL, phần Python thuần, hoặc phần ngoài phạm vi (ghi lý do ở cột Ghi chú).
- **lệch có chủ ý:** Rust cố ý làm khác; ghi chi tiết ở mục "Lệch có chủ ý".

| Sha Python | Ngày | Tiêu đề | Commit Rust | Trạng thái | Ghi chú |
|---|---|---|---|---|---|

## Lệch có chủ ý

Chỗ bản Python sai rõ ràng và Rust không làm theo (PRD mục 4). Mỗi mục ghi: sha Python và vị trí (file, hàm), hành vi Python, hành vi Rust, lý do, tên ngoại lệ trong bộ so khớp, issue ở repo đặc tả (nếu user cho mở).

Chưa có mục nào.
