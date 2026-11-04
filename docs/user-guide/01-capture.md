# 01. Lưu và ghi

[← Hướng dẫn](README.md)

Nguồn: `docs/capture-to-producer/07-vault-b-workshop.md` (mục 3 Tầng 1, mục 4) và `docs/capture-to-producer/playbook/05-workshop-ui.md` của `vantt/obsidian-pkm-automation` ở `9061581`. Ví dụ là hư cấu.

## Tạo note mới

Trên Trang chủ, mục **Tạo nhanh** có ba nút, mỗi nút mở một note mới đã có sẵn mẫu:

| Nút | Note mới ở | Mẫu |
|---|---|---|
| Ý mới | `Ý/` | Ý: ý chính bằng lời mình, từ nguồn nào, đem thử ở đâu |
| Bài viết mới | `Bài viết/` | Bài viết: ý một câu, nguyên liệu, thân bài |
| Bài cắt mới | `Inbox/` | Bài cắt: vì sao lưu (một câu), nội dung |

Note tạo trong `Ý/` hay `Bài viết/` bằng cách khác (Ctrl+N, plugin Templater) cũng tự nhận mẫu của thư mục. Các mẫu khác nằm trong `_Templates/`: Trích, Khái niệm, Dự án, Mục lục, Bản đồ (MOC).

## Loại note

Mỗi note có thuộc tính `type`. Loại quyết định note dùng mẫu nào, hiện ở view nào, và Claude hiểu nó là gì.

| `type` | Là gì | Thuộc tính chính |
|---|---|---|
| `clip` | Bài cắt: thứ bạn lưu từ nơi khác | `url`, `why` (vì sao lưu), `topic` |
| `quote` | Trích: một câu của người khác | `author`, `url` |
| `note` | Ý của bạn, viết bằng lời mình | `gist` (ý chính), `source` (note nguồn) |
| `post` | Bài viết | `status` (ý tưởng, nháp, đã đăng), `uses` (nguyên liệu), `channel` |
| `concept` | Khái niệm | `up` (khái niệm cha), `related` |
| `project` | Dự án | `status`, `goal`, `uses` |
| `toc` | Mục lục thư mục do máy sinh | |
| `moc` | Bản đồ chủ đề bạn tự viết | `parent_moc` |

`url` là đường link web; `source` là link tới một note khác trong vault (ý trỏ về bài cắt nó rút ra). Đừng ghi link web vào `source`.

Note chưa có `type` mà máy đoán được (ví dụ có link web thì có thể là bài cắt) sẽ có `proposed_type` và một đề xuất "gán loại" trong hàng chờ ([03](03-proposals.md)). Máy không tự đặt `type`.

## Bộ thẻ: `wf`, `wise`, `use`

Ba thuộc tính này không phải thẻ nằm im: gắn giá trị là note hiện ra đúng chỗ trên Trang chủ ([02](02-workshop-pages.md)).

| Thuộc tính | Nghĩa | Gắn thì |
|---|---|---|
| `wf` (quy trình) | Note đang ở bước nào: `to-read`, `to-review`, `to-remember`, `to-apply`, `to-blog`… | Note lên cột trạm tương ứng; `to-apply`, `to-blog` lên mục Hôm nay; thêm `done` thì sang mục Đã xong |
| `wise` (độ thông thái, 1–7) | Nội dung đang là tín hiệu, thông tin, tri thức hay minh triết (của nội dung, không phải mức bạn hiểu) | View Độ thông thái; ý có `wise` từ 4 trở lên đáng kéo làm nguyên liệu |
| `use` (mức dùng, 1–7) | Note có ích cho việc trước mắt tới đâu | `use` 1–2 lên mục Hôm nay |

Ví dụ: một bài cắt về cách đặt câu hỏi phỏng vấn khách, bạn muốn áp dụng tuần này: `wf: [to-apply]`, `use: 1`. Sáng mai nó nằm trong mục Hôm nay.

## Thư mục và trang chủ đề

Cứ để note trong thư mục như bạn quen. Trong các thư mục gốc được chọn lúc dựng vault, thư mục nào có từ 3 note trở lên được một trang `<thư mục> (chủ đề).md`: mở ra là lưới thẻ mọi note trong thư mục, rồi các nhóm Ghi chép của mình, Bài cắt, Trích. Note trong thư mục được coi là thuộc chủ đề đó (cạnh `topic` trong graph).

Thêm thuộc tính `cover` (link hoặc đường dẫn ảnh) cho note quan trọng để thẻ của nó có ảnh bìa; không có thì lấy ảnh đầu tiên trong note.

## Đoạn tô

Tô một đoạn bằng `==đoạn==` khi đọc. Đoạn tô từ 8 ký tự trở lên là tín hiệu "đáng rút ý": máy có thể đề xuất một note nháp trong `Ý/` từ đó ([03](03-proposals.md)), và các vùng bạn tô nhiều được ưu tiên trong Gợi ý bài viết ([05](05-writing.md)).

## Không phải dọn hộp thư

Bản B không bắt bạn xử lý hết đồ lưu. Đồ lưu được dùng khi một bài viết kéo tới nó; đồ không bài nào kéo tới thì không cần xử lý.
