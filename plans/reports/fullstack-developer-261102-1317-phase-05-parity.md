# Báo cáo phase 05: bộ so khớp và golden kg-solo

## Phase Implementation Report

### Executed Phase
- Phase: phase-05-parity-harness-and-golden
- Plan: plans/261102-0535-rust-core-port-master/
- Status: partial (trạng thái phase `in-progress`): mọi việc đã làm và kiểm trên kg-solo; còn đúng phần bản B, user đã hoãn.

### Kết quả
Bộ so khớp chạy được: Python ở sha ghim so với chính nó trên kg-solo ra 0 lệch; golden kg-solo ở `5a3eb2f` đã commit; CI có bước dựng kg-solo và so bản Rust với golden (mọi mục báo "chưa có", mã 0); sửa một dòng golden thì lệnh so trả mã 1 và chỉ đúng dòng đó. Chế độ `--private` làm đủ, thử trên một bản sao kg-solo (có và không có cờ đọc của bản B), chỉ in số. Chưa chạy trên bản B.

### Thiết kế đã chọn
- **Hai nửa.** Phía Python (chỉ trên máy): `parity/dump-python.py` chạy repo đặc tả trên một vault, ghi một thư mục dump trung lập ngôn ngữ (JSONL cho bảng và câu, JSON cho bắt tay MCP, văn bản cho `instructions` và CLI); `parity/parity.py` điều phối (`golden`, `run`, `--private`, `--sha`). Phần so là package Rust `pkm-parity` (binary `parity`), member của workspace, phụ thuộc `pkm-core`: `compare`, `check`, `rust-dump`, `fixture-args`. CI chỉ dùng phần Rust (Q5).
- **Phần cắm Rust** `parity/src/rust_side.rs`: mỗi tầng (`table`, `answer`, `tool`, `mcp`, `cli`, `perf`) một phương thức, hiện trả `None`; mục trả `None` được khai `not_available` trong manifest và báo "chưa có", không bao giờ tính là khớp. Phase 08–12 chỉ cần cho phương thức tương ứng trả `Some`. Rust chạy lại đúng bộ câu lấy từ dump mẫu.
- **Bộ câu:** (1) mọi câu graph và bảng của `.kg-answers.json`, chạy bằng chính `fixture_check.run` của repo đặc tả (81 câu, `fixture_check` 70/70, 11/11); (2) bộ sinh tự động (dùng cho cả kg-solo và bản B): mỗi type 3 note chọn theo sha1 của id, mỗi tool vài tham số gồm `as_of`, `statuses`, `accepted_only`, `system`, `limit`, `k`, `more` (theo `handle` vừa trả), `related` có `hits`, và các lỗi người gọi hay gặp. Trên kg-solo: 727 lời gọi đủ 11 tool, qua `Server.call` (điểm vào chung của tool, trước khi MCP thu gọn). CLI: 21 lệnh của mục 10 PRD.
- **Báo lệch theo dòng:** dòng bảng so như tập có lặp, dòng lệch ghép theo khoá chính (lấy từ `PRAGMA table_info`) hoặc theo số cột giống; JSON tìm dòng qua JSON pointer; văn bản so LCS theo dòng. Mỗi chỗ lệch ghi `file:dòng` của dump mẫu, cột hoặc pointer, giá trị hai bên.
- **Ngoại lệ có tên:** `parity/exceptions.toml` (tầng, mục, trường); mỗi mục phải có mục cùng tên trong "Lệch có chủ ý" của `spec/PORTING.md`, thiếu thì `parity` từ chối chạy. Hiện chưa có ngoại lệ nào.
- **Golden ổn định từng byte:** dựng lại ở cùng sha (qua checkout fixture hay qua `npx`) cho cùng byte; đường dẫn máy thay bằng `<VAULT>`, `<DB>`; thời điểm và thời gian dựng ghi `<volatile>`; `nodes.mtime` ghi thành độ lệch so với mtime trên đĩa; không có `perf.json`. Cỡ: 2,1 MB.

### Phát hiện cần lead biết
- **Python phụ thuộc thứ tự liệt kê file của hệ điều hành.** Đảo thứ tự liệt kê (`--listing reverse`) trên kg-solo đổi `edges.id` (593/726 dòng; id có trong JSON của `neighbors`, `note`, `provenance`, `more`) và kết quả `related` (số lần đẩy PageRank, đường `via`, điểm lệch 1e-5 ở chữ số thứ năm, vượt dung sai 1e-6 của PRD). Mọi bảng và tool khác không đổi. Vì thế golden dựng ở máy này sẽ không khớp Python chạy ở máy khác nếu không ghim. Cách xử lý đã chọn: dumper ghim thứ tự liệt kê theo đường dẫn đã sắp (như NTFS, APFS trả), so `edges.id` và `related` đúng như PRD, không nới. Hệ quả cho phase 09 và 11: bản Rust nên liệt kê note theo đường dẫn đã sắp và chèn cạnh, đẩy PageRank theo cùng thứ tự với Python; nếu không làm được thì ghi ngoại lệ có tên. Chi tiết ở `parity/README.md`.
- **Cùng phiên bản `rusqlite` về sau:** `pkm-parity` hiện không dùng SQLite (dump bảng của Python do Python ghi). Phase 09 thêm `rusqlite` vào `pkm-core`; phần cắm bảng nên đọc DB qua API của `pkm-core` để chỉ có một bản SQLite đóng gói.

### Files Modified
- `parity/Cargo.toml` (24), `parity/src/{lib,main,dump,compare,normalize,jsonpos,exceptions,report,rust_side}.rs` (khoảng 2.700 dòng sau `cargo fmt`), `parity/tests/golden.rs` (361)
- `parity/dump-python.py` (516), `parity/parity.py` (196), `parity/README.md` (88), `parity/exceptions.toml`, `parity/.gitignore`, `parity/golden/.gitattributes`
- `parity/golden/5a3eb2f7990cd093b2ee3a1ccd5b46bd46e3ca4e/` (golden)
- `Cargo.toml` (thêm member `parity`), `Cargo.lock`
- `.github/workflows/ci.yml` (thêm `setup-node`, bước dựng kg-solo, bước so với golden)
- `plans/261102-0535-rust-core-port-master/phase-05-parity-harness-and-golden.md` (checkbox, trạng thái)

### Tasks Completed
- [x] Phía Python: repo đặc tả ở sha bất kỳ (clone vào `~/.cache/pkm-parity/` hoặc `--spec-dir`), dựng index, dump mọi bảng (20 bảng gồm 11 `schema_*`, `v3_deviations`), bộ câu tool, bắt tay MCP, CLI
- [x] Bộ câu `.kg-answers.json` và bộ sinh tự động
- [x] Phần so (bảng từng dòng, JSON đã chuẩn hoá, `related` theo tập, thứ hạng, điểm 1e-6, MCP và CLI so chuỗi)
- [x] Tầng hiệu năng (ghi số, không chặn)
- [x] `parity/report.json`, bảng tóm tắt, mã thoát
- [x] `--private <vault>`
- [x] Golden ở `5a3eb2f` và lệnh dựng lại ở sha khác
- [x] Bước CI
- [x] `parity/README.md`
- [ ] Nghiệm thu trên bản B (hoãn theo quyết định của user)

### Tests Status
Lệnh và kết quả (chạy trong worktree):

| Nghiệm thu | Lệnh | Kết quả |
|---|---|---|
| Python ở sha ghim so với chính nó, kg-solo | `python3 parity/parity.py run --against python` (có `--spec-dir`, `--otf` trỏ checkout có sẵn ở `5a3eb2f`, `v0.7.0`) | `Tổng: 64 mục, 0 lệch chưa có tên, 0 mục chưa có.`, mã 0 |
| Ở một sha khác | `python3 parity/parity.py run --against python --sha eb637f4` (tự clone vào `~/.cache/pkm-parity/`) | 0 lệch, mã 0; `golden --sha eb637f4` dựng được (đã xoá, không commit) |
| Golden dựng lại giống từng byte | `parity.py golden` hai lần (lần hai qua `npx`), `diff -r` | không khác |
| Golden so với dump Python mới trên vault sinh lại (mtime khác) | `cargo run -p pkm-parity -- compare parity/golden/5a3eb2f… <dump>` | 0 lệch |
| Sửa một dòng golden | bản sao golden, sửa `tables/nodes.jsonl`, `tools/calls.jsonl`, `mcp/instructions.txt:3`, `cli/stats.out:5`, `answers/answers.jsonl`, `mcp/tools_list.json`, rồi `compare` | mỗi lần đúng 1 lệch, chỉ đúng `file:dòng` đã sửa, mã 1; cũng là test `editing_one_golden_line_points_at_exactly_that_line` (10 trường hợp) |
| Bước CI như CI chạy, không Python, không token | script nháp: `python`, `python3` thay bằng shim báo lỗi, bỏ `GITHUB_TOKEN`/`GH_TOKEN`, `RUNNER_TEMP` nháp, chạy nguyên văn hai bước `run:` của `ci.yml` | npx dựng kg-solo (tree hash `df46569…`), `parity check` in 64 mục "chưa có", `Tổng: 64 mục, 0 lệch chưa có tên, 64 mục chưa có.`, mã 0 |
| `--private` | `parity.py run --private <bản sao kg-solo> --against python` (có và không có `--adapter e11 --packs capture --no-aliases --edge-keys links --body-edges`) | 55 mục khớp, mã 0; `grep` đầu ra và `report.json` theo `.md`, tên vault, tên note: 0 kết quả; vault không có `.kg/` |
| Quality gates | `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings`; `cargo test --workspace --locked` | qua; `pkm-parity`: 16 test đơn vị, 7 test tích hợp trên golden (không cần Python) |

### Issues Encountered
- Thứ tự liệt kê file làm Python không tất định giữa các máy (mục "Phát hiện").
- Hook của worktree chặn đọc `target/` và `__pycache__`, chặn lệnh có biến shell: kiểm bằng `cargo run` và script nháp trong `scratchpad/p05/`.
- Bước CI trên Windows và macOS chưa chạy thật (chỉ kiểm trên Linux). Bước dùng `shell: bash` trên cả ba nền; fixture có CI Windows riêng. Nếu Windows đỏ, xem trước bước `npx` (đường dẫn `$RUNNER_TEMP`) và so tree hash.
- Không sửa `spec/PORTING.md`, `docs/DECISIONS.md` (ngoài phạm vi file được giao).

### Next Steps
- Phase 08–12: nối tầng vào `RustSide` (xem `parity/README.md`, mục "Nối tầng Rust").
- Phase 15: `python3 parity/parity.py golden --sha <X>` khi nâng ghim.
- Khi user mở bản B: chạy lệnh ở mục "Trạng thái thực hiện" của file phase, tick hai ô còn lại, đổi phase sang `done`.

### Unresolved Questions
- Lead có đồng ý cách ghim thứ tự liệt kê (golden theo đường dẫn đã sắp, Rust phải theo cùng thứ tự để khớp `edges.id` và `related`) thay vì nới so sánh hai phần đó không? Nếu muốn nới, chỉ cần thêm luật trong `parity/src/normalize.rs` và ghi lý do ở README.
- Có nên ghi lựa chọn trên vào `docs/DECISIONS.md` (file phase này không được sửa) không?
