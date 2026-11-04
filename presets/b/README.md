# Preset bản B

Dữ liệu để dựng một vault "bản B" (vault xưởng: lưu, rút ý, viết bài) trên máy khách. Chỉ có dữ liệu: mẫu, CSS, font, số ghim. Không có note, tên note, đường dẫn hay nội dung của vault thật nào.

Người dùng: `pkm setup` (chép preset vào vault, tải plugin ghim, cài MCP) và plugin Obsidian (giao diện xưởng, review log, nút phóng bảng). Cả hai đang viết, xem [plan](../../plans/261103-product-layout/plan.md). Hướng dẫn cho khách ở [docs/user-guide](../../docs/user-guide/README.md).

Nguồn: repo nghiên cứu `vantt/obsidian-pkm-automation` ở commit `9061581`. Cột "đổi lần cuối" là commit gần nhất chạm file nguồn tính tới `9061581`.

## Các file

| File | Là gì | Nguồn (đổi lần cuối) | Ghi vào vault ở |
|---|---|---|---|
| `vault-agent.md` | Hướng dẫn cho Claude trong chat của Obsidian (plugin Agent Client): cách trả lời về note đang mở, cách tìm, nguyên tắc người làm vườn. Đặt tên khác `CLAUDE.md` để Claude Code không nạp nó khi làm việc trong repo này | `scripts/e11/vault-agent.md` (`04eb84f`) | `.claude/CLAUDE.md`; đã có thì giữ |
| `folder-grid.css` | Lưới thẻ cho trang chủ đề (`cssclasses: folder-grid`, view thẻ của Bases) | `scripts/e11/folder-grid.css` (`8caac16`) | `.obsidian/snippets/e11-folder-grid.css`, bật trong `appearance.json` |
| `theme/e11-theme.src.css` | Snippet giao diện trên theme Primary: màu đủ tương phản, font đọc, nhịp dòng, bảng. Chỗ `/*FONTFACES*/` được thay bằng font nhúng khi dựng | `scripts/e11/theme/e11-theme.src.css` (`19c89e9`) | không ghi thẳng; dựng thành `theme/e11-theme.css` |
| `theme/fonts/` | `fonts.css` (khai báo `@font-face`) và file `woff2` của Merriweather, Archivo Narrow, Manrope (bộ `vietnamese`, `latin`, `latin-ext`), kèm giấy phép `OFL-*.txt` (SIL OFL 1.1) | `scripts/e11/theme/fonts/` (`19c89e9`) | không ghi thẳng; nhúng vào `theme/e11-theme.css` |
| `theme/e11-theme.css` | Snippet đã dựng, font nhúng base64 (khoảng 1 MB). Có sẵn để `pkm setup` chỉ việc chép, không phải dựng | dựng từ hai dòng trên bằng `node scripts/e11/theme.mjs build --out …` của repo nghiên cứu (`theme.mjs` đổi lần cuối `57563da`) | `.obsidian/snippets/e11-theme.css`, bật trong `appearance.json` |
| `setup.json` | Số ghim và giá trị cấu hình mà bước cài cần (xem mục dưới) | hằng số của `scripts/e11/setup_vault.py` (`04eb84f`) và `scripts/e11/theme.mjs` (`57563da`) | không ghi vào vault |

### Dựng lại `theme/e11-theme.css`

Phải dựng lại mỗi khi sửa `e11-theme.src.css` hay `fonts/`, trong cùng commit. Cách dựng (như hàm `build()` của `theme.mjs`): đọc `e11-theme.src.css`, thay chuỗi `/*FONTFACES*/` đầu tiên bằng nội dung `fonts/fonts.css` trong đó mỗi `url(<file>)` (bỏ dấu nháy) đổi thành `url(data:font/woff2;base64,<nội dung file>)`; file không có trong `fonts/` thì báo lỗi. Kết quả không phụ thuộc máy: hai lần dựng ra giống nhau từng byte. Bản trong repo có sha256 `80aa6969a25c2d50c2620fd49492eacfd61ad9dc49f8b5d99e56fc1d83eea6fe`.

### `setup.json`

| Khoá | Ý nghĩa | Từ |
|---|---|---|
| `plugins` | Plugin cộng đồng ghim: repo GitHub, phiên bản, sha256 của từng file release (`main.js`, `manifest.json`, `styles.css`). Tải từ `https://github.com/<repo>/releases/download/<version>/<file>`, sai hash thì không ghi gì. Plugin đã có trong vault thì giữ, khác phiên bản chỉ báo | `PLUGINS` |
| `core_plugins` | Core plugin phải bật trong `core-plugins.json` (Bases) | `CORE` |
| `agent` | Adapter ACP cho chat trong Obsidian (gói npm và phiên bản), preset Claude Code của Agent Client, mẫu `CLAUDE.md` và tool được dùng không hỏi (`.claude/settings.local.json`, mọi tool của `pkm` chỉ đọc) | `ACP`, `PRESET`, `PRESET_DEFAULT`, `AGENT_GUIDE`, `ALLOW` |
| `mcp` | Tên server MCP, các mục cũ nó thay, cờ đọc vault của bản B, câu mô tả vault (`--about`). Câu `about` là mặc định cho vault chuyển từ Evernote; khách nên sửa cho đúng vault của mình | `OLD_SERVERS`, `pkm_server()`, `ABOUT` |
| `snippets` | Tên snippet trong `.obsidian/snippets/` và file preset tương ứng | `GRID_SNIPPET` của `workshop_ui.py`, `SNIPPET` của `theme.mjs` |
| `theme` | Snippet viết cho theme Primary 2.10.0; cỡ chữ thân đặt khi cài (`baseFontSize` của `appearance.json`); plugin phải tắt vì ghi đè cỡ chữ | `THEME`, `FONT_SIZE`, `CONFLICTS` của `theme.mjs`; phiên bản Primary ghi ở đầu `e11-theme.src.css` |

## Schema

Bản B đọc gói `capture` của schema v3. Schema không có bản riêng ở đây: nguồn là [`crates/pkm-core/src/schema/schema.yaml`](../../crates/pkm-core/src/schema/schema.yaml), nhúng vào binary `pkm` lúc build. Ngày 2026-11-04 file đó giống từng byte `docs/knowledge-graph/schema/schema.yaml` của repo nghiên cứu ở `9061581` (đổi lần cuối `33ddf9a`). Từ 2026-11-03 repo này là nguồn sự thật của lõi, nên sửa schema thì sửa file nhúng; `pkm setup` không chép schema vào vault.

## Adapter e11

Không có dữ liệu riêng. Adapter bản B (cạnh thư mục, đề xuất, cặp OHS, trang hệ thống) là mã Rust ở [`crates/pkm-core/src/adapter/e11.rs`](../../crates/pkm-core/src/adapter/e11.rs), chọn bằng cờ `--adapter e11` (đã có trong `setup.json`).

## Chưa có ở đây và vì sao

| Thứ | Ở đâu bây giờ | Vì sao chưa chép |
|---|---|---|
| Trang chủ, `_Xưởng/Xưởng.base`, `_Xưởng/Chủ đề.base`, trang chủ đề, 8 mẫu note trong `_Templates/`, cấu hình Homepage và Templater | Chuỗi trong mã `scripts/e11/workshop_ui.py` (`TEMPLATES`, `BODIES`, `PROPS`, `QUICK`, `workshop_base()`, `topic_base()`, `homepage_config()`), không phải file | Plugin chuyển cả `workshop_ui` sang TS; mẫu và trang tĩnh ra file ở bước đó |
| Plugin `e11-review-log` (ghi lần mở note, lần đổi trạng thái đề xuất) và `e11-table-zoom` (nút phóng bảng) | `scripts/e11/review-log-plugin/`, `scripts/e11/table-zoom-plugin/` | Là mã JS, gộp vào plugin Obsidian của sản phẩm |
| Theme Primary và plugin Style Settings | Không script nào cài; vault của user có sẵn | Snippet chỉ đúng trên Primary; chọn font đọc cần Style Settings. Bước cài phải quyết: tải ghim cả hai, hay chỉ cảnh báo |
| `normalize`, `migrate_v3`, `highlight_colours`, `selection_signals` | `scripts/e11/` | Chỉ dành cho vault chuyển từ Evernote, đóng băng (docs/DECISIONS.md, 2026-11-03) |
