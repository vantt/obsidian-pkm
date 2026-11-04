# Hướng dẫn dùng bản B

Nguồn: `docs/capture-to-producer/07-vault-b-workshop.md` (mục 1–4) và `docs/capture-to-producer/playbook/README.md` của repo nghiên cứu `vantt/obsidian-pkm-automation` ở `9061581`. Chỉ giữ phần người dùng cần; số đo, thử nghiệm và cách chỉnh mã ở lại repo nghiên cứu. Mọi ví dụ là hư cấu.

## Bản B là gì

Một kho ghi chú thường chỉ lưu và tìm lại bằng từ khoá. Bản B biến vault Obsidian thành một **xưởng**: đồ lưu vào, bạn rút ra thành ý của mình, ý thành bài viết. Trên đường đi, các note được nối với nhau thành một graph, để bạn và Claude thấy những quan hệ mà tìm từ khoá không ra.

Một nguyên tắc xuyên suốt: **bạn là người làm vườn**. Máy tìm, gợi ý, chỉ chỗ; bạn quyết. Máy không tự sửa note mà bạn không biết.

## Bốn tầng

| Tầng | Bạn thấy gì | Đọc |
|---|---|---|
| Nguyên liệu | Note bạn lưu và viết: bài cắt, trích, ý, khái niệm, dự án | [01 Lưu và ghi](01-capture.md) |
| Giao diện | Trang chủ, bốn trạm, mục Hôm nay, trang chủ đề dạng lưới thẻ | [02 Trang xưởng](02-workshop-pages.md) |
| Cấu trúc | Hàng chờ đề xuất: máy gợi ý nối note, bạn nhận hay bỏ | [03 Đề xuất](03-proposals.md) |
| Đầu ra | Bản nháp trong `Bài viết/`, trang Nguyên liệu, gợi ý nên viết gì | [05 Viết bài](05-writing.md) |

Hỏi Claude về vault (Claude Desktop, hoặc chat ngay trong Obsidian) qua MCP `pkm`: [04 Hỏi Claude](04-ask-claude.md).

## Thư mục trong vault

| Thư mục, file | Là gì | Bạn sửa được không |
|---|---|---|
| `Trang chủ.md` | Trang mở lúc khởi động | Được; lần dựng sau giữ bản bạn đã sửa |
| `Ý/` | Ý của bạn, viết bằng lời mình | Của bạn |
| `Bài viết/` | Bản nháp và bài đã đăng | Của bạn |
| `Người/` | Trang người, tạo khi bạn nhận một đề xuất người | Của bạn |
| `<thư mục> (chủ đề).md` | Trang chủ đề máy sinh cho mỗi thư mục đủ note (`type: toc`) | Không nên: trang được sinh lại. Muốn bản đồ riêng thì tạo note từ mẫu "Bản đồ (MOC)" |
| `_Xưởng/` | File `.base` (các view của Trang chủ và trang chủ đề), trang Nguyên liệu, trang Gợi ý bài viết | `.base` sửa được; trang Nguyên liệu được dựng lại mỗi lần gọi |
| `_Đề xuất/` | Hàng chờ đề xuất | Chỉ đổi `status` và `reject_reason`; đừng xoá đề xuất đã bỏ |
| `_Templates/` | Mẫu note | Được |
| `_E11/` | Sổ ghi mọi thay đổi máy làm, để trả lại được | Không |
| `_ohs/lexicon.md` | Từ điển cho tìm tiếng Việt | Được ([04](04-ask-claude.md)) |

Claude, trang Nguyên liệu và các tool của `pkm` coi `_E11/`, `_Xưởng/`, `_Đề xuất/`, `_Templates/`, `_ohs/` và `Trang chủ.md` là trang hệ thống: không phải nguyên liệu, không trả về khi tìm.

## Cài đặt

Cài `pkm` và nối vault với Claude Desktop: xem [README](../../README.md), mục "Bắt đầu nhanh". Dựng bản B trên vault bằng `pkm setup VAULT --apply` (plugin, theme, chat trong Obsidian, MCP, index): xem [README](../../README.md), mục "Dựng bản B".

Lần đầu mở vault, Obsidian hỏi có tin plugin của vault không (Restricted mode). Đó là chốt an toàn của Obsidian; bấm tin thì Trang chủ và các view mới chạy.
