# Luật cho agent làm việc trong repo này

Áp dụng cho mọi agent (Claude Code, Codex, Gemini, ...) sửa `obsidian-pkm`: binary `pkm` (CLI, MCP), plugin Obsidian `pkm`, bộ cài, preset bản B.

## Mỗi thay đổi người dùng thấy được phải có đủ ba thứ

Người dùng thấy được = lệnh mới, nút, cài đặt, hành vi đổi, thông báo, công cụ MCP, cờ CLI. Sửa nội bộ không đổi hành vi thì chỉ cần unit test.

1. **Unit test** cho phần tính toán, tách khỏi Obsidian (như `apps/obsidian-plugin/src/compare-scroll/session.ts`).
2. **Test e2e chạy tự động** trên app thật, không chỉ trên mock:
   - Plugin: script chạy trong Obsidian thật qua `obsidian eval` (Obsidian CLI), trên **vault thử có dữ liệu bịa**, đọc trạng thái thật (DOM, vị trí cuộn, file đã ghi) và so với kỳ vọng. Không chạy trên vault thật của người dùng; không để lại plugin, file hay cấu hình thừa.
   - CLI và MCP: chạy binary đã build trên vault bịa, so đầu ra.
   - Chờ đủ lâu cho Obsidian vẽ xong trước khi đo (chế độ đọc vẽ dần note dài và tự chỉnh vị trí cuộn).
   - Ghi trong commit: đã chạy e2e ở đâu, kết quả.
3. **Hướng dẫn sử dụng** trong `docs/user-guide/`, cùng commit:
   - Viết cho người dùng, không cho người phát triển: dùng để làm gì (ví dụ đời thường), các bước đánh số, mỗi bước nói bấm gì và **thấy gì** (chữ trên nút, thông báo, thanh trạng thái), điều nên biết, cách tắt.
   - Đủ rõ để quay thành video hướng dẫn và dùng khi bán hàng.
   - Ví dụ hư cấu; không tên note, nội dung hay tên người từ vault thật.

Thiếu một trong ba thì việc chưa xong: không báo xong, không merge.

## Mã lấy từ người khác

- Chỉ lấy mã có giấy phép cho phép (MIT, BSD, Apache...). Ghi công ở đầu file (tên dự án, link, giấy phép, tác giả) và mang theo toàn văn giấy phép vào bản phát hành (ví dụ hiện trong trang cài đặt), vì `legalComments: 'none'` bỏ comment khỏi `main.js`.
- Tìm ra lỗi hay cải tiến chung trong mã đó thì gửi lại cho tác giả (issue, PR từ fork dưới tài khoản `vantt`), người dùng duyệt lời trước khi gửi. Ghi link PR trong mã.

## Theo dõi dự án nguồn (distill)

Plugin, theme mà `pkm setup` ghim, mã chép vào plugin `pkm` và OHS gốc được theo dõi ở [docs/distillery](docs/distillery/distill.yaml) bằng skill `distill-repo` (script `distill.mjs`). Bài học nằm ở `docs/distillery/lessons/<domain>.yaml`, mỗi bài ghim tới `nguồn@commit:đường dẫn#dòng`.

- Trước khi nâng phiên bản ghim trong `presets/b/setup.json`, hay sửa mã chép từ nguồn khác: chạy `delta <nguồn>`, đọc thay đổi từ cursor, ghi bài học, `seal`.
- Bản sao nguồn ở `upstreams/` (gitignore); thiếu thì `git clone --filter=blob:none <url> upstreams/<nguồn>`.
- Chỉ người dùng chốt bài học (`planned`, `rejected`...). Bài học không tự thành việc; muốn làm thì đưa vào plan như việc khác.
- PR đã gửi cho nguồn (ví dụ relative-scroll-lock PR 1) ghi trong `notes` của nguồn; mỗi lượt delta xem PR đó đã merge chưa.
