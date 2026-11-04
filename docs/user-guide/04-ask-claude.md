# 04. Hỏi Claude về vault

[← Hướng dẫn](README.md)

Nguồn: `docs/capture-to-producer/playbook/04-graph-kg.md` (mục pkm), `06-writing.md` (mục chat trong Obsidian), `02-search-ohs.md` (mục OHS và `_ohs/lexicon.md`, phần dùng) và `scripts/e11/vault-agent.md` của `vantt/obsidian-pkm-automation` ở `9061581`; danh sách tool theo `crates/pkm-core/src/tools/defs.rs` của nhánh `phase-17-one-door`. Bỏ phần chỉnh cờ, số đo và bộ thử thách. Ví dụ là hư cấu.

Claude đọc vault qua một MCP server tên `pkm` (binary `pkm` của repo này). Server gộp hai thứ: graph có loại của vault, và tìm theo nghĩa bằng OHS (obsidian-hybrid-search, chạy trên máy, hiểu cả tiếng Việt lẫn tiếng Anh). Bản cũ có hai server riêng `kg` và `ohs`; còn thấy chúng trong cấu hình thì gỡ, `pkm` đã thay cả hai.

## Hai chỗ hỏi

| Chỗ | Cài | Hợp với |
|---|---|---|
| Claude Desktop (không bắt buộc) | `pkm mcp install VAULT …` ([README](../../README.md), "Bắt đầu nhanh"). Thoát hẳn Claude Desktop, cả ở khay hệ thống, trước khi cài: app ghi đè cấu hình lúc thoát | Câu hỏi về cả vault, phiên dài |
| Chat trong Obsidian (bản B cần) | Plugin Agent Client mở Claude Code ở side panel, cùng server `pkm`, đăng ký riêng cho thư mục vault (`pkm setup` thêm). Cần Claude Code đã đăng nhập bằng tài khoản Claude của bạn. Cách trả lời nằm ở `VAULT/.claude/CLAUDE.md` | Câu hỏi về note đang mở: "bài này", "note này" là note đó |

Nội dung note Claude đọc được gửi lên Anthropic. Chỉ nối những vault bạn đồng ý cho đọc.

## Hỏi thế nào

Hỏi như nói với người, không cần gọi tên tool. Vài kiểu câu hay dùng:

- **Tìm lại:** "cái bài về cách định giá theo giá trị mình lưu năm ngoái đâu?" Gõ không dấu, Telex dở, sai chính tả vẫn được.
- **Hỏi theo ý:** "làm sao giữ được nhịp viết khi bận?" Câu nói khác chữ trong note: Claude tìm theo nghĩa.
- **Quanh một note:** "note này nối với gì? có gì phản bác ý này không?"
- **Đường đi:** "dự án A và ý B dính nhau qua đâu?"
- **Chỉ thứ mình đã duyệt:** thêm "chỉ cạnh đã duyệt" vào câu.

Câu trả lời tốt có link mở thẳng note (`[[đường dẫn|tên]]`), một câu trích từ note nguồn, và ghi rõ quan hệ nào bạn đã nối, quan hệ nào máy gợi ý.

## Hai lớp cạnh

| Lớp | Là gì |
|---|---|
| `accepted` | Bạn viết (link, thuộc tính) hoặc đã nhận trong hàng chờ |
| `proposed` | Máy gợi ý: đề xuất đang chờ, cặp gần nghĩa của OHS. Có điểm và hồ sơ ai gợi ý |

Claude được dặn tách hai lớp này. Thấy Claude coi cạnh máy gợi ý là sự thật thì hỏi lại kèm "chỉ cạnh đã duyệt", hoặc duyệt hàng chờ ([03](03-proposals.md)).

## 13 tool của `pkm`

Claude tự chọn tool. Bảng này giúp bạn đọc Claude đang làm gì và hỏi trúng hơn. Mọi tool chỉ đọc, không sửa vault.

| Tool | Làm gì |
|---|---|
| `find` | Tìm note theo tên bạn gõ (bỏ qua hoa thường và dấu) |
| `search` | Tìm theo nghĩa hoặc theo chữ qua OHS. Chế độ `hybrid` (mặc định), `semantic` cho câu hỏi theo ý, `fulltext` cho một từ chính xác, `title` khi chỉ biết tên |
| `note` | Thông tin một note: loại, thuộc tính, số cạnh theo quan hệ và lớp, đầu thân note |
| `read` | Thân đầy đủ của một hay nhiều note, để trích và để xét có đúng không |
| `neighbors` | Cạnh quanh một note, một bước |
| `reach` | Mọi note đi tới được từ một note (xuôi hay ngược) |
| `paths` | Các đường ngắn nhất giữa hai note |
| `loops` | Vòng lặp đi qua một note (vòng tăng cường, vòng cân bằng) |
| `provenance` | Một nhận định từ đâu ra: nguồn, note ủng hộ, phản bác, hồ sơ duyệt |
| `select` | Lọc note theo thuộc tính, ví dụ mọi đề xuất đang chờ do model gợi ý |
| `valid` | Note của một loại còn hiệu lực vào một ngày |
| `related` | Một lần gọi là tìm theo nghĩa rồi đi graph: từ câu hỏi (`query`) hay từ một note (`node`), đưa ra cả note tìm chữ không ra mà graph nối tới |
| `more` | Lấy tiếp phần sau của một kết quả dài |

Với vault kiểu bản B, phần lớn giá trị đến từ `search`, `read` và `related`. `valid`, `loops`, `provenance` cần dữ liệu (thời hạn hiệu lực, quan hệ nhân quả, note nhận định) mà vault ghi chép cá nhân thường chưa có; khi đó câu trả lời đúng là "vault chưa có dữ liệu đó".

## Chat trong Obsidian

Mở side panel Agent Client, hỏi về note đang mở. Theo `CLAUDE.md` của vault, Claude sẽ:

1. đọc note đang mở;
2. gọi `related` từ note đó, rồi `neighbors` để thấy cạnh đã có;
3. đọc thân 5–8 note đứng đầu trước khi xét quan hệ (điểm tìm theo nghĩa gần nhau không nói được note nào đúng), bỏ note không trích được câu nào ăn với bài;
4. trả lời theo nhóm (ủng hộ, phản bác, ví dụ cho, cùng chủ đề), mỗi note một dòng có link, lý do, câu trích;
5. không viết ý hộ bạn, không sửa note khi chưa được bảo; muốn thêm note vào `uses` của bài thì hỏi trước.

Muốn Claude trả lời khác (ngắn hơn, thêm nhóm "chỗ mâu thuẫn") thì sửa `VAULT/.claude/CLAUDE.md`, rồi mở phiên chat mới. Giữ bốn điều trên: đó là phần người làm vườn, không phải phong cách. Lần dựng lại sau không ghi đè `CLAUDE.md` đã có.

## Tìm tiếng Việt tốt hơn: `_ohs/lexicon.md`

Gõ `KH` hay `customer` mà note viết `khách hàng` thì không ra? Thêm vào từ điển `VAULT/_ohs/lexicon.md` (sửa trong Obsidian):

```markdown
## Đồng nghĩa
khách hàng = customer = người mua

## Viết tắt
KH = khách hàng

## Từ ghép
thời khoá biểu
```

Đồng nghĩa là hai chiều, viết tắt một chiều. Bỏ chữ tắt có nhiều nghĩa trong vault của bạn (ví dụ `CV`). Chữ tắt mở ra cụm quá phổ biến có thể làm tìm tệ đi: thêm một dòng thì thử lại vài câu quen, tệ hơn thì bỏ dòng đó. Từ điển có tác dụng sau khi OHS index lại.

## Khi có lỗi

| Thấy | Làm |
|---|---|
| `search`, `related` báo `ohs_not_ready` | OHS đang khởi động hay đang index: chờ. Các tool graph vẫn chạy. `pkm doctor --deep` cho biết trạng thái |
| Windows mới cài, `ohs_not_ready` với `model_missing` | Thiếu Visual C++ Redistributable: `winget install --id Microsoft.VCRedist.2015+.x64`, rồi khởi động lại Claude Desktop. `pkm doctor` báo mục `ohs_runtime` |
| `paths`, `loops` báo `query too expensive` | Hỏi hẹp lại (đường ngắn hơn, một quan hệ) |
| Note mới viết không ra khi tìm | Chờ OHS index lại, hoặc khởi động lại Claude Desktop |
| Claude hiểu "công cụ", "dự án" là tool của nó | Sửa câu mô tả vault (`--about` khi `pkm mcp install`) cho rõ vault chứa gì |
| Mọi lỗi khác | `pkm doctor`: kiểm index, cấu hình, gói OHS và in cách sửa. `pkm mcp install --check` bắt tay như Claude Desktop, phải ra 13 tool |
