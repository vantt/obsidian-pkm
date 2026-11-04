# Bước 2: preset bản B và hướng dẫn người dùng

Trạng thái: xong, đã đẩy lên `main` (`6b7e48e` preset, `3c7dbf8` hướng dẫn, `494a069` plan). Nguồn: `vantt/obsidian-pkm-automation` ở `9061581`.

## Đã chép

| Đích | Nguồn (đổi lần cuối) |
|---|---|
| `presets/b/vault-agent.md` | `scripts/e11/vault-agent.md` (`04eb84f`) |
| `presets/b/folder-grid.css` | `scripts/e11/folder-grid.css` (`8caac16`) |
| `presets/b/theme/e11-theme.src.css`, `theme/fonts/*` (woff2, `fonts.css`, ba file OFL) | `scripts/e11/theme/` (`19c89e9`) |
| `presets/b/theme/e11-theme.css` (khoảng 1 MB, font nhúng) | dựng bằng `node scripts/e11/theme.mjs build` (`57563da`); dựng hai lần ra giống từng byte, sha256 `80aa6969…` |
| `presets/b/setup.json` | hằng số của `setup_vault.py` (`PLUGINS` kèm sha256, `CORE`, `ACP`, `PRESET`, `ALLOW`, `OLD_SERVERS`, cờ MCP, `ABOUT`) và `theme.mjs` (`THEME`, `FONT_SIZE`, `CONFLICTS`) |
| `presets/b/README.md` | mô tả từng file, nguồn, người dùng, cách dựng lại CSS |
| `docs/user-guide/` (README, 01–05, tổng 375 dòng) | `docs/capture-to-producer/playbook/` 03, 04, 05, 06, 02 (phần dùng); `07-vault-b-workshop.md`, `09-material-while-writing.md`; danh sách 13 tool lấy từ `tools/defs.rs` của nhánh `phase-17-one-door` |

Đặt tên `vault-agent.md`, không `CLAUDE.md`, để Claude Code không nạp nó như hướng dẫn của repo khi làm việc trong `presets/b/`.

## Schema

`crates/pkm-core/src/schema/schema.yaml` giống từng byte `docs/knowledge-graph/schema/schema.yaml` ở `9061581` (`cmp`). Không chép bản thứ hai; README của preset ghi file nhúng là gốc. Mục rủi ro "hai bản schema" trong plan đã cập nhật.

## Không chép và vì sao

- Adapter e11: là mã Rust (`crates/pkm-core/src/adapter/e11.rs`), không có phần dữ liệu thuần.
- Trang chủ, `.base`, trang chủ đề, 8 mẫu note, cấu hình Homepage và Templater: chuỗi Python trong `workshop_ui.py`, theo dặn không trích; plugin chuyển ở phase 5.
- `review-log-plugin/`, `table-zoom-plugin/`: mã JS, gộp vào plugin.
- `theme.mjs` (bộ đo tương phản, cài, gỡ): mã; cách dựng CSS ghi trong README của preset.
- Playbook 01 (setup_vault, WSL), 07 (bộ thử thách kg), phần núm vặn, số đo, golden set, `ohs.mjs`, `similar_pairs`, `note_knn`: dành cho người nghiên cứu.
- Lệnh chạy `propose.py --apply`, `material.py`: hướng dẫn nói lệnh đang chuyển vào `pkm`/plugin, tạm nhờ người cài.

## Rà nội dung thật

Grep `presets/` và `docs/user-guide/` theo đường dẫn vault (`obsidan`, `workshop-b`, `/mnt/`, `C:\`, `/home/`), số đo của vault thật, tên user: không có kết quả nào ngoài tên repo công khai. Ví dụ trong hướng dẫn đều tự đặt. Đã đọc mọi file văn bản được thêm.

## Câu hỏi còn mở

1. Theme Primary 2.10.0 và plugin Style Settings: không script nào cài, vault của user có sẵn. `pkm setup` tải ghim cả hai hay chỉ cảnh báo?
2. `TOPIC_ROOTS = ('Evernote', 'vantt')` và `INBOXES` của `workshop_ui.py` ghi cứng thư mục của vault nghiên cứu (có tên user). Bản plugin cần cho chọn thư mục gốc; hướng dẫn đã viết chung là "thư mục gốc được chọn lúc dựng vault".
3. `setup.json` là dạng tôi tự đặt cho bước 4; agent `pkm setup` có thể muốn đổi khoá. Câu `about` mặc định nói vault "chuyển từ Evernote"; khách khác nên có câu riêng.
4. README gốc vẫn ghi MCP `kg`, `rc.2`; hướng dẫn viết theo MCP `pkm` 13 tool (nhánh `phase-17-one-door`). Hai chỗ sẽ khớp khi nhánh đó merge.
