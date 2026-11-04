# 02. Trang xưởng

[← Hướng dẫn](README.md)

Nguồn: `docs/capture-to-producer/playbook/05-workshop-ui.md` (phần dùng, không có phần chỉnh mã) và `docs/capture-to-producer/07-vault-b-workshop.md` (mục 3 Tầng 3) của `vantt/obsidian-pkm-automation` ở `9061581`.

Mở vault là thấy việc của hôm nay, không cần Ctrl+P hay nhớ lệnh nào.

## Trang chủ

Mở lúc khởi động (plugin Homepage), ở chế độ đọc. Các mục:

| Mục | Hiện gì |
|---|---|
| Tạo nhanh | Nút Ý mới, Bài viết mới, Bài cắt mới ([01](01-capture.md)) |
| Hôm nay | Tối đa 5 note nên đụng tới hôm nay: `use` 1–2, hoặc `wf` là `to-apply`, `to-blog` |
| Bốn trạm | Làm rõ, Suy ngẫm, Ghi nhớ, Sản xuất, theo giá trị `wf`. Note nằm lâu nhất hiện đầu |
| Đã xong | Note có `wf` chứa `done` |
| Chủ đề, Bản đồ (MOC) | Mọi trang chủ đề máy sinh; bản đồ bạn tự viết |
| Bài viết | Chia theo `status`: ý tưởng, nháp, đã đăng |
| Đề xuất chờ duyệt, Đề xuất đã xử lý | Hàng chờ nhóm theo kiểu ([03](03-proposals.md)) |
| Chờ duyệt, Mới lưu | Note có loại máy đoán chưa duyệt; note mới trong hộp thư |
| Độ thông thái, Trích theo tác giả | Note theo `wise`; trích nhóm theo người nói |

Các trạm theo giá trị `wf`:

| Trạm | `wf` |
|---|---|
| Làm rõ | `to-read`, `to-research`, `to-summary`, `to-clarify` |
| Suy ngẫm | `to-review`, `re-read`, `to-re-read`, `to-absorb`, `re-think` |
| Ghi nhớ | `to-remember`, `to-surprise` |
| Sản xuất | `to-organize`, `to-apply`, `to-blog`, `to-mixture` |

## Trang chủ đề

`<thư mục> (chủ đề).md` mở ra là lưới thẻ mọi note của thư mục, rồi Ghi chép của mình, Bài cắt, Trích, Đã gắn thẻ. Đây là mục lục máy sinh (`type: toc`), sẽ được sinh lại: đừng viết vào đó. Muốn bản đồ thật cho một chủ đề thì tạo note từ mẫu "Bản đồ (MOC)". Trong mẫu, các callout `[!question]` là chỗ chỉ bạn viết (ý định, cầu nối giữa các nhóm); vùng giữa `%% moc:auto:start %%` và `%% moc:auto:end %%` là chỗ máy được ghi.

## Tự chỉnh

Mọi trang và view là file trong vault, sửa thẳng trong Obsidian. Lần dựng lại sau giữ file bạn đã sửa.

- **View (Bases):** thanh trên mỗi view cho đổi sắp xếp, cột, cỡ thẻ; Obsidian ghi lại vào `_Xưởng/Xưởng.base` hay `_Xưởng/Chủ đề.base`. Muốn sửa bộ lọc (ví dụ mục Hôm nay) thì mở file `.base` đó.
- **Mẫu note:** sửa file trong `_Templates/`.
- **Trang chủ:** sửa `Trang chủ.md`, thêm bớt mục, đổi thứ tự.

Công thức hay dùng:

- **Mục Hôm nay trống.** Bạn ít gắn `use` 1–2 hay `to-apply`, `to-blog`. Gắn thêm, hoặc sửa bộ lọc view "Hôm nay" trong `_Xưởng/Xưởng.base`. Đừng nâng giới hạn 5: Hôm nay là ít mà trúng.
- **Một trạm luôn dài, không ai mở.** Thêm `limit` cho view của trạm, hoặc tách trạm.
- **Thẻ trên trang chủ đề không có ảnh.** Thêm `cover` cho note; chỉnh cỡ thẻ trên thanh của view Lưới.

## Giao diện

Bản B dùng theme Primary cộng một snippet `e11-theme` (bật trong Settings, Appearance, CSS snippets): chữ đủ tương phản ở cả Light lẫn Dark, font đọc Merriweather cỡ 19, bảng có viền, tiêu đề và sọc. Gỡ snippet là về Primary nguyên gốc.

- **Đổi font đọc, font heading:** plugin Style Settings, mục "E11 giao diện" (font đọc Merriweather hay Inter; heading Inter, Archivo Narrow, Manrope hay cùng font đọc). Đổi font ở Settings, Appearance, Text font không có tác dụng trong note vì snippet đặt font cho khung note.
- **Đổi cỡ chữ:** thanh cỡ chữ ở Settings, Appearance.
- **Bảng rộng bị bóp cột:** rê chuột lên bảng, bấm nút ở góc trên bên phải để phóng bảng ra hết chiều ngang; bấm lại hoặc Esc để thu. Có cho bảng Markdown, bảng Bases và bảng Dataview. Lệnh (Ctrl+P): "Phóng hoặc thu mọi bảng trong note đang mở".

## Lưu ý

- Cần Obsidian 1.9 trở lên và core plugin Bases bật.
- Đóng vault trong Obsidian trước khi chạy lệnh dựng hay cài lại: Obsidian ghi đè danh sách plugin lúc đóng.
