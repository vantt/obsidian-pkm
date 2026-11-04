# 05. Viết bài

[← Hướng dẫn](README.md)

Nguồn: `docs/capture-to-producer/playbook/06-writing.md` (mục trang Nguyên liệu, Gợi ý bài viết), `docs/capture-to-producer/09-material-while-writing.md` (mục 1, 2) và `docs/capture-to-producer/07-vault-b-workshop.md` (mục 3 Tầng 4) của `vantt/obsidian-pkm-automation` ở `9061581`. Bỏ phần chỉnh mã và số đo. Ví dụ là hư cấu.

Đây là tầng đích: mỗi tuần một bài, và bài kéo đồ lưu cũ ra dùng. Đầu ra kéo đầu vào, thay vì dọn hết đồ lưu rồi mới viết.

| Lúc | Dùng | Đưa ra |
|---|---|---|
| Chưa biết viết gì | Trang **Gợi ý bài viết** | Những vùng note bạn đã đầu tư mà chưa thành bài |
| Đang viết một bài | Trang **Nguyên liệu** cạnh bản nháp | Note khớp từng đoạn của bài, và note graph đưa tới mà tìm chữ không ra |
| Có câu hỏi về note đang mở | Chat trong Obsidian ([04](04-ask-claude.md)) | Note ủng hộ, phản bác, ví dụ, kèm câu trích |

Máy chỉ chỗ; bạn đọc và viết.

## Vòng một tuần

1. Bấm **Bài viết mới** trên Trang chủ. Viết ý một câu; có dàn ý thì tốt hơn.
2. Gọi trang Nguyên liệu cho bài.
3. Đọc lướt, mở note nào thấy đáng, rút ý bằng lời mình vào `Ý/`.
4. Đánh dấu note dùng; chúng vào `uses` của bài. Mục "Nguyên liệu" trong bản nháp (từ mẫu) liệt kê chúng.
5. Viết, đăng, đổi `status` của bài thành `đã đăng`.

## Trang Nguyên liệu

Trang `_Xưởng/Nguyên liệu/<tên bài>.md`, dựng lại từ bản nháp mới nhất mỗi lần gọi. Lệnh gọi đang chuyển vào `pkm` và plugin Obsidian; tới khi có, nhờ người cài vault chạy, hoặc hỏi Claude "gom nguyên liệu cho bài này" (khi đó nội dung bài đi qua Claude).

| Mục | Có gì |
|---|---|
| Đã dùng trong bài | Note trong `uses` hoặc đã link trong thân bài |
| Tìm theo chữ | Note khớp các đoạn của bài, chia theo vai trò: ý của bạn, trích, bài lưu. Mỗi dòng ghi đoạn nào của bài khớp, `wise`, `use`, `gist`, `why` nếu có, và hai đoạn tô đầu |
| Graph đưa tới, tìm chữ không ra | Note mà tìm chữ không ra nhưng graph nối tới từ một note tìm được, kèm đường đi. Bước nào là cạnh máy gợi ý thì in nghiêng, ghi ai gợi ý và điểm |
| Cửa vào chủ đề | Thư mục chứa nhiều note tìm được, để mở cả nhóm |
| Đoạn của bài máy đã tìm | Các đoạn máy lấy từ bản nháp, để bạn thấy máy hiểu bài thế nào |

Dùng một note: đánh dấu `[x]` ở dòng của nó rồi gọi lại. Note đó vào `uses` của bài, và được ghi vào sổ `_E11/material-log.jsonl`. Dùng là duyệt: cạnh `uses` này thuộc lớp `accepted`. Trang không bao giờ sửa note nào khác ngoài `uses` của chính bản nháp.

Mẹo:

- **Kết quả chung chung.** Viết dàn ý thành đoạn có tiêu đề rõ, mỗi đoạn một ý: mỗi đoạn là một lần tìm. Mục "Đoạn của bài máy đã tìm" cho thấy đoạn nào máy lấy sai. Mục tiêu đề "Nguyên liệu" của bản nháp không đem đi tìm.
- **Toàn note cùng thư mục.** Thư mục đó có nhiều cạnh "liên quan" máy gợi ý nội bộ: duyệt các đề xuất liên quan của thư mục đó ([03](03-proposals.md)).

Ví dụ: bản nháp "Vì sao tôi bỏ to-do list" có ba đoạn: cảm giác tội lỗi khi danh sách dài, thử chỉ giữ ba việc mỗi ngày, điều học được. Trang Nguyên liệu đưa ra một trích về sức chú ý có hạn, một bài cắt về phương pháp "một việc lớn mỗi ngày", và qua graph một ý cũ của bạn về năng lượng buổi sáng mà tìm chữ không ra vì không có từ "to-do" nào.

## Gợi ý bài viết

Trang `_Xưởng/Gợi ý bài viết.md` chỉ ra các **vùng tri thức**: cụm note gần nghĩa mà bạn tô nhiều, viết ý riêng nhiều, trải nhiều thư mục và nhiều năm, nhưng chưa có bài. Vùng xếp theo điểm. Mỗi vùng có tên (các chữ hay gặp trong tiêu đề, máy đặt; góc viết là việc của bạn), số note, số thư mục, số đoạn tô, số ý của bạn, các note chính, vài đoạn tô tiêu biểu, nút **Viết bài từ vùng này** (tạo bản nháp trong `Bài viết/` có sẵn danh sách note nguồn) và dòng **Chấm**.

Ghi vào dòng **Chấm** của mỗi vùng: `đáng viết`, `không`, hoặc `bất ngờ` (vùng tự mình không thấy). Lần dựng lại sau không ghi đè trang khi trang đã có, để giữ các dòng Chấm của bạn; muốn dựng lại thì chép các dòng Chấm ra trước.

Màu tô (khi vault chuyển từ Evernote còn giữ màu) có trọng số khác nhau: đoạn tô theo vai trò "làm được", "lỗi mình hay mắc" nặng hơn đoạn tô "thú vị".
