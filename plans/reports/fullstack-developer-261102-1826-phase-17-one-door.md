# Phase 17 (một cửa): phần nền, chờ thiết kế chữ ký tool

Ngày 2026-11-02. Nhánh `phase-17-one-door` (từ `889f0cd`), đã đẩy lên origin. Không merge, không tag.

Phạm vi đã thu hẹp theo yêu cầu giữa chừng của controller: chỉ làm nhánh, nâng ghim OHS, đường gọi `search`/`read` nội bộ trong `pkm-core::ohs`, và ghi lại giao thức stdio của OHS để thiết kế chữ ký tool. **Chưa** thêm hay đổi tool MCP, tên server, `instructions`, hay việc đăng ký MCP của bộ cài.

## Đã làm

| Commit | Nội dung |
|---|---|
| `3dd81f0` build(install) | Ghim gói OHS `v0.17.2-vantt.4` (`crates/pkm/src/install/ohs_bundle.rs`), test và README dòng cài đặt theo; mục mới trong `docs/DECISIONS.md`. |
| `780a9f8` feat(ohs) | `Ohs::read(&[String], options) -> Result<Vec<Value>, OhsError>`; `search` và `read` dùng chung `request` (cổng `ready`, lỗi `ohs_not_ready`, timeout, lỗi OHS). Chuẩn hoá NFC thêm cho `links`, `backlinks`, `markdownLinks`, `markdownBacklinks`, `suggestions` (trước chỉ `path`). Server giả có action `read`; test thật đọc qua gói vantt.4. `spec/PORTING.md` mục `ohs` cập nhật. |

sha256 (khớp file `.sha256` đăng kèm và `digest` của asset; tự tải và đo lại):

| Nền tảng | sha256 |
|---|---|
| win32-x64 (`.zip`) | `5cb480f81ded99abfb2ab3ef0c0582320de85430ebff0d7dcd2f582d53e94f12` |
| darwin-arm64 | `ac2c3175776415a07fe3a933569c81e71fcc532890b49a56b2ed00d8fbcc9952` |
| darwin-x64 | `37eed430ac9697e8db0bab411a017fedd32a6c7e7d3bd14da2b67396f95a2a22` |
| linux-x64 | `e6972863f284a254d1d6c8b33e778cd53df7246df2ac5794e06bcb92838110b4` |

Ghi chú: release `vantt.3` cũng liệt kê chín bản vá (gồm `stdio-parity`), nên `read` không phải chỉ có từ vantt.4; dù vậy vantt.4 là bản user chỉ định và là bản đã chạy thử.

## Kiểm

- `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`: qua.
- Gói linux-x64 vantt.4 thật trên kg-solo (seed 11): `ohs_real` (search song song + read), `related_ohs_real`, `downloads_and_installs_pinned_release_asset` (`#[ignore]`): qua; không còn tiến trình `node`.
- Bộ so khớp: 64 mục, 0 lệch chưa có tên.
- CI trên nhánh: CI chỉ tự chạy ở push `main` và PR, nên chạy bằng `workflow_dispatch` (run `41437902908`).

## Giao thức stdio của OHS (`ohs serve --stdio`, fork `v0.17.2-vantt.4`)

Nguồn: `src/transports/stdio-server.ts`, `src/contracts/boundary-validation.ts`, `src/search/searcher.ts` của fork tại tag; mẫu phản hồi chạy thật trên kg-solo.

Chung: mỗi dòng một JSON, có `id` (chuỗi); phản hồi mang cùng `id`. Lỗi là `{id, error: "<chuỗi>"}`. Dòng đầu là `{"ready":true}` (in khi mở DB, **trước** khi index xong). Schema zod không `strict`: **khoá lạ bị bỏ qua im lặng** (ví dụ `snippet_length` kiểu snake_case không có tác dụng). Tham số stdio viết camelCase, khác tool MCP của OHS (snake_case: `snippet_length`, `link_type`, và `path` thay cho `notePath`).

### `search`

Request: `{"id","query": string (bắt buộc), "options": {…}?}` (có thể kèm `"action":"search"`). `options`:

| Khoá | Kiểu | Mặc định |
|---|---|---|
| `mode` | `hybrid`\|`semantic`\|`fulltext`\|`title` | `hybrid` |
| `limit` | int ≥ 0 (0 = không giới hạn) | 10 |
| `threshold` | 0..1 | 0 |
| `snippetLength` | int ≥ 0 | 300 |
| `scope` | chuỗi, mảng chuỗi, hoặc chuỗi JSON của mảng (tiền tố thư mục) | — |
| `tag` | như `scope` | — |
| `frontmatter` | như `scope` (dạng `key:value`) | — |
| `paths` | mảng chuỗi: giới hạn mọi nhánh tìm vào đúng các note này | — |
| `related` | bool: đi graph từ một note thay vì tìm | false |
| `depth` | int ≥ 0 (khi `related`) | 1 |
| `direction` | `outgoing`\|`backlinks`\|`both` (khi `related`) | `both` |
| `linkType` | `wiki`\|`markdown`\|`all` (khi `related`) | `wiki` |
| `notePath` | chuỗi: note gốc cho `related`/tương tự; có nó thì `query` không dùng làm câu tìm | — |
| `rerank` | bool (chỉ `hybrid`) | false |
| `queries` | mảng chuỗi: tìm song song rồi gộp RRF | — |
| `anchors` | bool: thêm `previewAnchors`, `primaryAnchorIndex` | false |
| `explain` | bool: thêm `explain: {ranks, debug}` | false |

`related: true` mà không có `notePath` thì `query` chỉ được coi là đường dẫn khi chứa `/` hoặc kết thúc `.md`.

Phản hồi: `{"id","results":[…]}`. Mỗi kết quả (mẫu thật, `limit: 2, snippetLength: 80`):

```json
{"path": "Yếu tố/Hợp đồng ABC còn hiệu lực.md", "title": "Hợp đồng ABC còn hiệu lực", "tags": [], "aliases": [],
 "snippet": "# Hợp đồng ABC còn hiệu lực", "score": 1,
 "scores": {"semantic": 0.8443319529334712, "bm25": 0.9510236123760036, "fuzzy_title": 0.9627863532005316, "hybrid": 1},
 "semanticAnchor": {"kind": "semantic", "headingPath": null, "matchText": "Hợp đồng ABC còn hiệu lực", "charStart": 0, "charEnd": 27},
 "parsedTags": [], "matchedTerms": ["Hợp", "đồng"], "matchedBy": ["semantic", "bm25", "title", "view:seg"],
 "links": [], "backlinks": [], "markdownLinks": [], "markdownBacklinks": [], "urls": [], "rank": 1}
```

Khoá theo kiểu `SearchResult`: `path, title, tags[], aliases[], score, rank?, depth? (chỉ khi related; âm là phía backlink), snippet, matchedBy[], links[], backlinks[], markdownLinks[], markdownBacklinks[], urls[], scores{semantic,bm25,fuzzy_title,hybrid} (số hoặc null), previewAnchors?[], primaryAnchorIndex?, matchedTerms?[], explain?{ranks, debug?}`. Phản hồi thật còn lọt khoá nội bộ không có trong kiểu: `semanticAnchor`, `bm25Anchor`, `parsedTags`. Anchor: `{kind: bm25|semantic, headingPath, matchText, charStart, charEnd, blockId?, hasHighlight?}`. Ở chế độ `related`, kết quả gốc có `depth: 0`, `matchedBy: ["source"]`, `scores` toàn `null`. Đường dẫn trả ở dạng NFD (pkm chuẩn hoá NFC).

### `read`

Request: `{"id","action":"read","paths": …,"snippetLength"?: int ≥ 0,"related"?: bool}`.

- `paths`: chuỗi, mảng chuỗi, hoặc chuỗi JSON của mảng; rỗng thì `{id, error: "Invalid stdio request: paths must not be empty"}`.
- `snippetLength`: cắt `content` (body gốc, sau frontmatter) còn chừng ấy ký tự (đơn vị UTF-16 của JS); không có thì cả note. Âm thì `error: "Invalid stdio request: snippetLength Too small: expected number to be >=0"`.
- `related`: mặc định **true** (kèm `links`, `backlinks`, `markdownLinks`, `markdownBacklinks`); `false` thì bốn mảng đó rỗng. `urls` luôn có.
- Tra đường dẫn đúng nguyên văn (sau NFD); **không** tự thêm `.md` hay giải tên ngắn như `search`.

Phản hồi: `{"id","notes":[…]}`, đúng thứ tự `paths`, mỗi phần tử một trong hai dạng (mẫu thật):

```json
{"path": "Yếu tố/Hợp đồng ABC còn hiệu lực.md", "found": true, "title": "Hợp đồng ABC còn hiệu lực", "aliases": [], "tags": [],
 "content": "# Hợp đồng ABC còn hiệu lực\n", "links": [], "backlinks": [], "markdownLinks": [], "markdownBacklinks": [], "urls": []}
{"path": "Không có.md", "found": false, "suggestions": ["Yếu tố/Mức hài lòng của Đông Á.md", "Người/Vũ Thị Khôi.md", "Người/Vũ Văn Khôi.md"]}
```

`suggestions`: tối đa 3 note gần tên (fuzzy theo tên file). Thiếu `.md` thì `found: false`, gợi ý đầu là đúng note đó.

### Action khác (không dùng ở đây)

`status` (`{id, status:{…}}`), `vectors`, `neighbors`, `reindex` (ghi). Sự kiện `{"event":"indexed"|"removed",…}` không `id` chỉ có khi đặt `OHS_STDIO_EVENTS=1` (pkm không đặt).

## API Rust mới

- `Ohs::read(paths: &[String], options: Value)`: `options` là object hoặc `null`, các khoá đặt ở mức trên cùng của request (như giao thức); cấm `id`, `action`, `paths` trong `options`; `paths` rỗng hoặc có phần tử rỗng là `OhsError::Invalid` trước khi gửi. Chỉ chạy khi `ready` (đang index thì note chưa vào index sẽ bị báo `found: false` sai).
- `Ohs::search` không đổi chữ ký.

## Còn mở

1. Chờ thiết kế chữ ký tool MCP (`search`, `read`, `related` nhận câu hỏi, `instructions`, có thể "code mode") từ controller; sau đó mới làm tool, đổi tên `kg` → `pkm`, bộ cài thôi đăng ký `ohs`, `docs/MCP-CHANGELOG.md`, ghi lệch có chủ ý vào bộ so khớp.
2. Sửa `CLAUDE.md` mẫu của vault và hướng dẫn cài ở repo đặc tả: cần user cho phép.
3. Đo nhóm A–D playbook hai cửa và một cửa: cần dùng thật, chưa làm.
4. Trạng thái phase file và `plan.md` chưa đổi vì phase mới làm phần nền.
5. `plans/261102-0535-rust-core-port-master/handoff-windows-test.md` còn ghi `vantt.3` và sha cũ; ngoài danh sách file được sửa nên để nguyên.
6. Báo cáo này nằm trong nhánh (worktree cô lập không ghi được vào checkout chính).
