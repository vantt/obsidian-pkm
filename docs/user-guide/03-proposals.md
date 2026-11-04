# 03. Hàng chờ đề xuất

[← Hướng dẫn](README.md)

Nguồn: `docs/capture-to-producer/playbook/03-review-queue.md` (bảng kiểu đề xuất, vòng duyệt, bẫy) và `docs/capture-to-producer/07-vault-b-workshop.md` (mục 3 Tầng 2) của `vantt/obsidian-pkm-automation` ở `9061581`. Bỏ phần chỉnh luật trong mã và số đo. Ví dụ là hư cấu.

Graph cần cạnh: quan hệ giữa hai note. Bạn không có thời gian tự nối hàng nghìn note, còn để máy tự nối thì graph đầy nhiễu và bạn không tin. Bản B làm thế này: **máy tìm ứng viên, bạn quyết, mỗi lần nhận sinh ra một cạnh thật.**

## Một đề xuất trông thế nào

Mỗi đề xuất là một note trong `_Đề xuất/`, có:

- `kind`: kiểu đề xuất (bảng dưới);
- `reason`: vì sao máy đề xuất;
- các note liên quan, nhúng ngay trong đề xuất (tối đa 3) để đọc tại chỗ;
- `status: chờ`;
- hồ sơ: ai đề xuất (`proposed_by`: `rule` là luật cố định, `model` là model tìm theo nghĩa) và bằng cách nào (`extracted_by`).

## Bảy kiểu

| Kiểu | Máy dựa vào | Nhận thì |
|---|---|---|
| Gán loại (`type-batch`) | Note có loại máy đoán (`proposed_type`), gom theo thư mục thành lô | Cả lô nhận `type` |
| Trùng (`duplicate`) | Nội dung giống hệt nhau, dài trên 40 ký tự | `duplicate_of` trỏ về bản giữ; không xoá note nào |
| Sửa liên kết (`fix-link`) | Link gãy mà chỉ có đúng một note tên gần giống | Link trong note nguồn được sửa |
| Cùng chủ đề (`same-topic`) | Hai trang chủ đề cùng tên khi bỏ dấu | `related` giữa hai trang |
| Liên quan (`related`) | Hai note gần nghĩa nằm ở hai thư mục khác nhau (tìm từ khoá dễ bỏ sót) | `related` hai chiều |
| Rút ý (`idea`) | Note có đoạn bạn đã tô (`==đoạn==`, từ 8 ký tự) | Một note nháp trong `Ý/`, `source` trỏ về note gốc; ý chính để trống cho bạn viết |
| Người (`person`) | Họ tên xuất hiện ở ít nhất 2 note | Trang `Người/<tên>` và thuộc tính `who` trên các note đó |

Kiểu rút ý và người giới hạn 20 đề xuất chờ cùng lúc, để hàng chờ không thành một hộp thư nữa.

## Vòng duyệt

1. Mở Trang chủ, mục **Đề xuất chờ duyệt** (nhóm theo kiểu).
2. Mở một đề xuất, đọc lý do và note nhúng. Chưa chắc thì mở note nguồn.
3. Đổi `status` thành `nhận` hoặc `bỏ`.
4. Bỏ thì ghi vài chữ vào `reject_reason` (Lý do bỏ), ví dụ "tên phố, không phải người". Lý do bỏ là dữ liệu để chỉnh luật; đề xuất bỏ không được gợi ý lại.
5. Áp dụng: lệnh áp dụng đọc các đề xuất đã nhận hay bỏ, sửa note theo đó, ghi sổ `_E11/proposals-log.jsonl` để trả lại được. Lệnh này đang chuyển vào `pkm` và plugin Obsidian; tới khi có, nhờ người cài vault chạy.

Thứ tự nên duyệt khi hàng chờ còn mới: kiểu nhẹ trước (gán loại, trùng, sửa liên kết), rồi liên quan, rút ý, người.

Ví dụ: đề xuất "Liên quan" giữa một bài cắt về thói quen buổi sáng (thư mục `Sức khoẻ/`) và ý của bạn về cách giữ nhịp viết (thư mục `Viết/`). Mở cả hai, thấy cùng nói về việc làm việc khó trước khi kiểm tin nhắn: nhận. Lần sau hỏi Claude về nhịp viết, bài cắt kia hiện ra qua cạnh đã duyệt.

## Trước và sau khi duyệt

- Đề xuất đang chờ đã có trong graph ở lớp `proposed` (máy gợi ý): Claude và trang Nguyên liệu thấy được nhưng phải ghi rõ là gợi ý.
- Nhận và áp dụng xong thì cạnh lên lớp `accepted` (bạn đã duyệt).
- Bỏ thì cạnh ở lớp `rejected`: lưu để không gợi ý lại, không bao giờ trả về.

## Lưu ý

- **Mở nguồn trước khi gật.** Plugin review log ghi lại bạn mở note nào, lúc nào đổi `status` (chỉ đường dẫn và giờ, không ghi nội dung, không gửi ra ngoài vault). Nhận nhiều mà ít khi mở nguồn là dấu hiệu bạn đang gật đầu theo máy.
- **Đừng xoá đề xuất đã bỏ.** Xoá thì lần chạy sau máy gợi ý lại.
- **Lưu note trước khi áp dụng.** Lệnh áp dụng đọc trạng thái từ file; `status` vừa đổi mà Obsidian chưa ghi xuống đĩa thì bị bỏ sót.
- **Người không có trong hàng chờ** (tên nước ngoài, biệt danh): tự tạo trang `Người/<tên>` và ghi `who` trên note.
