# Brainstorm: bề mặt tool của MCP một cửa `pkm` và có nên làm "code mode"

- Ngày: 2026-11-02 (Asia/Saigon). Loại: phân tích chỉ đọc, không sửa mã.
- Việc: phase 17 (R14, Q1, Q2) của `plans/261102-0535-rust-core-port-master/`.
- Nguồn đã đọc: `phase-17-one-door-mcp.md`; `docs/PRD.md` mục 3.1, 8, 9, 14, 15; `docs/DECISIONS.md`; `crates/pkm-core/src/tools/defs.rs`, `tools.rs` (hàm `related`), `related.rs`, `ohs.rs`, `ohs/state.rs`; `crates/pkm/src/mcp.rs`, `mcp/instructions.rs`, `mcp/tool_layer.rs`; golden `parity/golden/223c9c3…/mcp/` (`tools_list.json`, `instructions.txt`); fork OHS tag `v0.17.2-vantt.4` (`src/transports/mcp-runtime.ts`, `stdio-server.ts`, `contracts/boundary-validation.ts`, `search/searcher.ts`); playbook `07-kg-challenges.md` của repo đặc tả (bản clone `~/.cache/pkm-follow/spec-repo`, `origin/main`); repo `vantt/browser-worker` (`hub/src/mcp_code_mode*.rs`, `agent_js_client_emitter.rs`, `hub/client/bw-client-runtime.mjs`, `skill/browser-worker/references/scripting.md`, `plans/reports/hub-mcp-code-mode-report.md`).

## 0. Tóm tắt

Khuyến nghị phương án B ("một cửa gọn"). Phương án này giữ tool rời như hiện nay và thêm `search`, `read`. `related` nhận `query`, `queries`, `path` làm đường chính. `search` chỉ phơi một tập tham số OHS đã chọn, giữ đúng tên mà MCP của OHS dùng, và trả kết quả đã bỏ các mảng link. Theo ước lượng, phần định nghĩa tool còn khoảng 4,4 nghìn token so với khoảng 6,0 nghìn của hai cửa. Câu nhóm D cần 2 lượt gọi thay vì 3, và đỡ khoảng 2,5 nghìn token mỗi câu.

Không làm code mode ở phase 17. Lợi ích thật của code mode chỉ ở vài câu "đuôi dài" (B4, C4, D5, nhóm E). Cái giá thì lớn: phải nhúng trình thông dịch vào binary Rust, mô tả API tốn token, model trên Claude Desktop viết script dễ lỗi, và bộ so khớp không phủ được. Cơ chế "code mode" của `browser-worker` thật ra không chạy mã ở server. Nó gồm ba cửa: CLI, một client JS sinh tự động, và một profile MCP có 3 meta-tool. Phần mã chạy trên shell của chính agent, nên Claude Desktop không dùng được. Ghi lại một điều kiện để xem lại: sau khi đo playbook, nếu câu loại "phép tập hợp" vượt giới hạn H4 thì làm một tool `run` (QuickJS nhúng) dạng hybrid, công khoảng 4–6 ngày.

Hai giả định của phase 17 nên xem lại:
1. "`read` chuyển tiếp sang OHS". OHS `read` chỉ đọc được note đã nằm trong index của OHS. Vì vậy trong lúc OHS `starting`, `indexing` hay `model_missing`, việc đọc một file markdown sẽ báo `ohs_not_ready`, điều này vô lý. `pkm` đã tự đọc thân note (tool `note`), nên `read` nên làm tại chỗ với tham số và dạng kết quả giống OHS (câu hỏi mở Q-A).
2. "Giữ tên tham số của OHS". OHS có hai bộ tên. MCP của OHS dùng `snippet_length`, `link_type`, `path`; stdio dùng `snippetLength`, `linkType`, `notePath`, và gộp `query` với `queries` theo cách riêng. Agent đang quen bộ tên MCP, nên `pkm` phải phơi bộ tên MCP và tự đổi sang stdio.

## 1. Hiện trạng

### 1.1. Tool của MCP `kg` (Rust, khớp Python ở `223c9c3`)

Cỡ là số ký tự JSON gọn của mục trong `tools/list` (golden kg-solo, không có trang hệ thống nên không có tham số `system`).

| Tool | Tham số (bắt buộc in đậm) | Kết quả | Cỡ |
|---|---|---|---|
| `find` | **name**, type, limit | danh sách `{id, title, type, aliases}`, rút gọn khi dài | 811 |
| `note` | **node**, body_chars (800) | thẻ note: type, props, aliases, cửa sổ hiệu lực, nguồn gốc, số cạnh theo quan hệ và lớp, block, đầu thân note, `url` | 600 |
| `neighbors` | **node**, dir, rels, as_of, statuses, accepted_only, (system), limit | các cạnh một bước: rel, node, type, dir, status, layer, claim, valid_from/to, source, proposed_by, extracted_by, confidence | 1.923 |
| `reach` | **node**, dir, max_depth, bộ lọc, limit | note tới được kèm độ sâu; dài thì `count`, `by_type`, `items`, `handle` | 1.963 |
| `paths` | **from**, **to**, max_len (6), k (10), bộ lọc | đường: nodes, rels, statuses, sign, layer | 1.744 |
| `loops` | **node**, **max_len**, bộ lọc, limit | vòng: nodes, rels, negatives, kind, layer | 1.792 |
| `provenance` | **claim** | nguồn, ủng hộ, phản bác, bản ghi, cạnh khẳng định, chuỗi thay thế | 503 |
| `select` | type, status, proposed_by, where, limit, (system) | note khớp frontmatter | 1.155 |
| `valid` | **type**, **as_of**, limit, (system) | note trong cửa sổ hiệu lực | 570 |
| `related` | hits, query ("chỉ dùng khi hits rỗng"), k (15), types, bộ lọc | hit xếp lại (`found_by: "ohs"`) và note chỉ graph thấy (`found_by: "graph"`) kèm `via`; `seed_source` là `hits` hoặc `ohs-cli` | 1.914 |
| `more` | **handle**, offset, limit | trang tiếp của kết quả dài | 406 |
| **Cộng** | | | **13.381 ký tự, khoảng 3,3 nghìn token** |

Bộ lọc cạnh (`rels`, `as_of`, `statuses`, `accepted_only`) lặp lại ở 5 tool, khoảng 700 ký tự mỗi lần, tức khoảng 700 token cho cả danh sách. `instructions` trên kg-solo dài 4.310 ký tự, khoảng 1,2 nghìn token; vault thật có nhiều loại note và quan hệ hơn nên sẽ dài hơn. Kết quả tool đi qua `compact()` (bỏ `null` và chuỗi rỗng) rồi cắt ở `--max-chars`.

### 1.2. Tool của MCP `ohs` (fork `v0.17.2-vantt.4`)

| Tool | Tham số MCP | Cỡ ước lượng |
|---|---|---|
| `search` | query, queries, path, mode, scope, limit, threshold, paths, tag, frontmatter, related, depth, direction, link_type, snippet_length, rerank, anchors, explain | khoảng 8.400 ký tự |
| `read` | **paths** (chuỗi hoặc mảng), snippet_length, related | khoảng 1.400 |
| `status` | include_activity | khoảng 200 |
| `reindex` | path, force | khoảng 800 |
| **Cộng** | | **khoảng 10.900 ký tự, khoảng 2,7 nghìn token**, không có `instructions` |

Một kết quả `search` có: `path`, `title`, `tags[]`, `aliases[]`, `score`, `rank`, `snippet`, `matchedBy[]`, `links[]`, `backlinks[]`, `markdownLinks[]`, `markdownBacklinks[]`, `urls[]`, `scores{semantic,bm25,fuzzy_title,hybrid}`. MCP của OHS in JSON thụt lề 2 ký tự. Mười kết quả với snippet 300 ký tự vào khoảng 7–10 nghìn ký tự, tức khoảng 2–2,5 nghìn token. Note trung tâm có nhiều link sẽ làm con số này phình thêm.

Một kết quả `read` có: `{path, found:true, title, aliases, tags, content, links, backlinks, markdownLinks, markdownBacklinks, urls}`. Khi không thấy note thì trả `{path, found:false, suggestions[3]}`.

**Hai cửa cộng lại:** khoảng 6,0 nghìn token định nghĩa tool cộng 1,2 nghìn token `instructions` cho mỗi request. Prompt caching làm phần này rẻ về tiền nhưng không bớt chỗ trong ngữ cảnh. Cách ước lượng: khoảng 4 ký tự mỗi token cho chữ tiếng Anh; chữ tiếng Việt có dấu đắt hơn, khoảng 3–3,6 ký tự mỗi token. Muốn có số chính xác thì đo bằng endpoint đếm token của API hoặc bằng log phiên của Agent Client.

### 1.3. Giao thức stdio của OHS mà `pkm` sẽ gọi

- Lệnh tìm: `{"id","query":<chuỗi, bắt buộc>,"options":{mode, scope, limit, threshold, tag, frontmatter, paths, related, depth, direction, linkType, snippetLength, notePath, rerank, queries, anchors, explain}}`, trả `{"id","results":[…]}`.
- Lệnh đọc: `{"id","action":"read","paths":…,"snippetLength","related"}`, trả `{"id","notes":[…]}`.
- MCP của OHS tự đổi tham số trước khi tìm, nên `pkm` phải làm y như vậy: `query` gửi đi là `path ?? query ?? queries[0]`; `options.notePath = path`; `options.queries` chỉ gửi khi có từ hai câu trở lên (câu `query` đứng đầu); đổi `snippet_length` thành `snippetLength`, `link_type` thành `linkType`.
- Khi `path` trỏ mơ hồ, MCP của OHS trả `{type:"ambiguous", candidates}`. Qua stdio thì chỉ còn `{error: message}`, mất danh sách ứng viên.

### 1.4. Phát hiện bên lề (nên biết trước khi làm)

1. **Hợp đồng năm khoá của `ohs_not_ready` chỉ đúng khi `state` là `indexing`.** `mcp.rs` đưa mọi kết quả qua `compact()`, hàm này bỏ khoá `null`. Ở các trạng thái `starting`, `model_missing`, `broken` thì `progress` và `eta_s` là `null`, nên agent chỉ thấy ba khoá. Test ở `ohs/state.rs` kiểm đối tượng trước khi qua `compact`, nên không bắt được chỗ này. Cần chọn: miễn `compact` cho lỗi `ohs_not_ready`, hoặc ghi rõ trong hợp đồng là "năm khoá, khoá `null` bị bỏ ở tầng MCP".
2. Lời lỗi `NO_OHS_INDEX` và mô tả `related` còn nói "OHS search tool"; `seed_source` có giá trị `"ohs-cli"`. Ở một cửa thì các câu chữ này sai.
3. `SERVER_NAME = "kg"`; dòng đầu của `instructions` là "Typed knowledge graph (kg)"; đoạn cuối hướng dẫn phối hợp với "OHS server". Cả ba cần đổi theo Q2.
4. Nếu người dùng giữ MCP `ohs` bên cạnh, agent sẽ thấy hai tool cùng tên `search`. Hai tiến trình OHS cũng sẽ nạp model hai lần, và có thể cùng ghi một DB index nếu dùng chung đường dẫn DB. `instructions` phải nói rõ là ưu tiên `search` của `pkm`, và `pkm doctor` nên cảnh báo trường hợp này.

## 2. Các phương án

Cả bốn phương án dùng chung các điểm sau:
- Tên server là `pkm`.
- 11 tool cũ giữ nguyên tên và tham số.
- Lỗi nghiệp vụ là `{"error": "…"}`, có thể kèm một khoá nữa (`did_you_mean` hoặc `detail`), và có `isError`.
- Tool cần OHS trả đúng `{"error":"ohs_not_ready","state","progress","eta_s","action"}`, còn tool graph vẫn chạy bình thường.

### Phương án A: thêm tối thiểu (làm đúng câu chữ của phase 17)

**Tool:** 11 tool cũ, thêm `search`, `read`; tổng 13 tool.
- `search`: chuyển tiếp mỏng toàn bộ 18 tham số MCP của OHS, chép nguyên mô tả. Kết quả là `{"results":[…]}` đúng như OHS, in JSON gọn.
- `read`: chuyển tiếp sang OHS `read` (`paths`, `snippet_length`, `related`). Kết quả là `{"results":[…]}` như OHS.
- `related`: chỉ đổi câu mô tả của `query` thành "đường chính". Kết quả giữ nguyên.

**Lỗi:** `search` và `read` trả `ohs_not_ready`. Lỗi OHS khác thành `{"error":"ohs_error","detail":"…"}`.

**Phác `instructions`:** thay đoạn "Search vs graph…" bằng đoạn ở phương án B.

**Token định nghĩa:** khoảng 3,35 + 2,1 + 0,35 = **5,8 nghìn**, chỉ bớt khoảng 0,2 nghìn so với hai cửa nhờ bỏ `status` và `reindex`.

**Ưu:** công ít nhất (khoảng 1,5 ngày như plan); bám OHS sát nên khi OHS thêm tùy chọn chỉ cần chép.
**Nhược:**
- Mô tả `search` vẫn nặng.
- Agent thấy hai cách đi graph: OHS `related`/`depth`/`direction`, chỉ có wikilink, và `neighbors`/`reach` có loại. Playbook nhóm B, C dễ chọn nhầm.
- `read` hỏng khi OHS chưa sẵn sàng.
- D3 (note cô lập) vẫn cần `search(path)` rồi `related(hits)`, tức 2 lượt.

### Phương án B: một cửa gọn (khuyến nghị)

**Tool:** 13 tool, cùng tên với A, nhưng `search`, `read`, `related` được thiết kế lại.

`search`: tham số dùng tên MCP của OHS, chỉ giữ phần đáng giá. Không tham số nào bắt buộc trong schema; nếu thiếu cả `query`, `queries` lẫn `path` thì trả lỗi.
```json
{"name":"search","description":"Find notes by words or meaning, in any language, also when the user only half-remembers the content. Hybrid BM25 + vector + fuzzy title by default. Results: rank, path (= note id for every other tool), title, type, tags, aliases, snippet, score 0-1 (>=0.5 relevant, <0.2 weak), matchedBy. For what is related to an idea beyond text matches, use `related` with the same query instead. For typed links around a hit use neighbors/reach.",
 "inputSchema":{"type":"object","properties":{
  "query":{"type":"string","description":"Text to find."},
  "queries":{"type":"array","items":{"type":"string"},"description":"2-4 rewordings when recall matters (other language, synonyms); merged by rank fusion."},
  "path":{"type":"string","description":"Note id: notes similar in meaning to it, excluding the note and notes it already links to. Ignores query."},
  "mode":{"type":"string","enum":["hybrid","semantic","fulltext","title"],"description":"Default hybrid. fulltext = exact words, title = a named note, semantic = wording unknown."},
  "scope":{"oneOf":[{"type":"string"},{"type":"array","items":{"type":"string"}}],"description":"Folder prefix(es); \"-\" prefix excludes."},
  "tag":{"oneOf":[{"type":"string"},{"type":"array","items":{"type":"string"}}],"description":"Tags, ANDed; \"-\" prefix excludes."},
  "frontmatter":{"oneOf":[{"type":"string"},{"type":"array","items":{"type":"string"}}],"description":"\"field:value\" filters, ANDed; without query lists matches by title."},
  "paths":{"type":"array","items":{"type":"string"},"description":"Search only inside these note ids."},
  "limit":{"type":"integer","description":"Default 10; keep <= 10 unless the user wants a broad list."},
  "threshold":{"type":"number","description":"Minimum score 0-1; ~0.2 hides weak matches."},
  "snippet_length":{"type":"integer","description":"Snippet characters, default 300."}}}}
```
- **Bỏ khỏi bề mặt:** `related`, `depth`, `direction`, `link_type` (đi graph là việc của `neighbors`/`reach`); `rerank` (lần đầu tải model khoảng 570 MB và chậm, không hợp với Claude Desktop); `anchors` (chỉ phục vụ UI); `explain` (để gỡ lỗi). Tên các tham số còn lại giữ đúng như OHS, nên câu chữ trong `CLAUDE.md` và playbook 02 vẫn đúng.
- **Kết quả:** `{"results":[{"rank","path","title","type","tags","aliases","snippet","score","matchedBy"}]}`.
  - `path` đã chuẩn hoá về NFC.
  - `type` lấy từ index của `pkm` bằng một câu SQL. Agent biết ngay loại note mà không phải gọi `note`, và tránh được bẫy F3.
  - Bỏ `links`, `backlinks`, `markdownLinks`, `markdownBacklinks`, `urls`, `scores`. Phần quan hệ đã có `neighbors` trả có loại và có lớp. Phần bỏ đi thường chiếm phần lớn kết quả ở note trung tâm; cần đo trên bản B trước khi chốt (Q-B).
- **Lỗi:**
  - OHS chưa sẵn sàng thì trả `ohs_not_ready`.
  - `path` mơ hồ thì trả `{"error":"ambiguous path","did_you_mean":[…]}`. Muốn có danh sách này thì gọi `find` của `pkm`, vì stdio không trả ứng viên.
  - Lỗi OHS khác thì trả `{"error":"search failed","detail":"…"}`.

`read`: làm tại chỗ trong `pkm`, không cần OHS. Tham số và dạng kết quả giống OHS.
```json
{"name":"read","description":"Full text of one or more notes by id (vault path), with title, aliases, tags, links and backlinks. Use after search/related/find to read what you will quote; pass several ids at once. Missing id: found false with suggestions.",
 "inputSchema":{"type":"object","properties":{
  "paths":{"oneOf":[{"type":"string"},{"type":"array","items":{"type":"string"}}],"description":"Note id(s)."},
  "snippet_length":{"type":"integer","description":"Max characters of content per note (default full). Use ~2000 when reading many."},
  "related":{"type":"boolean","description":"Include links/backlinks (default true)."}},"required":["paths"]}}
```
- **Kết quả:** `{"results":[{"path","found":true,"title","aliases","tags","content","links","backlinks","url"}]}`, hoặc `{"path","found":false,"suggestions":[…]}`.
  - `content` là thân note sau frontmatter. Module `vault` đã tách sẵn phần này.
  - `links`/`backlinks` lấy từ bảng `links` của index.
  - `url` là link `obsidian://` như tool `note`.
  - `suggestions` lấy từ `find` theo tên file.
- **Lỗi:** chỉ là lỗi nghiệp vụ (đường dẫn ra ngoài vault, đọc file lỗi), không bao giờ là `ohs_not_ready`.
- Nếu user giữ quyết định "chuyển tiếp" thì dạng kết quả vẫn như trên, chỉ khác nguồn. Lúc đó `read` sẽ lỗi khi OHS chưa sẵn sàng.

`related`: câu hỏi là đường chính. Các khoá cũ giữ nguyên, thêm `queries` và `path`.
```json
{"name":"related","description":"What in the vault relates to an idea, question or note: searches it (hybrid) and walks the typed graph from the hits with personalised PageRank, in one call. Returns the hits re-ranked by graph support (found_by \"ohs\") and notes only the graph reached (found_by \"graph\": what text search missed), each with `via` = the shortest edge chain from a hit. Read the notes before stating how they relate. Pass `hits` only if you already ran search and want exactly those seeds.",
 "inputSchema":{"type":"object","properties":{
  "query":{"type":"string","description":"The idea or question, in the user's words. Main input."},
  "queries":{"type":"array","items":{"type":"string"},"description":"2-4 rewordings (e.g. English and Vietnamese) for better seeds."},
  "path":{"type":"string","description":"Note id: seeds are notes similar to it (for a note with no links)."},
  "hits":{"type":"array","items":{},"description":"Optional seeds you already have: search results ({path, score}) or note ids."},
  "k":{"type":"integer","description":"Graph-only notes returned (default 15)."},
  "types":{"type":"array","items":{"type":"string"},"description":"Only return notes of these types."},
  "rels":"…","as_of":"…","statuses":"…","accepted_only":"…"},"required":[]}}
```
- **Thứ tự ưu tiên của nguồn hạt giống:** `hits` (nếu có) trước, rồi `path`, rồi `query`/`queries`.
- **Kết quả:** như cũ. `seed_source` có giá trị `"hits"`, `"query"` hoặc `"path"` (đổi từ `"ohs-cli"`), ghi vào changelog.
- **Lỗi:** hạt giống từ OHS thì có thể trả `ohs_not_ready`; không có nguồn hạt giống nào thì `{"error":"related needs query, path or hits"}`.

**Phác `instructions`** (thay đoạn cuối, đổi "(kg)" thành "(pkm)"):
> One door: this server (pkm) answers every question about the vault. Words, meaning, full text, half-remembered content, other languages: `search`. Named things: `find`. Typed relations, multi-hop, paths, loops, time windows, provenance: the graph tools. "What relates to <idea>": `related` with `query` (search and graph walk in one call; do not run search first just to feed hits). Full text: `read` (several ids at once). If another server also offers search, use this one. A tool that answers {"error":"ohs_not_ready"} tells you the state and what to do: tell the user; graph tools still work.

**Token định nghĩa:** 3,35 (cũ) + khoảng 0,7 (`search`) + khoảng 0,3 (`read`) + khoảng 0,05 (thay đổi ở `related`) = **khoảng 4,4 nghìn**, tức bớt khoảng 1,6 nghìn token mỗi request so với hai cửa. `instructions` gần như không đổi độ dài.

**Ưu:**
- Đáp ứng nghiệm thu của phase 17 với dư địa rõ.
- Nhóm D: 2 lượt thay 3.
- `read` vẫn chạy khi OHS đang index.
- Agent chỉ thấy một bề mặt đi graph.
- Không thêm phụ thuộc nào.

**Nhược:**
- Lệch khỏi "chuyển tiếp mỏng" ở hai chỗ (bộ tham số đã chọn, kết quả bỏ mảng link) và ở `read` làm tại chỗ. Ba chỗ này cần user đồng ý.
- Khi OHS thêm tùy chọn mới thì phải quyết có phơi ra không. Đây là việc chủ ý chứ không phải chép.

**Công:** khoảng 2 ngày (1,5 như plan, thêm khoảng 0,5 cho `read` tại chỗ và cột `type` của `search`).

### Phương án C: code mode thật, nhúng trình thông dịch (hybrid)

**Tool:** 13 tool của B, thêm `run`.
```json
{"name":"run","description":"Run a short JavaScript program against the vault when a question needs many calls, set operations or filtering (e.g. notes in folder X with no link except topic; bridges between two areas). Inside, `pkm.<tool>(args)` calls any tool above with the same args and returns its result object (errors come back as {error}). `return` the value to show; keep it small. Limits: 10 s, 64 MB, 50 tool calls. No files, no network.",
 "inputSchema":{"type":"object","properties":{"code":{"type":"string","description":"Function body, e.g. const a = pkm.reach({node:'X.md', max_depth:2}); return a.count;"}},"required":["code"]}}
```
- **Kết quả:** `{"result": <giá trị return>, "calls": n}`, đi qua `compact` và `--max-chars` như mọi tool.
- **Lỗi:**
  - Lỗi script trả `{"error":"script error","detail":"line 3: …"}`.
  - Gặp `ohs_not_ready` trong script thì dừng script và trả nguyên đối tượng năm khoá.
  - Quá giới hạn thì trả `{"error":"script limit","detail":"time|memory|calls"}`.
- **Cách chạy:**
  - Dùng QuickJS qua `rquickjs`. Thư viện này viết bằng C, build bằng `cc` như SQLite bundled, và không cần tokio.
  - Bộ ngắt (interrupt handler) cắt theo thời gian; giới hạn bộ nhớ của runtime chặn tràn bộ nhớ.
  - Không nạp module `std`/`os`, nên script không có IO.
  - `pkm.*` gọi thẳng `tools::call` trên cùng phiên; kết quả chuyển qua lại bằng `serde_json`.
  - Phương án thay thế là Rhai: viết bằng Rust thuần, có sẵn `set_max_operations` và `on_progress`. Đổi lại model viết Rhai kém hơn JS, nên dễ lỗi cú pháp hơn.
- **`instructions`:** thêm một câu "For questions that need many calls or set logic, prefer `run`".
- **Token định nghĩa:** khoảng 4,4 + 0,4 = **khoảng 4,8 nghìn**. Nếu bỏ tool rời, chỉ giữ `run` kèm tài liệu API, thì khoảng 2,5 nghìn token. Nhưng khi đó mất mô tả từng tool, mà mô tả này chính là "núm" mà playbook 07 mục 8 dùng để vặn hành vi agent.
- **Công:** khoảng 4–6 ngày: nhúng thư viện, build và test trên 4 đích (Windows MSVC CRT tĩnh), binding, giới hạn, ánh xạ lỗi, test, tài liệu, ngoại lệ có tên trong bộ so khớp, đo lại playbook. Binary nặng thêm, ước lượng khoảng 1 MB, cần đo.

### Phương án D: port cơ chế "code mode" của `browser-worker`

**Cơ chế thật của `browser-worker`** (đọc từ mã, xem mục 3.1): server không chạy mã nào. Có ba cửa cùng một bộ hành động:
1. **CLI** `browser-worker-hub agent <tool> key=value --json`, trả một dòng `{"ok":true,"data"}` hoặc `{"ok":false,"error":{status,code,hint,message}}` và thoát mã 1. Đi kèm hai lệnh tra cứu `agent --list --json` và `agent describe <tool>`.
2. **Client JS sinh tự động** `bw-client.mjs`: ESM không phụ thuộc, Node 18 trở lên, mỗi tool là một hàm async có JSDoc sinh từ schema, gọi REST của hub, ném `BwError{status,code,hint}`. Dòng đầu file có hash, và `doctor` so hash này với bản binary đang chạy.
3. **Profile MCP `code`** với 3 meta-tool `tools_search`, `tool_describe`, `tool_call`. Test giữ ngân sách `tools/list` ≤ 1,5 KB và `instructions` ≤ 1,2 KB.

Mã do agent viết chạy trên shell hoặc Node của máy agent, qua tool Bash của Claude Code. Hub không có sandbox; ranh giới an toàn là router theo phạm vi (`tool_call` không với tới tool ngoài scope) và các cổng cần người duyệt. Hub dùng `rmcp` cộng `tokio` cộng `axum`.

**Port sang `pkm`:**

| Phần | Làm thế nào ở `pkm` | Công | Giá trị cho người dùng `pkm` |
|---|---|---|---|
| CLI chung `pkm VAULT call <tool> --args JSON` với cùng envelope, cộng `pkm tools --list/--describe` | Gọi `tools::call` có sẵn; CLI hiện đã có lệnh riêng cho 9 tool graph và `related` | 0,5–1 ngày | Chỉ agent có shell dùng được: chat Obsidian qua Agent Client (Claude Code), Claude Code. Claude Desktop thì không. |
| `search`, `read` qua CLI | Mỗi lần gọi CLI phải chạy một tiến trình OHS và nạp model (vài giây tới hàng chục giây). Muốn nhanh thì cần daemon và IPC (named pipe trên Windows) | 2–3 ngày | Không đáng ở giai đoạn này |
| Profile 3 meta-tool | Thêm cờ `--profile code` cho `pkm mcp` | 0,5–1 ngày | Âm: 13 tool chỉ khoảng 4,4 nghìn token, mà mỗi tool mới cần thêm 2 lượt (`tools_search`, `tool_describe`), trái nghiệm thu "lượt gọi không tăng". Ngoài ra Claude Code đã có Tool Search phía client (Claude Desktop thì chưa xác minh) nên tự hoãn nạp định nghĩa tool. |
| Client JS | Cần HTTP API, tức `pkm serve`, việc của giai đoạn sau (PRD D8). Bọc CLI bằng `child_process` thì không hơn gì gọi CLI trực tiếp | Chưa làm được | Không |

**Kết luận về D:** chỉ có phần CLI là port được rẻ (khoảng 1 ngày), và phần này chỉ giúp được agent có shell, và chỉ cho tool graph. Phần lớn đã có sẵn: CLI hiện in JSON. Muốn mở cửa này cho chat Obsidian thì chỉ cần viết một đoạn hướng dẫn trong `CLAUDE.md` mẫu, ví dụ vòng lặp `pkm VAULT reach …` bằng bash, cộng có thể thêm envelope `--json`. Đây không phải công của phase 17.

### 2.1. Playbook A–D: lượt gọi theo phương án

Ghi chú cách đọc bảng:
- "2∥" là 2 lượt gọi song song trong cùng một turn.
- Câu có tên note thì đều bắt đầu bằng `find`, vì A3 bắt agent hỏi lại khi tên mơ hồ. Do đó không gộp "tìm tên rồi đi graph" vào một lời gọi; code mode cũng phải giữ bước này.

| Câu | Hai cửa | A | B | C (hybrid) |
|---|---|---|---|---|
| A1 bỏ dấu | 1 `find` | 1 | 1 | 1 |
| A2 nhớ nội dung | 2 (`ohs.search` rồi `note`) | 2 | 1–2 (`search` đã có `type`, snippet; `read` khi cần) | 1–2 |
| A3 trùng tên | 1 | 1 | 1 | 1 |
| A4 song ngữ, A5 gõ sai | 1 `search` | 1 | 1 | 1 |
| A6 "công cụ" | 1 `select` | 1 | 1 | 1 |
| B1–B3, B5, B6 | 2 (`find` rồi `neighbors`/`select`) | 2 | 2 | 2 |
| B4 note cô lập trong thư mục | 2 + N (kiểm từng note) | như hai cửa | như hai cửa | **1–2** |
| C1, C2 đường giữa hai vùng | 3 (`find` 2∥, `paths`) | 3 | 3 | 2–3 |
| C3, C5, C6 | 2 | 2 | 2 | 2 |
| C4 note cầu nối | 4–8 | 4–8 | 4–8 | **1–2** |
| D1 câu trích → quản lý đội | 3 (`ohs.search`, `related(hits)`, `ohs.read`) | 2 (`related(query)`, `read`) | 2 | 1–2 |
| D2 hỏi Việt, đáp Anh | 1 | 1 | 1 (`queries` hai thứ tiếng) | 1 |
| D3 note cô lập nối được gì | 3 (`search(path)`, `related(hits)`, `read`) | 3 | **2** (`related(path)`, `read`) | 1–2 |
| D4 như D1 nhưng `accepted_only` | +1 (truyền lại hits) | +1 | +1 | +0–1 |
| D5 note cũ chưa dùng về Y | 3–6 | 3–6 | 3–5 | **1–2** |

**Token cho D1 (ước lượng, chưa gồm định nghĩa tool):**
- Hai cửa:
  - Kết quả `ohs.search`: khoảng 2–2,5 nghìn token.
  - Agent viết lại `hits`: khoảng 150–250 token đầu ra.
  - Kết quả `related`: khoảng 1,3 nghìn.
  - `read` 3 note với `snippet_length` 2000: khoảng 1,5–2 nghìn.
  - Cộng khoảng 5,3 nghìn, qua 3 turn, mỗi turn mang 6 nghìn token định nghĩa.
- A và B: khoảng 2,8–3,3 nghìn qua 2 turn. Định nghĩa mỗi turn là 5,8 nghìn (A) hoặc 4,4 nghìn (B).
- C: khoảng 2,5–3 nghìn cộng 150–300 token viết script, qua 1 turn. Chỉ lợi rõ khi script tự lọc bớt; nếu script lỗi thì phải thêm một turn.

### 2.2. So sánh tổng

| Tiêu chí | A | B | C | D |
|---|---|---|---|---|
| Token định nghĩa mỗi request | khoảng 5,8 nghìn | **khoảng 4,4 nghìn** | khoảng 4,8 nghìn | như B, cộng 0 (CLI) hoặc âm lượt (meta-tool) |
| Nhóm D (giá trị chính, playbook mục 1) | 2–3 lượt | **2 lượt** | 1–2 lượt | không đổi |
| Câu đuôi dài (B4, C4, D5, nhóm E) | không đổi | không đổi | **tốt hơn rõ** | chỉ khi có shell |
| Chạy được trên Claude Desktop | có | có | có, nhưng model phải viết JS đúng | CLI thì không |
| Phụ thuộc mới, cỡ binary | không | không | QuickJS (C), thêm khoảng 1 MB | không |
| Bộ so khớp | 2 tool mới, 1 tool đổi | như A | thêm `run`, không có golden Python | thêm lệnh CLI |
| Bảo trì khi OHS đổi | chép tham số | chọn tham số | như B, cộng API script | như B |
| Công | khoảng 1,5 ngày | khoảng 2 ngày | 4–6 ngày (sau B) | 1 ngày (CLI) đến 4 ngày (đủ ba cửa, trừ client JS) |

## 3. Phán quyết về code mode

### 3.1. Code mode ở nơi khác thực chất là gì
- **Cloudflare Code Mode và bài "code execution with MCP" của Anthropic:** model viết TypeScript gọi API sinh từ MCP, và mã chạy trong sandbox (V8 isolate, hoặc container chạy mã). Mức tiết kiệm lớn mà hai nơi này báo (tới khoảng 98%) đến từ các luồng truyền dữ liệu lớn giữa các tool, ví dụ tài liệu dài, bảng tính, hàng nghìn dòng. Dữ liệu đó không cần đi qua ngữ cảnh.
- **`browser-worker`:** không có sandbox và không chạy mã ở server. "Code mode" ở đây là ba cửa (CLI, client JS, 3 meta-tool) để agent dùng shell của chính nó. Cách này hợp với hub có 69 tool, khoảng 32 KB định nghĩa, và agent là Claude Code.

### 3.2. Áp vào `pkm`: lợi và hại cụ thể

**Lợi:**
- Câu cần phép tập hợp hoặc lọc qua nhiều lời gọi (B4, C4, D5, E1–E5) xuống còn 1–2 lượt.
- Kết quả trung gian (danh sách 300 note của `reach`) không đi qua ngữ cảnh.
- Hợp với hướng "Bản đồ bài" (E11.20) sau này.

**Hại:**
1. **Không có sẵn chỗ chạy.** Claude Desktop trên Windows không có shell cho agent, và tính năng "programmatic tool calling" của API Anthropic không áp cho MCP cục bộ trong Desktop (chưa thấy nguồn chính thức nói có). Muốn code mode trên Desktop thì phải nhúng trình thông dịch vào `pkm`. Như vậy là thêm một thành phần C, thêm giới hạn thời gian và bộ nhớ, và thêm một bề mặt cần test trên 4 đích. Điều này đi ngược tinh thần của Q4 (bỏ `rmcp` vì 56 crate và 2,8 MB).
2. **Dữ liệu của `pkm` vốn đã gọn.** Kết quả dài đã được rút gọn (`count`, `by_type`, `handle`), có `more`, và bị cắt ở `--max-chars`. Phần tiết kiệm lớn của code mode ở nơi khác không có ở đây. Ở nhóm D, `related(query)` đã lấy được gần hết lợi ích.
3. **Độ tin cậy của model.** Mỗi lỗi cú pháp hay sai tên tham số trong script tốn thêm một turn. Mô tả tool là "núm" mà playbook dùng để sửa hành vi; nhét mọi thứ vào `run` sẽ làm mờ núm này.
4. **Bộ so khớp và đo lường.** `run` không có golden Python. Log JSONL hiện ghi theo tool, nên với `run` phải ghi thêm các lời gọi con thì mới đếm được "lượt gọi" theo H4.
5. **An toàn không phải vấn đề chính.** Tool đều chỉ đọc, chạy trên máy một người dùng. QuickJS không có module IO thì không đọc ghi được file. Rủi ro thật là treo (vòng lặp vô hạn) giữa lúc server chạy một luồng, giải bằng bộ ngắt và giới hạn bộ nhớ. Note chứa câu chèn lệnh (prompt injection) cũng chỉ khiến agent gọi đúng những tool chỉ đọc vốn đã có, nên không mở thêm quyền gì.
6. **Bảo trì.** API script là thêm một hợp đồng công khai cần ghi changelog và giữ ổn định, bên cạnh 13 tool.

### 3.3. Phương án nhẹ hơn: lấy được bao nhiêu lợi ích
- **`batch` (danh sách lời gọi độc lập):** gần như không lợi, vì Claude đã gọi tool song song trong một turn (C1: `find` 2∥). Bác.
- **Pipeline JSON có tham chiếu (`{"node":{"$ref":"0.items.0.id"}}`):** đây là một DSL nhỏ không có vòng lặp hay điều kiện. Nó tự chọn hit đầu tiên, nên làm hỏng A3 (phải hỏi lại khi trùng tên). Độ phức tạp gần bằng code mode mà lợi ít hơn. Bác.
- **Tool gộp có mục đích:**
  - `related(query|queries|path)` lấy gần hết lợi ích ở nhóm D, tức nhóm mà playbook mục 1 nói là đang mang phần lớn giá trị.
  - Cột `type` trong `search` bớt một lượt `note` ở A2.
  - Đây là những gì phương án B làm. Ước lượng phần này lấy được khoảng 70–80% lợi ích tính theo các câu trong playbook A–D. Phần còn lại nằm ở B4, C4, D5.

### 3.4. Kết luận
Không làm code mode ở phase 17. Ghi một điều kiện để xem lại vào plan:
- Sau khi đo phase 17 trên bản B, nếu B4, C4, D5 hay nhóm E vượt giới hạn H4 (nhóm B dưới 3 lượt, nhóm C dưới 5) ở từ 2 câu trở lên, hoặc user thấy câu "phép tập hợp" xuất hiện thường xuyên khi viết bài, thì mở một phase riêng cho `run` (phương án C, QuickJS, hybrid, tắt mặc định bằng cờ `--code-mode` cho tới khi đo xong).
- Cửa CLI kiểu `browser-worker` cho chat Obsidian chỉ cần tài liệu trong `CLAUDE.md` mẫu. Nếu muốn, thêm envelope `--json` khi làm việc với plugin (PRD D4), vì plugin cũng gọi `pkm` qua `child_process`.

## 4. Khuyến nghị, chuyển đổi, changelog

### 4.1. Làm gì ở phase 17
1. Đổi tên server thành `pkm` (Q2 đã chốt). Đổi dòng đầu và đoạn cuối của `instructions` như phác ở phương án B.
2. Thêm `search` theo phương án B: tập tham số đã chọn, tên tham số của MCP OHS, tự đổi sang stdio, kết quả gọn có `type`.
3. Thêm `read` theo phương án B: làm tại chỗ, nếu user đồng ý Q-A.
4. Sửa `related`:
   - `query` thành đường chính; thêm `queries` và `path`.
   - Thứ tự hạt giống: `hits`, rồi `path`, rồi `query`.
   - Đổi giá trị `seed_source` và lời lỗi `NO_OHS_INDEX`.
   - `OHS_SEEDS = 10` giữ nguyên.
5. Sửa chỗ `compact()` làm rơi hai khoá của `ohs_not_ready` (mục 1.4.1).
6. Thêm thứ tự tool vào cuối danh sách (`search`, `read` sau `more`) để phần lệch của bộ so khớp tầng MCP nhỏ nhất. Thứ tự này ít ảnh hưởng tới việc agent chọn tool, vì mô tả mới là thứ quyết định.
7. `pkm mcp install`:
   - Chuyển mục `kg` do chính nó tạo thành `pkm`.
   - Thôi đăng ký `ohs` do chính nó tạo; giữ mục `ohs` user tự thêm.
   - `pkm doctor` cảnh báo khi còn MCP `ohs` cùng vault: hai tool `search`, hai lần nạp model.
8. Đo: chạy nhóm A–D trên bản B với hai cửa và với một cửa. Ghi lượt gọi từ log `--log` và token mỗi câu từ log phiên của Agent Client; ghi thêm cỡ `tools/list` và `instructions` (ký tự và token).

### 4.2. Tương thích
- 11 tool cũ giữ nguyên tên, tham số, mô tả và dạng kết quả. Chỉ `related` đổi: mô tả, thêm hai tham số, và giá trị `seed_source`. Lời gọi cũ `related(hits=…)` cho kết quả như trước.
- Project instructions và `CLAUDE.md` của vault nhắc `kg` hay "OHS search": cập nhật bản mẫu khi user cho phép (đã có trong plan).
- Bộ so khớp: ghi ngoại lệ có tên trong `parity/exceptions.toml` và mục "Lệch có chủ ý" trong `spec/PORTING.md` cho:
  - `serverInfo.name`;
  - `instructions` (hai đoạn);
  - mô tả và schema của `related`;
  - `seed_source`;
  - hai tool mới.

### 4.3. Nội dung `docs/MCP-CHANGELOG.md`

Khung đề xuất: mỗi bản một mục, mỗi thay đổi một dòng. Mỗi dòng có loại (thêm, đổi, bỏ, sửa lỗi), tool, trước, sau, cách chuyển.

```markdown
# Thay đổi hợp đồng MCP của `pkm`

Hợp đồng gốc: MCP `kg` của bản Python ở sha ghim (PRD mục 8). Mục này chỉ ghi phần khác.

## 0.2.0 (phase 17, một cửa), YYYY-MM-DD
| Loại | Chỗ | Trước | Sau | Cách chuyển |
|---|---|---|---|---|
| Đổi | tên server | `kg` | `pkm` | `pkm mcp install` tự đổi mục; sửa Project instructions nhắc `kg` |
| Thêm | `search` | (MCP `ohs`) | tham số: query, queries, path, mode, scope, tag, frontmatter, paths, limit, threshold, snippet_length; kết quả bỏ links/backlinks/markdown*/urls/scores, thêm type | dùng `search` của `pkm`; đi link bằng `neighbors` |
| Thêm | `read` | (MCP `ohs`) | paths, snippet_length, related; đọc tại chỗ, không cần OHS | như OHS `read` |
| Đổi | `related` | `query` chỉ dùng khi không có hits | `query`/`queries`/`path` là đường chính; `hits` vẫn nhận | bỏ bước search trước related |
| Đổi | `related.seed_source` | `hits`, `ohs-cli` | `hits`, `query`, `path` | |
| Đổi | `instructions` | phối hợp hai server | một cửa | |
| Sửa lỗi | `ohs_not_ready` | khoá null bị bỏ ở tầng MCP | luôn đủ năm khoá | |
| Bỏ | (MCP `ohs`) `status`, `reindex` | | không có trong `pkm`; trạng thái nằm trong lỗi `ohs_not_ready`, reindex do OHS tự làm | |
```

## 5. Câu hỏi mở cho user (chỉ những quyết định thật)

- **Q-A. `read` làm tại chỗ hay chuyển tiếp sang OHS?**
  - Đề xuất: làm tại chỗ. Lý do: đọc được cả khi OHS đang index, đã có thiếu model hoặc hỏng; không phụ thuộc bản vá P04; dạng kết quả vẫn giống OHS.
  - Giá phải trả: lệch khỏi PRD 3.1 ("chuyển tiếp mỏng"), và `links`/`backlinks` lấy từ index của `pkm` nên có thể khác OHS ở ca biên (link markdown, alias).
- **Q-B. `search` phơi tập tham số đã chọn và bỏ mảng link trong kết quả, hay chuyển tiếp nguyên?**
  - Đề xuất: chọn lọc. Đổi lại bớt khoảng 1,4 nghìn token định nghĩa và một phần lớn kết quả ở note trung tâm.
  - Giá phải trả: tính năng mới của OHS không tự có, và agent không đi wikilink qua `search` được nữa (phải dùng `neighbors`).
  - Nếu user muốn giữ `rerank` thì cần biết máy khách có chịu được lần tải khoảng 570 MB hay không.
- **Q-C. Có ghi điều kiện xem lại code mode (mục 3.4) vào plan không?** Hay bỏ hẳn ý code mode cho tới khi có `pkm serve`?

## 6. Rủi ro và cách kiểm

| Rủi ro | Cách giảm hoặc kiểm |
|---|---|
| Agent vẫn gọi `search` rồi `related(hits)` theo thói quen cũ | Câu "do not run search first just to feed hits" có trong mô tả `related` và trong `instructions`; đếm trong log D1, D3 |
| Bỏ mảng link làm agent mất ngữ cảnh ở A2 | So điểm A2, B1 giữa hai cửa và một cửa; nếu tụt thì thêm lại `links` dạng đếm |
| Đổi tên tham số MCP sang stdio sai (`path` sang `notePath`, gộp `queries`) | Test chuyển tiếp với OHS giả (`tests/ohs_fake.rs`) cho từng tham số; một test thật trên kg-solo |
| Hai MCP cùng có `search` khi user giữ `ohs` | `instructions` và `pkm doctor` |
| `read` tại chỗ lệch OHS ở NFC/NFD, đường dẫn ra ngoài vault | Chuẩn hoá NFC như `note`; từ chối đường dẫn có `..` hoặc nằm ngoài vault |

Tiêu chí xong (thêm vào nghiệm thu của phase 17):
- `tools/list` của `pkm` ≤ 4,6 nghìn token trên kg-solo.
- D1, D3 mỗi câu ≤ 2 lượt gọi.
- `read` trả kết quả khi OHS ở trạng thái `indexing`.
- `ohs_not_ready` có đủ năm khoá ở mọi trạng thái.

## Nguồn ngoài
- [Advanced Tool Use on the Claude Developer Platform (ghi chép)](https://snowan.gitbook.io/study-notes/ai/ai-article-analysis/advanced-tool-use/comic/source): tool search, programmatic tool calling là tính năng của API.
- [MCP tool search trong Claude Code](https://kuanhaohuang.pages.dev/mcp-tool-search-claude-code): Claude Code hoãn nạp định nghĩa tool.
- [Programmatic Tool Call MCP (ptc-mcp)](https://glama.ai/mcp/servers/@gallanoe/ptc-mcp/blob/2865cd9bbd6ca960b751251659eaabe23212cf0d/README.md): Claude Code theo gói thuê bao không có programmatic tool calling của API.
- [mcp claude guide 2026](https://agence-scroll.com/en/blog/mcp-claude-guide-2026): nói Claude Desktop có Tool Search; chưa xác minh từ nguồn chính thức.

## 7. Phản biện chéo (so với `kongming-261102-1826-one-door-mcp-interface.md`)

Hai bản giống nhau ở các ý chính: 13 tool; `read` do `pkm` tự đọc; `search` chỉ phơi một phần tham số và cắt gọn kết quả; `related` nhận câu hỏi làm đường chính; không làm code mode; không thêm tool `status`.

**Kongming tốt hơn ở các điểm sau, tôi nhận:**
- Kết quả dùng khoá `id` thay cho `path`, đúng với `NODE_DESC` của các tool cũ. `hits_from` đã nhận cả `id` lẫn `path`, nên không phải sửa gì thêm.
- `read` dùng `nodes`, `max_chars` (mặc định 4000), `offset`. Note sai id thì lỗi nằm trong item đó kèm `did_you_mean`, cả kết quả không bị `isError`. `read` tự giữ ngân sách và cắt đều các thân note, để bộ cắt `--max-chars` (`mcp.rs` dòng 700–709) không chặt giữa JSON. Bản của tôi chép dạng `paths`/`content` của OHS nên dính đúng lỗi chặt giữa JSON này.
- `related` dùng `node` thay cho `path`, cùng tên với các tool graph.
- `search` thêm `system` và mặc định lọc trang hệ thống (tránh bẫy F7); kết quả có `title`.
- Bỏ thêm `tag`, `frontmatter`, `threshold`, `paths`: playbook A–D không có câu nào cần, và bớt được khoảng 150 token.
- `search` chờ tối đa 8 giây khi OHS đang `starting` (đã có sẵn `Ohs::wait_ready`), và hạn chờ tìm là 25 giây.
- Mã lỗi `{"error":"ohs_error","detail"}`.
- Test ghim ngân sách cỡ `tools/list` và `instructions`.
- Đặt `search` cạnh `find`, `read` cạnh `note`. Chèn giữa danh sách không làm tốn thêm ở bộ so khớp, vì `parity/src/compare.rs` dòng 601 so cả file `mcp/tools_list.json`: thêm một tool là đã phải ghi ngoại lệ, dù thêm ở đâu.
- Ghi rõ hai chỗ hệ thống chưa làm được: B4 (không tool nào trả bậc của từng note) và D5 (`select` không lọc được `created <`). Cả hai nằm ngoài phase 17.

**Tôi không đồng ý ở các điểm sau, kèm bằng chứng:**
1. **Số token của `tools/list` hiện tại bị tính thấp.** Kongming tính khoảng 7 KB, tức khoảng 1,9 nghìn token. Golden `parity/golden/223c9c3…/mcp/tools_list.json` ở dạng JSON gọn dài 13.381 ký tự, tức khoảng 3,3–3,8 nghìn token. Riêng bốn tham số lọc cạnh (`rels`, `as_of`, `statuses`, `accepted_only`) đã lặp lại ở 5 tool. Thứ hạng giữa các phương án không đổi, nhưng ngân sách cho bài test kích thước phải đặt theo số đo này: ≤ 17 KB `tools/list` trên kg-solo, không phải 11 KB.
2. **"Đúng 5 khoá (đã có)" chỉ đúng khi `state` là `indexing`.** `mcp.rs` dòng 692 đưa kết quả qua `compact()`, hàm này bỏ khoá `null`. Ở các trạng thái khác, `ohs/state.rs` dòng 64 đặt `progress` và `eta_s` là `null`, nên agent chỉ còn thấy 3 khoá. Phải sửa và test ở tầng MCP.
3. **Không thêm `title` vào mọi dòng của `related`.** Id đã chứa tên file (`Thư mục/Tên.md`), đủ để viết link `obsidian://`; mỗi phần tử thêm khoảng 30 ký tự là phình 25 dòng mỗi lần gọi. Tôi nhận phần `snippet` (160 ký tự), nhưng chỉ cho hạt giống khi gieo bằng `query` hoặc `node`: snippet lấy từ chính lượt tìm OHS, khoảng 0,45 nghìn token, đổi lại agent chọn được note cần đọc.
4. **Không thêm tham số `seeds` vào `related`.** Chưa có câu nào trong playbook cần; giữ hằng `OHS_SEEDS = 10`.
5. **Cửa CLI `pkm VAULT search` chậm.** CLI chạy một tiến trình OHS và nạp model cho mỗi lần gọi (`cli.rs` dòng 539). Chỉ nên thêm `read` vào CLI (rẻ, không cần OHS); `related` đã có sẵn. Nếu vẫn thêm `search` thì phải ghi rõ độ trễ trong tài liệu.

**Bộ tool sẽ phát hành** (thứ tự trong `tools/list`; 11 tool cũ giữ nguyên câu chữ, trừ `related`):

| Tool | Tham số (in đậm là bắt buộc) | Kết quả |
|---|---|---|
| `find` | **name**, type, limit | như nay |
| `search` | query, queries, path, mode, scope, limit (10), snippet_length (200), (system) | `{count, mode, items:[{id, title, type, score, snippet}]}` |
| `note` | **node**, body_chars | như nay |
| `read` | **nodes** (1–20), max_chars (4000), offset | `{items:[{id, title, type, url, chars, truncated, body}` hoặc `{id, error, did_you_mean}]}` |
| `neighbors`, `reach`, `paths`, `loops`, `provenance`, `select`, `valid` | như nay | như nay |
| `related` | query, queries, node, hits, k (15), types, rels, as_of, statuses, accepted_only, (system) | như nay, cộng `snippet` cho hạt giống khi gieo bằng `query`/`node`; `seed_source` có giá trị `query`, `node` hoặc `hits` |
| `more` | **handle**, offset, limit | như nay |

Ghi chú cho bảng:
- Ưu tiên hạt giống của `related`: `hits`, rồi `node`, rồi `query`/`queries`.
- Lỗi: `ohs_not_ready` đủ 5 khoá ở mọi trạng thái (sửa `compact`); `ohs_error` kèm `detail`; thiếu tham số hay sai id trả câu lỗi ngắn, sai id kèm `did_you_mean`.
- `instructions`: lấy đoạn chọn tool của Kongming (theo cách người dùng nói: gọi tên thì `find`, mô tả thì `search`, cả một loại thì `select`; `find` rỗng thì thử `search`; gặp `ohs_not_ready` thì tiếp tục bằng tool graph). Đổi "(kg)" thành "(pkm)".
- Câu hỏi mở cho user: giữ Q-A và Q-C của mục 5. Q-B được thay bằng câu 5 của Kongming (có mở `tag`/`frontmatter`/`threshold` không; đề xuất là không). Thêm câu 4 của Kongming: `pkm mcp install` có làm ấm index OHS ngay lúc cài không.
