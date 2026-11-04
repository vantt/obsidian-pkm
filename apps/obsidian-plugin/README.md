# Plugin Obsidian `pkm`

Plugin lo phần giao diện và hành vi trong Obsidian. Việc tính trên cả vault (index, graph, đề xuất, nguyên liệu, vùng) là của binary `pkm`: plugin gọi `pkm … --json` rồi hiện kết quả, không tự tính nặng. Một tag ra cả binary và plugin cùng phiên bản.

Chỉ chạy trên máy tính (`isDesktopOnly`), vì plugin chạy binary `pkm`.

## Có gì

| Lệnh (Ctrl+P) | Việc |
|---|---|
| PKM: Dựng xưởng | Ghi Trang chủ, `_Xưởng/Xưởng.base`, `_Xưởng/Chủ đề.base`, mẫu trong `_Templates/`, trang chủ đề cho mỗi thư mục chủ đề, thư mục `Bài viết/`, `Ý/`, `Inbox/`; đặt cấu hình Homepage và Templater. Hỏi trước khi ghi. |
| PKM: Gỡ xưởng | Xoá những gì Dựng xưởng đã ghi, trả cấu hình về như trước. |
| PKM: Kiểm tra pkm (doctor) | Chạy `pkm doctor --json`, báo mục lỗi và cách sửa. |
| PKM: Phóng hoặc thu mọi bảng trong note đang mở | Như bấm nút phóng ở từng bảng. |
| PKM: Ghi báo cáo nút phóng bảng | Ghi `_E11/table-zoom-report.json`: bề rộng từng bảng, để soát bố cục mà không cần chụp màn hình. |

Chạy ngầm, không cần lệnh:

- **Nhật ký duyệt:** mỗi lần mở note và mỗi lần `status` của một đề xuất trong `_Đề xuất/` đổi, thêm một dòng vào `_E11/review-log.jsonl`. Không gửi gì ra ngoài vault.
- **Nút phóng bảng:** góc trên phải mỗi bảng (Markdown, Bases, Dataview) có nút kéo bảng ra hết chiều ngang khung; bấm lại hoặc Esc để thu.
- **Lưới thư mục:** trang có `cssclasses: folder-grid` (trang chủ đề) hiện note thành thẻ có ảnh bìa.

### Dựng xưởng không đè file bạn đã sửa

Mỗi lần ghi, plugin lưu mã băm của từng file vào `_E11/workshop-ui-manifest.json`. Lần sau:

- File bạn đã sửa (nội dung khác mã băm) được giữ nguyên và liệt kê ra. File `.base` mà Obsidian chỉ viết lại theo kiểu của nó (đổi thứ tự khoá, dấu nháy) không tính là sửa.
- Nếu một file sắp ghi đã có sẵn mà không do xưởng tạo, plugin không ghi gì cả và nói file nào. Đổi tên hoặc chuyển nó đi rồi chạy lại.
- File xưởng từng ghi mà nay không còn dùng thì bị xoá, trừ khi bạn đã sửa.
- Ghi vào note đang mở đi qua API của Obsidian (`vault.process`), xoá đi qua thùng rác theo cài đặt của Obsidian.

Manifest dùng chung định dạng với script `workshop_ui.py` cũ, nên vault đã dựng bằng script đó chạy Dựng xưởng được ngay: file script ghi mà bạn chưa sửa được cập nhật; plugin `e11-review-log` và snippet `e11-folder-grid.css` cũ bị xoá (nếu bạn chưa sửa) vì plugin này đã gồm cả hai; `e11-review-log`, `e11-table-zoom` bị bỏ khỏi danh sách plugin bật. Sau Dựng xưởng, tải lại Obsidian (Ctrl+R) để Homepage và Templater nhận cấu hình mới.

### Theme

Theme (`e11-theme.css`, phông nhúng sẵn, viết cho theme Primary) không nằm trong plugin. Nó là CSS snippet do `pkm setup` chép từ `presets/b/`, bật tắt riêng trong Cài đặt > Giao diện. Lý do: phông nhúng (base64) nặng khoảng 1 MB, chỉ hợp với theme Primary, và người dùng cần tắt được theme mà không mất plugin. Plugin chỉ mang CSS của chính các thành phần nó vẽ (lưới thư mục, nút phóng bảng) trong `styles.css`. Vì nút phóng bảng đã ở trong plugin, bộ cài không cần cài plugin `e11-table-zoom` nữa.

## Cài đặt (Cài đặt > PKM)

| Mục | Mặc định | Ý nghĩa |
|---|---|---|
| Đường dẫn tới pkm | trống | Trống thì tìm chỗ bộ cài đặt: `%LOCALAPPDATA%\pkm\bin\pkm.exe` trên Windows, `~/.local/bin/pkm` trên macOS và Linux (hoặc thư mục trong biến `PKM_INSTALL_DIR`). Nút Kiểm tra chạy `pkm --version`. |
| Thời gian chờ pkm | 90 giây | Quá hạn thì dừng `pkm` và báo lỗi. |
| Thư mục chủ đề | trống | Mỗi dòng một thư mục gốc. Mỗi thư mục `gốc/nhóm/chủ đề` (tên không bắt đầu bằng `_`) có từ 3 note trở lên được một trang chủ đề. Trống thì không có trang chủ đề. |
| Thư mục lưu nhanh khác | trống | Thêm vào mục "Mới lưu" trên Trang chủ, cùng với `Inbox`. |

Lỗi khi gọi `pkm` hiện thành thông báo tiếng Việt: không thấy `pkm`, không có quyền chạy, quá thời gian chờ, `pkm` báo lỗi, và `ohs_not_ready` (tìm theo nghĩa đang khởi động, đang lập chỉ mục bao nhiêu phần trăm, còn bao lâu, việc cần làm).

## Phát triển

Dùng pnpm workspace ở gốc repo (`pnpm-workspace.yaml`). Node 20 trở lên.

```sh
pnpm install                               # ở gốc repo
pnpm --filter pkm-obsidian-plugin test     # unit test (vitest)
pnpm --filter pkm-obsidian-plugin build    # kiểm phiên bản, typecheck, ra dist/
```

Trong `apps/obsidian-plugin/` chạy thẳng `pnpm test`, `pnpm run build`, `pnpm run typecheck`, `pnpm run check-version`.

`build` ra `dist/main.js`, `dist/manifest.json`, `dist/styles.css` (không commit). Cài thử vào một vault: chép ba file đó vào `VAULT/.obsidian/plugins/pkm/`, rồi bật plugin PKM trong Cài đặt > Plugin cộng đồng.

`check-version` dừng build nếu `version` trong `manifest.json` hoặc `package.json` khác `version` của `[workspace.package]` trong `Cargo.toml` ở gốc. Đổi phiên bản thì sửa cả ba chỗ.

### Bố cục

| Đường dẫn | Việc |
|---|---|
| `src/main.ts` | Plugin: lệnh, cài đặt, nạp các phần dưới |
| `src/bridge.ts` | Chạy `pkm <args> --json` có hạn giờ, đọc JSON, đổi lỗi thành câu tiếng Việt |
| `src/pkm-path.ts` | Chỗ bộ cài đặt để `pkm` |
| `src/workshop/templates/` | Mẫu tĩnh của xưởng (gói vào `main.js` dạng chữ); JS chỉ điền biến `{{…}}` |
| `src/workshop/plan.ts` | Danh sách file sẽ ghi, tìm thư mục chủ đề |
| `src/workshop/apply.ts` | Ghi, gỡ, manifest, không đè file đã sửa |
| `src/review-log.ts`, `src/table-zoom.ts` | Nhật ký duyệt, nút phóng bảng |
| `test/` | Unit test; `test/obsidian-mock.ts` thay module `obsidian` (gói npm chỉ có kiểu) |

`test/fixtures/python-workshop.json` là đầu ra của `workshop_ui.py` (repo nghiên cứu, commit `9061581`) trên một vault mẫu trung tính; test so từng byte để giữ kết quả giống bản Python.
