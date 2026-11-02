# Sửa theo báo cáo thử Windows của v0.1.0-rc.1

Ngày 2026-11-02, giờ Asia/Saigon. Đầu vào: [báo cáo Windows](./windows-tester-261102-1816-windows-install.md) (đã đưa lên `main` bằng cherry-pick commit `0bb85c6`).

## Đã sửa

| Mục báo cáo | Sửa |
|---|---|
| 4.4 `Scrub` của `parity/dump-python.py` không che đường dẫn vault Windows dạng JSON | `Scrub` thêm dạng đã thoát JSON (`json.dumps(p, ensure_ascii=False)[1:-1]`, `\` thành `\\`) của mỗi đường dẫn, bỏ trùng, vẫn xếp đường dài trước; giống `Scrub::new` phía Rust. Trên Linux hai dạng trùng nhau nên golden không đổi byte nào. Thử với `ntpath` giả lập Windows: `{"vault": "C:\\…\\kg-solo", "db": "C:\\tmp\\cli.db"}` thành `<VAULT>`, `<DB>` |
| 4.2 in tiếng Việt ra pipe sập (cp1252) | `parity.py`, `dump-python.py` (cả `main` lẫn `kg-cli`) và `probes/s1-yaml/pyyaml-dump.py` đặt stdout, stderr về UTF-8 bằng `reconfigure` ngay đầu `main`. Thử bằng `PYTHONIOENCODING=cp1252`: lệnh `print` trần sập như trên Windows, ba script thì chạy hết và in đúng |
| 4.1 probe không thấy PyYAML cài `--user` | `probes/s1-yaml` gọi Python với `-E -P` thay `-I`. `-I` gồm `-E -s -P`; `-s` mới là cờ bỏ site người dùng. `-E` vẫn bỏ mọi biến `PYTHON*`, `-P` (Python 3.11 trở lên, handoff đã đòi 3.11) vẫn không đưa thư mục script và cwd vào `sys.path`, nên không có gì được import từ vault. Site người dùng do chính người dùng cài, không phải dữ liệu lạ. Dòng `Usage` của `pyyaml-dump.py`, `make-cases.py` đổi theo |
| 4.3 và các chỗ handoff sai | Handoff: clone spec bằng `git -c core.autocrlf=false clone …` kèm lý do (`schema_digest`); `git clone https://…` thay `gh repo clone`; cách chạy bộ cài bằng `-File` khi `| iex` bị chặn; bản Store 4.4212 ghi cấu hình ở `%APPDATA%\Claude\` và không có `mcp-server-kg.log`; `npx` hỏng thì dùng clone `obsidian-test-fixture` và `--otf`; PyYAML cài kiểu nào cũng được, không cần `-X utf8` |

## Trạng thái plan

Đánh dấu xong (ô và `status: done`, dòng "Cổng" ghi bằng chứng): phase 02, 03, 05, 08, 09, 11. Phase 10 và 13 vẫn mở, bảng "Trạng thái thực hiện" của `plan.md` ghi lý do. Ô phase 03 ("server thử trong Claude Desktop") đánh dấu theo đúng thay thế handoff đã ghi: chính `pkm` chạy trong Claude Desktop thay cho hai server thử.

## Kiểm

- Bộ so khớp như CI (kg-solo từ `npx github:vantt/obsidian-test-fixture#v0.7.0 generate --preset kg-solo --seed 11`, `--dump-out` thư mục tạm): 64 mục, 0 lệch chưa có tên, 0 mục chưa có.
- `python3 parity/parity.py golden`: tự so 64 mục 0 lệch, golden dựng lại không đổi file nào trong git.
- `python3 parity/parity.py run` (qua pipe): mã 0.
- `probes/s1-yaml compare --python python3` trên kg-solo: 746 note, 0 lệch ở mọi ứng viên; `cargo test`, `fmt`, `clippy` của probe qua.

## Còn mở

1. Phase 10: chạy lại `python parity\parity.py run --private <bản B> …` trên Windows để thấy tầng CLI 0 lệch với `Scrub` mới.
2. Phase 13: thử bộ cài trên một máy Windows không có Python để đóng nghiệm thu "Windows sạch".
3. Mục 4.7: dòng `ohs_node` của `pkm doctor` in lẫn `\` và `/` (`…\pkm\ohs\node/node.exe`). Lỗi hiển thị trong crate Rust, ngoài phạm vi lượt này (agent khác đang đưa thay đổi crate vào `main`).
4. Nhận diện Claude Code cài bằng winget vẫn chưa thử được (máy thử cài Claude Code kiểu khác).
5. Trong lúc làm, `main` nâng ghim OHS lên `v0.17.2-vantt.4`; mục A5 và B3 của handoff vẫn ghi `vantt.3` cùng sha256 cũ. Người đưa bản ghim mới vào nên sửa hai mục đó trước lần thử Windows sau.
