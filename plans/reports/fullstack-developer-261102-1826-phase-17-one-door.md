# Phase 17 (một cửa): MCP `pkm` có `search`, `read`, `related` theo câu hỏi

Ngày 2026-11-02. Nhánh `phase-17-one-door` (từ `889f0cd`, đã gộp `origin/main` tới `583f5b8`), đã đẩy lên origin. Không merge vào main, không tag, không release.

Làm theo thiết kế chữ ký user đã chốt (hai báo cáo brainstormer và kongming, cộng 13 quyết định của controller). Phần nền (ghim OHS `vantt.4`, `Ohs::read`, ghi lại giao thức stdio) làm ở đợt trước, mục cuối báo cáo này.

## Đã làm

| Commit | Nội dung |
|---|---|
| `cb27795` fix(mcp) | `compact()` không bỏ khoá của `ohs_not_ready`: luôn đủ năm khoá, `progress`/`eta_s` là `null` ngoài `indexing`. Test ở tầng MCP. |
| `daf2c5f` feat(tools) | `search` (chuyển tiếp OHS, tham số chọn lọc theo tên MCP của OHS, kết quả gọn có `type`), `read` (đọc thẳng vault, lỗi theo từng mục, cắt đều theo `--max-chars`), `related` nhận `query`/`queries`/`node`/`hits`, `snippet` cho hạt giống, `seed_source` mới, `ohs_error`, chờ 8 s khi OHS `starting`. CLI `pkm VAULT read`. |
| `81c3b6b` feat(mcp)! | Server tên `pkm`; `instructions` dòng đầu "(pkm)", dòng cuối theo văn của kongming (chỉ dùng `pkm`, cách chọn tool). |
| `a5e0d70` feat(install)! | Bộ cài ghi mục `pkm`, đổi tên mục `kg` cũ do pkm ghi tại chỗ, giữ `kg` của Python, không đụng `ohs`; làm ấm index OHS sau khi cài (`--no-warm`); `--remove` xoá cả `kg` cũ do pkm ghi. Doctor: `kg_entry` kiểm `pkm` (mục `kg` cũ là `warn` kèm lệnh cài lại), mục mới `ohs_mcp` cảnh báo khi còn MCP `ohs` riêng. |
| `9a117ff` test(parity) | Bốn ngoại lệ có tên `mot-cua-*` và mục "Lệch có chủ ý" tương ứng ở `spec/PORTING.md`. |
| `c549660` docs | `docs/MCP-CHANGELOG.md` (mới), bốn mục `docs/DECISIONS.md`, README (dòng cài và tên MCP), phase 17 (checklist, thiết kế đã chốt, điều kiện xem lại code mode). |
| `e0a2d83` merge | Gộp `origin/main` (cài OHS dùng chung `--ohs DIR`, ghim `vantt.4` đã vào main). Làm ấm dùng gói `--ohs DIR` khi có. |
| `4305dc9` test(ohs) | Test với gói OHS thật: `search`, `search(path)`, `related(query)`, `related(node)`. |

## Hợp đồng đổi (chi tiết ở `docs/MCP-CHANGELOG.md`)

- Tên server và mục cấu hình: `kg` → `pkm`.
- 13 tool: `find`, `search`, `note`, `read`, `neighbors`, `reach`, `paths`, `loops`, `provenance`, `select`, `valid`, `related`, `more`. Mười tool còn lại giữ nguyên chữ (test so từng chữ với golden).
- `search(query, queries, path, mode, scope, tag, limit=10, snippet_length=200, system)` → `{count, mode, items: [{id, title, type, score, snippet}]}`.
- `read(nodes, max_chars=4000, offset=0)` → `{items: [{id, title, type, url, chars, truncated, body} | {id, error, did_you_mean}]}`; không cần OHS.
- `related`: thêm `query` (đường chính), `queries`, `node`; ưu tiên `hits` > `node` > câu hỏi; `seed_source` = `query` | `node` | `hits`; lỗi mới `related needs query, queries, node or hits`, `ohs_error`, `search found no notes to start from`.
- `ohs_not_ready` luôn đủ năm khoá ở tầng MCP (sửa lỗi).
- `doctor --json`: thêm mã mục `ohs_mcp`; giữ mã `kg_entry` (hợp đồng mã ổn định).

## Kiểm

- `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` (410 test): qua, trên cây đã gộp main.
- Bộ so khớp kg-solo (seed 11): 64 mục, 0 lệch chưa có tên; lệch có tên: `initialize` 1, `tools_list` 112, `instructions` 2, `tools/related` 1.
- Gói OHS thật linux-x64 `vantt.4` trên kg-solo (`#[ignore]`, `PKM_OHS_DIR`, `PKM_OHS_TEST_VAULT`): `ohs_real` (search song song, `Ohs::read`) và `related_ohs_real` (`related` theo câu hỏi và theo note, `search`, `search(path)`) qua; không còn tiến trình `node`.
- Cỡ `tools/list` (JSON gọn): 17.664 ký tự không có trang hệ thống, 19.160 khi có (11 tool của Python: 13.403); test ghim ngân sách 18.500 và 20.000.
- CI trên nhánh chạy bằng `workflow_dispatch` (CI chỉ tự chạy ở push `main` và PR): run `41468339999` xanh cả 8 job (lần 3). Hai lần đầu job `asset aarch64-apple-darwin` chờ 15 phút không được cấp runner `macos-15` (main đang chiếm) rồi bị huỷ, chưa chạy bước nào; chạy lại sau khi run của main xong thì qua.

## Còn mở

1. Đo nhóm A–D playbook hai cửa và một cửa (lượt gọi tool, token mỗi câu): cần dùng thật, chưa làm. Nếu từ 2 câu trở lên vượt H4 thì mở phase `run` (điều kiện ghi ở phase 17 và DECISIONS).
2. Sửa `CLAUDE.md` mẫu của vault và hướng dẫn cài ở repo đặc tả: cần user cho phép.
3. `scripts/install.sh` (dòng 29, 335) và `scripts/install.ps1` (dòng 236) còn nói mục `'kg'`; ngoài danh sách file được sửa nên để nguyên.
4. `plans/261102-0535-rust-core-port-master/handoff-windows-test.md` mô tả rc.1 với `kg`; để nguyên theo chỉ dẫn.
5. Trạng thái phase 17 và `plan.md` chưa đổi: đổi trạng thái phải qua `ak plan` (kho kế hoạch chung của máy), để controller làm.
6. Hai báo cáo thiết kế (brainstormer, kongming) chưa có trong git; changelog và DECISIONS nhắc tới chúng không kèm đường dẫn.
7. Cỡ `tools/list` vượt ước lượng "khoảng 17 KB" chừng 4% vì giữ nguyên câu mô tả đã chốt; muốn gọn hơn thì phải rút câu mô tả.

## Phần nền (đợt trước): giao thức stdio của OHS `vantt.4`

Ghim gói OHS `v0.17.2-vantt.4`, sha256 tự tải và đo, khớp `.sha256` và `digest` của asset: win32-x64 `5cb480f8…`, darwin-arm64 `ac2c3175…`, darwin-x64 `37eed430…`, linux-x64 `e6972863…`. `Ohs::read` gửi action `read` qua tiến trình đang chạy; MCP `read` không dùng nó (đọc thẳng vault) nhưng giữ lại cho plugin, CLI về sau.

Giao thức (nguồn `src/transports/stdio-server.ts`, `src/contracts/boundary-validation.ts` của fork tại tag; mẫu chạy thật trên kg-solo): mỗi dòng một JSON có `id`; lỗi `{id, error}`; dòng đầu `{"ready":true}` in trước khi index xong; khoá lạ bị bỏ qua im lặng; tên tham số stdio là camelCase (`snippetLength`, `linkType`, `notePath`), khác tool MCP của OHS (`snippet_length`, `link_type`, `path`).

- `search`: `{id, query, options: {mode=hybrid, limit=10 (0 = không giới hạn), threshold=0, snippetLength=300, scope, tag, frontmatter, paths, related=false, depth=1, direction=both, linkType=wiki, notePath, rerank=false, queries, anchors=false, explain=false}}` → `{id, results: [{path, title, tags, aliases, score, rank, depth?, snippet, matchedBy, links, backlinks, markdownLinks, markdownBacklinks, urls, scores{semantic,bm25,fuzzy_title,hybrid}, previewAnchors?, primaryAnchorIndex?, matchedTerms?, explain?}]}`; phản hồi thật còn lọt `semanticAnchor`, `bm25Anchor`, `parsedTags`. Đường dẫn trả ở dạng NFD.
- `read`: `{id, action: "read", paths (chuỗi, mảng, hoặc chuỗi JSON của mảng; không rỗng), snippetLength?, related? (mặc định true)}` → `{id, notes: [{path, found: true, title, aliases, tags, content, links, backlinks, markdownLinks, markdownBacklinks, urls} | {path, found: false, suggestions (≤ 3)}]}`; tra đường dẫn đúng nguyên văn, không tự thêm `.md`.
