# Kongming: giao diện MCP cho một cửa `pkm`

- Ngày: 2026-11-02 (Asia/Saigon). Vai trò: tư vấn độc lập, không sửa mã.
- Góc nhìn chấm: agent tiêu thụ (Claude trong Claude Desktop trả lời câu hỏi về vault). Tiêu chí: ít lượt gọi, ít token, chọn tool không mơ hồ, lỗi có cấu trúc, mặc định tốt.
- Bằng chứng đã đọc: `plans/261102-0535-rust-core-port-master/phase-17-one-door-mcp.md`; `docs/PRD.md` mục 3.1, 8, 9, R14, Q1, Q2; `docs/DECISIONS.md` (Q4, Q2, gói OHS `vantt.3`); `crates/pkm-core/src/tools/defs.rs` (schema và mô tả 11 tool), `crates/pkm/src/mcp/instructions.rs` (khối `instructions`), `crates/pkm/src/mcp.rs` (`compact`, `is_error`, cắt `--max-chars`), `crates/pkm-core/src/tools.rs` (`related`, `need`/`did_you_mean`, hằng `NO_OHS_INDEX`), `crates/pkm-core/src/related.rs` (`hits_from`, dạng kết quả), `crates/pkm-core/src/ohs.rs` và `ohs/state.rs` (giao thức, `wait_ready`, lỗi `ohs_not_ready`); fork OHS tag `v0.17.2-vantt.4`: `src/transports/stdio-server.ts`, `src/contracts/boundary-validation.ts`, `src/transports/mcp-runtime.ts` (mô tả tool `search`/`read` mà agent đang thấy), `src/search/searcher.ts` (`SearchResult`, `readNotes`), `vantt/patches/0004-stdio-parity.spec.md`; repo đặc tả: `docs/capture-to-producer/playbook/07-kg-challenges.md` (nhóm A–D), `docs/knowledge-graph/install-guide-kg-mcp.md` mục 8, `scripts/e11/vault-agent.md`; repo `vantt/browser-worker`: `docs/agent-cli-and-connect.md` mục "Code mode", `hub/src/mcp_code_mode.rs`, `plans/reports/hub-mcp-code-mode-report.md`.

## TL;DR

Một cửa nên là **13 tool**: 11 tool graph giữ nguyên, thêm `search` (chuyển tiếp mỏng sang OHS nhưng chỉ lộ 7 tham số và trả 5 trường mỗi kết quả, có thêm `type` từ index), thêm `read` (**đọc thẳng file trong vault bằng `pkm`, không đi qua OHS**), và `related` nhận `query` hoặc `node` làm đường chính. Không thêm tool `status`, không làm "code mode": cơ chế của browser-worker là tìm tool lười (`tools_search`/`tool_describe`/`tool_call`) cộng CLI và client JS cho agent có shell, sinh ra để giấu danh sách 57–69 tool; `pkm` chỉ có 13 tool và chạy trong Claude Desktop không có shell, nên áp vào sẽ tăng lượt gọi và làm mờ lựa chọn `find`/`search`. Phần đáng mượn duy nhất là cửa CLI cho Claude Code (`pkm VAULT search|read|related`) và bài test ngân sách kích thước `tools/list`.

## 1. Bài toán thật

Không phải "ghép hai server", mà là **thiết kế lại bề mặt mà model nhìn** sao cho với mỗi câu hỏi nhóm A–D, model chọn đúng tool ngay lượt đầu và nhận về đúng lượng dữ liệu cần để trả lời có nguồn. Ràng buộc: 11 tool cũ giữ câu chữ (so khớp Python); tool mới và `related` là lệch có chủ ý (`phase-17`, PRD mục 8); JSON-RPC tự viết, đồng bộ (Q4); D7: tool cần OHS không được lùi về kết quả kém; đa số người dùng Windows, note tiếng Việt.

Số đo hiện tại mà agent phải trả mỗi phiên (ước lượng từ chuỗi trong mã; 1 token xấp xỉ 3,5 ký tự JSON tiếng Anh, 2,7 ký tự văn bản tiếng Việt):

| Thứ model nạp mỗi phiên | Hai cửa hôm nay | Một cửa đề xuất |
|---|---|---|
| `tools/list` của `kg`/`pkm` | 11 tool, khoảng 5,3 KB chữ + bao JSON ≈ 7 KB ≈ 1,9 k token | 13 tool, khoảng 7,8 KB chữ ≈ 10 KB ≈ 2,7 k token |
| `tools/list` của `ohs` | 4 tool, khoảng 9,5 KB chữ ≈ 11 KB ≈ 2,8 k token (riêng `search` của OHS có 15 tham số, mỗi tham số một đoạn văn) | 0 |
| `instructions` | `kg` ≈ 3–4 KB trên bản B ≈ 1 k token; OHS không có | ≈ 1,1 k token (thêm một đoạn chọn tool) |
| **Cộng** | **≈ 5,7 k token** | **≈ 3,8 k token** |

Mỗi câu hỏi: kết quả `search` thô của OHS (links, backlinks, markdownLinks, scores{4 số}, matchedBy, tags, aliases, urls cho mỗi hit, in `JSON.stringify(..., null, 2)`) nặng 400–900 byte một hit; bản cắt gọn của `pkm` nặng 150–250 byte. Đây là chỗ tiết kiệm lớn nhất mỗi câu, lớn hơn cả việc bớt một lượt gọi.

## 2. Bộ tool đề xuất

Thứ tự trong `tools/list` (thứ tự là thứ tự model đọc; đặt `search` cạnh `find`, `read` cạnh `note` để hai cặp dễ nhầm nằm sát nhau): `find`, `search`, `note`, `read`, `neighbors`, `reach`, `paths`, `loops`, `provenance`, `select`, `valid`, `related`, `more`. Chèn vào giữa là lệch có chủ ý ở tầng so khớp MCP (ghi ngoại lệ có tên); nếu muốn giữ so khớp sạch nhất thì nối `search`, `read` vào cuối, chấp nhận kém hơn một chút về chọn tool.

Quy ước chung cho tool mới: khoá note trong kết quả là `id` (đúng `NODE_DESC` hiện có: "Note id = vault path exactly as returned by find/other tools"); `hits_from` của `related` đã nhận cả `id` lẫn `path` (`related.rs` dòng 440–447), nên không phải đổi gì ở đó. Tham số của `search` giữ tên của OHS (`query`, `queries`, `path`, `mode`, `scope`, `limit`, `snippet_length`) theo đúng yêu cầu của phase 17; tham số của `read` theo tên của `pkm` (`nodes`) vì `read` không còn là chuyển tiếp (xem mục 2.2).

### 2.1. `search`

Mô tả (đúng câu agent thấy):

```text
Find notes by meaning or words: topics, ideas, half-remembered content ("cái bài về…"), questions whose wording differs from the note, bilingual (question in Vietnamese, note in English), typos and missing diacritics. Hybrid BM25 + embeddings via the vault's search index. Use it when the user DESCRIBES what a note is about; use `find` when the user NAMES a note, person or thing; use `select` for every note of a type. Returns ranked items: id (note id, the same id the graph tools take), title, type, score (0-1), snippet. A snippet is an excerpt: call `read` before quoting or judging relevance. Result {"error":"ohs_not_ready",...} means the search index is starting or indexing (state, progress, eta_s, action): tell the user, answer with find / select / graph tools meanwhile, and never conclude from it that a note does not exist.
```

Schema:

```json
{
  "type": "object",
  "properties": {
    "query": {"type": "string", "description": "What to look for, in the user's words (Vietnamese or English). Omit only when `path` is given."},
    "queries": {"type": "array", "items": {"type": "string"}, "description": "2-4 reformulations searched together and merged (better recall; add an English or Vietnamese translation of the question when the note may be in the other language)."},
    "path": {"type": "string", "description": "Note id. Return notes semantically similar to this note instead of searching text (what an isolated note could connect to). Ignores query."},
    "mode": {"type": "string", "enum": ["hybrid", "semantic", "fulltext", "title"], "description": "hybrid (default) for most questions; semantic when the user's wording differs from the note's; fulltext for an exact term; title only to navigate to a named note (prefer `find`)."},
    "scope": {"oneOf": [{"type": "string"}, {"type": "array", "items": {"type": "string"}}], "description": "Folder prefix(es) to search in, e.g. \"Dự án/\"; prefix \"-\" to exclude. Only when the user limits the area."},
    "limit": {"type": "integer", "description": "Items returned (default 10; keep <= 15, later ranks are weak)."},
    "snippet_length": {"type": "integer", "description": "Snippet characters per item (default 200; 0 = none)."},
    "system": {"type": "boolean", "description": "(chỉ khi index có trang ẩn, như các tool khác) true = also return system pages."}
  },
  "required": []
}
```

Kết quả:

```json
{"count": 10, "mode": "hybrid",
 "items": [{"id": "Trích dẫn/Marcus về kỷ luật.md", "title": "Marcus về kỷ luật", "type": "quote", "score": 0.83, "snippet": "…"}]}
```

Cách làm bên trong: gọi `Ohs::search(query, {mode, limit, scope, snippetLength, notePath, queries})`; đường dẫn đã được chuẩn NFC trong module; tra `type` và `title` từ index theo `id`; bỏ item có `id` nằm trong `meta.hidden` trừ khi `system: true` (nhất quán với "tools skip them by default" của `instructions`, và tránh bẫy F7); bỏ hết trường khác của OHS. Khi OHS ở trạng thái `starting` (chưa `ready`, thường 2–5 giây sau khi Claude Desktop mở), **chờ tối đa 8 giây** bằng `Ohs::wait_ready` rồi mới trả `ohs_not_ready`; khi `indexing`, `model_missing`, `broken` thì trả ngay. Hạn chờ phản hồi `search` nên ngắn hơn hạn của client (đề xuất 25 giây) để agent nhận lỗi có cấu trúc thay vì lỗi chung của client rồi gọi lại.

Vì sao không lộ `tag`, `frontmatter`, `threshold`, `paths`, `related`/`depth`/`direction`/`link_type`, `rerank`, `anchors`, `explain`: `frontmatter` trùng việc của `select`; `related`/`depth`/`direction` là đi graph không loại, `neighbors`/`reach` làm tốt hơn và có lớp cạnh; `rerank` tải model 570 MB về máy khách Windows mà không ai duyệt; `anchors`/`explain` cho UI và đánh giá. Mỗi tham số thừa là vài chục token mỗi phiên và một cách chọn sai nữa. Thêm lại khi có câu trong playbook cần.

### 2.2. `read` (đọc thẳng, không qua OHS)

Đây là chỗ tôi lệch với câu chữ của phase 17 ("`read` chuyển tiếp mỏng sang OHS") và đề nghị đổi plan. Lý do, từ góc agent:

1. Đọc một file không cần index nghĩa. Nếu `read` đi qua OHS thì trong 10 phút OHS index bản B lần đầu, hoặc khi thiếu model, agent **không đọc được note nào** dù `find`/`note` vẫn chạy; D7 nói rõ "tool không cần OHS chạy bình thường" (PRD mục 8), và `read` là tool không cần OHS.
2. `pkm` đã có mọi thứ: `ToolContext.vault` để đọc thân note (`tool_layer.rs`: "`note` đọc thân note từ đây"), `need()` trả `did_you_mean` khi sai id (`tools.rs` dòng 566–598), `url` `obsidian://open`, `title`/`type` trong index. OHS `read` chỉ thêm `links`/`backlinks` (index có) và gợi ý tiêu đề gần (đã có).
3. Bớt một chỗ phụ thuộc bản vá P04 của fork và bớt chuyển NFC/NFD đường dẫn.

Mô tả:

```text
Full body text of one or more notes (frontmatter removed), for quoting and for judging relevance. `note` gives metadata and edge counts; `read` gives the text. Pass note ids exactly as returned by search / find / related. Reads several notes in ONE call: after search or related, read the top 5-8 together instead of one call each. Bodies longer than max_chars are cut with truncated:true and chars = full length; call again with offset for the rest. Unknown id: that item carries error + did_you_mean, the others are still returned.
```

Schema:

```json
{
  "type": "object",
  "properties": {
    "nodes": {"type": "array", "items": {"type": "string"}, "description": "Note ids (vault paths), 1-20."},
    "max_chars": {"type": "integer", "description": "Characters per note (default 4000; 0 = whole note). The whole reply is also capped by the server; with many notes each body is cut evenly."},
    "offset": {"type": "integer", "description": "Skip this many characters of each body (default 0)."}
  },
  "required": ["nodes"]
}
```

Kết quả:

```json
{"items": [
  {"id": "Dự án/Blog/Chia user story.md", "title": "Chia user story", "type": "article", "url": "obsidian://open?vault=…&file=…", "chars": 9120, "truncated": true, "body": "…"},
  {"id": "Nguoi/Khahc hang.md", "error": "no note with id 'Nguoi/Khahc hang.md'; ids are exact vault paths", "did_you_mean": ["Người/Khách hàng ABC.md"]}
]}
```

Điểm an toàn bắt buộc: `read` tự giữ ngân sách tổng (ví dụ 80% của `--max-chars`) và cắt đều từng thân trước khi trả, để bộ cắt chung ở `tools_call` (`mcp.rs` dòng 700–709) không chặt đứt giữa JSON; kết quả có `items` thì `is_error` hiện tại (`mcp.rs` dòng 242–245) vẫn coi là không lỗi, đúng ý muốn.

Mặc định 4000 ký tự một note (≈ 1,4 k token tiếng Việt) là ngưỡng đủ để trích câu; 8 note ≈ 11 k token, là giá của việc "đọc rồi mới nói quan hệ" mà playbook đòi (D1 cột "Hỏng thì").

### 2.3. `related` (đổi đường chính)

Mô tả:

```text
Notes related to a question, ranked by personalised PageRank over the typed graph and seeded by semantic search: ONE call = search + graph walk. Give `query` (the user's question or a note's main idea), or `node` (a note id: start from notes similar to it, e.g. what an isolated note could connect to), or `hits` (search results you already hold). Returns seeds (the search hits re-ranked by graph support, found_by "ohs", with snippet) and related (notes only the graph reached, found_by "graph": what text search missed), each with via = the edge chain from a seed and layer (accepted | proposed, say which). Use for "liên quan gì / nối với gì / có thể nối với gì"; then `read` the top notes before stating a relation. accepted_only=true shows what the user's own links contribute. ohs_not_ready error: as in search.
```

Schema (phần đầu; các bộ lọc `rels`, `as_of`, `statuses`, `accepted_only`, `system` giữ nguyên như nay):

```json
{
  "query": {"type": "string", "description": "Question or idea to search, then walk the graph from the hits."},
  "node": {"type": "string", "description": "Note id: seeds are the notes most similar in meaning to this note (the note itself excluded)."},
  "hits": {"type": "array", "items": {}, "description": "Search results you already have: objects with id or path (and score), or id strings, in search order. Skips the search step."},
  "k": {"type": "integer", "description": "Graph-only notes returned (default 15)."},
  "types": {"type": "array", "items": {"type": "string"}, "description": "Only return notes of these types."},
  "seeds": {"type": "integer", "description": "Search hits used as seeds when seeding by query or node (default 10)."}
}
```

Ba nguồn hạt giống loại trừ nhau; thứ tự ưu tiên khi gửi nhiều hơn một: `hits` > `node` > `query`. Kết quả giữ nguyên các trường hiện có (`seeds`, `related`, `missing`, `method`, `seed_source`), thêm `snippet` (160 ký tự, lấy từ OHS cùng lượt tìm, `snippetLength` 160 thay vì 0 như `tools.rs` dòng 1007) cho hạt giống khi gieo bằng `query`/`node`, và thêm `title` cho mọi dòng (agent cần tiêu đề để viết link mở note; không có thì nó gọi `note` từng cái). Đổi `seed_source` thành `"query" | "node" | "hits"` thay cho `"ohs-cli"` (chuỗi đó mô tả cách của Python, không còn đúng). Mọi thay đổi ghi vào `docs/MCP-CHANGELOG.md`.

Cần đổi bên trong: `Ohs::search` hiện từ chối câu rỗng (`ohs.rs` dòng 378–381); với `node` gửi `query` = chính id và `options.notePath` = id (OHS bỏ qua `query` khi có `notePath`, theo mô tả tool `path` của chính OHS). Hai chuỗi lỗi đang bảo agent "call the OHS search tool first" (`tools.rs` dòng 975 `NO_OHS_INDEX`, dòng 980 `no_ohs_state`) phải viết lại, vì một cửa không còn tool đó để gọi.

### 2.4. Không thêm `status`

Lỗi `ohs_not_ready` đã mang `state`, `progress`, `eta_s`, `action`; agent không ngủ chờ được nên thông tin này chỉ để nói lại với người dùng, và nó đến đúng lúc cần (khi gọi `search`). Một tool `status` tốn khoảng 150 token mỗi phiên, thêm một lựa chọn, và `pkm doctor` đã là chỗ cho người. Nếu sau này cần "index graph dựng xong chưa" thì trả trong `instructions` (đã có `pending`).

### 2.5. Khối `instructions`

Thay đoạn cuối hiện tại ("Search vs graph: if the OHS server … is also connected …", `instructions.rs` dòng 245) bằng:

```text
One server does both search and graph. Choose by how the user refers to things: they NAME a note, person, client, factor -> find, then neighbors / reach / paths / loops / note. They DESCRIBE content, a topic, an idea, or remember vaguely -> search; add related when they also ask what it connects to or what else is relevant (related alone = search + graph in one call). Every note of a type -> select. Before quoting or judging relevance -> read (several ids in one call). Ids from search are the same ids the graph tools take. find returns nothing -> try search before saying the note does not exist. search and related need the vault's search index: {"error":"ohs_not_ready"} means it is starting or indexing (state, progress, eta_s, action): tell the user that and what to do, keep answering with find / select / graph tools, never treat it as "not found". If a separate `ohs` server is also connected, prefer the tools here: one index, ids match the graph.
```

Giữ các dòng còn lại nguyên văn (mô tả vault, loại note, quan hệ, lớp cạnh, link `obsidian://open`). Khối này tăng khoảng 120 token so với nay.

### 2.6. Hợp đồng lỗi (toàn server)

| Tình huống | Kết quả (`isError: true` trừ khi ghi khác) | Agent nên làm |
|---|---|---|
| OHS chưa sẵn sàng (`search`, `related` gieo bằng `query`/`node`) | `{"error":"ohs_not_ready","state":"starting|indexing|model_missing|broken","progress":0.64,"eta_s":180,"action":"…"}` đúng 5 khoá, thứ tự này (đã có, `state.rs` dòng 58–73) | Nói trạng thái và việc cần làm; dùng tool graph |
| OHS trả lỗi, hết hạn, sai định dạng | `{"error":"ohs_error","detail":"…"}` (2 khoá, hợp luật `is_error`) | Báo lỗi, không gọi lại quá một lần |
| Thiếu tham số | `{"error":"search needs query, queries or path"}`, `{"error":"related needs query, node or hits"}`, `{"error":"read needs nodes"}` | Sửa tham số |
| Id sai ở tool một note | `{"error":"no note with id '…'; ids are exact vault paths","did_you_mean":[…]}` (đã có) | Chọn trong `did_you_mean` hoặc `find` |
| Id sai trong `read` nhiều note | Lỗi nằm **trong item** đó, cả kết quả không `isError` | Trả các note còn lại, nêu note thiếu |
| Index graph chưa dựng | `{"error":"index unavailable: …"}` (đã có) | Chờ, báo người dùng |
| Kết quả dài | `count`, `by_type`, `items`, `truncated`, `handle` (đã có); `read` tự cắt đều | `more` chỉ khi cần đủ danh sách |

Không phát minh mã lỗi mới ngoài `ohs_not_ready` và `ohs_error`; phần còn lại là câu tiếng Anh ngắn như Python để giữ một kiểu.

## 3. Đi qua từng câu nhóm A–D

Token là ước lượng phần kết quả tool (phần chiếm chính); lời gọi vào tốn 60–120 token mỗi lượt. "Hai cửa" = `kg` + MCP `ohs` với mô tả hiện tại; "một cửa" = bộ ở mục 2. Số lượt gọi tối thiểu mà một agent chọn đúng sẽ làm; câu nào agent thực tế hay làm thừa thì ghi ở cột ghi chú.

| Mã | Hai cửa: lượt (token) | Một cửa: lượt (token) | Ghi chú |
|---|---|---|---|
| A1 gõ không dấu, có tên | `find` 1 (0,2 k) | `find` 1 (0,2 k) | Bằng nhau |
| A2 nhớ nội dung | `ohs.search` → `ohs.read` hoặc `kg.note` 2 (3–4 k + 1,5 k) | `search` → `read` 2 (0,9 k + 1,5 k) | Bằng lượt, bớt khoảng 2,5 k token nhờ cắt kết quả tìm |
| A3 hai note trùng tên | `find` 1 (0,3 k), hỏi lại | như nhau | Mô tả `find` đã dặn hỏi lại |
| A4 gọi tên tiếng Anh | `ohs.search` 1 (3 k) | `search` với `queries` [vi, en] 1 (0,9 k) | Mô tả `queries` nói rõ việc dịch; `find` không khớp được |
| A5 gõ sai chữ | `find` rỗng → `ohs.search` 2 (0,1 k + 3 k) | `find` rỗng → `search` 2 (0,1 k + 0,9 k) | `instructions`: "find rỗng → thử search trước khi nói không có" |
| A6 "mấy công cụ" | `select type` 1 (0,4 k) | như nhau | Dòng hai của `instructions` đã chặn việc liệt kê tool MCP |
| B1 nối với gì | `find` → `neighbors` 2 (0,2 k + 1,2 k) | như nhau | |
| B2 chỉ link tự viết | `find` → `neighbors accepted_only` 2 | như nhau | |
| B3 đếm câu của tác giả | `find` → `neighbors rels:[author] dir:in` 2 (0,2 k + 0,5 k tóm tắt) | như nhau | Đọc `count`, không đếm tay |
| B4 thư mục có gì, cái nào cô lập | `find` → `neighbors system:true` → nhiều `neighbors` 3+ | như nhau | Khoảng trống chung: không tool nào trả bậc của từng note; không thuộc phạm vi một cửa, ghi nhận |
| B5 bản trùng | `find` → `neighbors rels:[duplicate_of]` 2 | như nhau | |
| B6 hàng chờ đề xuất người | `select type:proposal status:proposed` 1 (0,4 k) | như nhau | |
| C1 hai vùng nối qua đâu | `find`×2 → `paths` 3 (0,4 k + 1,2 k) | như nhau | |
| C2 chỉ qua link tự viết | 3, `paths accepted_only` | như nhau | Trả "không có đường" là đúng |
| C3 từ chủ đề đi ra vùng nào | `find` → `reach max_depth` 2 (0,2 k + 0,5 k tóm tắt) | như nhau | |
| C4 note cầu nối | `find`×2 → `reach`×2 hoặc `paths` 3–4 | như nhau | |
| C5 sâu 2 bước, lớp đã duyệt | `find` → `reach max_depth:2 accepted_only` 2 | như nhau | |
| C6 vòng quanh X | `find` → `loops max_len:6` 2 (0,2 k + 0,6 k) | như nhau | |
| **D1** câu trích liên quan gì tới quản lý đội | `ohs.search` → `kg.related(hits)` → `ohs.read`(5) **3** (3,5 k + 2 k + 7 k ≈ 12,5 k) | `related(query)` → `read`(5) **2** (2,6 k + 7 k ≈ 9,6 k) | Bớt một lượt và khoảng 3 k token; `snippet` trong hạt giống giúp chọn 5 note để đọc |
| D2 hỏi tiếng Việt, đáp án tiếng Anh | `ohs.search` (+ `read`) 1–2 | `search queries:[vi,en]` (+ `read`) 1–2 | Token tìm giảm 3 lần |
| **D3** note cô lập nối được với gì | `ohs.search path:X` → `kg.related(hits)` → `read` **3** | `related(node:X)` → `read` **2** | Cần tham số `node` ở mục 2.3; không có thì `search path` → `related(hits)` vẫn 3 lượt như hai cửa |
| D4 D1 với `accepted_only` | `ohs.search` → `related`×2 3 | `related(query)` + `related(query, accepted_only)` 2 | Là số đo, không phải đạt/hỏng |
| D5 ba note cũ nhất chưa dùng về Y | `ohs.search` → `note`×n ≥ 4 | `search` → `read`/`note`×n ≥ 4 | Khoảng trống chung: `select` so chuỗi, không lọc `created <`; nếu D5 phải đạt thì thêm `sort`/`before` cho `select` ở phase khác |

Kết luận đo: không câu nào một cửa nhiều lượt hơn hai cửa; D1, D3 bớt một lượt; mọi câu có bước tìm bớt 2–3 k token. Ngưỡng H4 (nhóm B dưới 3 lượt, nhóm C dưới 5) đều trong tầm. Cách đo thật: chạy `pkm VAULT mcp --log FILE.jsonl` (đã ghi `tool`, `chars`, `ms`) với `claude -p --strict-mcp-config` như install-guide mục 7.2, mỗi câu một run, lấy token từ usage của run; chạy cùng bộ câu với cấu hình hai cửa để có bảng so như R14 đòi.

## 4. Code mode: kết luận **từ chối**, giữ "hợp phần hoá" và cửa CLI

Cơ chế trong `vantt/browser-worker` (đã đọc mã, không suy đoán):

1. Profile MCP `code` = ba meta tool `tools_search(query, profile, limit)` → `tool_describe(name)` → `tool_call(name, args)` (`hub/src/mcp_code_mode.rs`, 104 dòng, xếp hạng bằng khớp từ trong tên và mô tả). Mục đích: giấu danh sách 57–69 tool (28–32 KB) sau 863 byte. Đây là **tìm tool lười**, không phải thực thi mã.
2. Cửa CLI `browser-worker-hub agent <tool> key=value --json` cộng `jq`, và client JS `bw-client.mjs` sinh từ schema (`agent emit-client`): phần "code" thật sự là agent có shell (Claude Code) tự viết script Node/bash gọi REST của hub, vòng lặp và kết quả lớn nằm ngoài ngữ cảnh.

Áp vào `pkm`:

- Tìm tool lười: `pkm` có 13 tool, danh sách ≈ 10 KB. Lười hoá biến lượt đầu của mỗi phiên thành 3 lượt (`tools_search` → `tool_describe` → `tool_call`), và quan trọng hơn, model **không còn nhìn thấy `find` và `search` cạnh nhau** để chọn; mà chọn sai ở bước resolve là lỗi gốc của cả playbook (mục A). Lợi 2 k token mỗi phiên không bù nổi. Từ chối.
- Thực thi mã trong `pkm` (agent viết script, `pkm` chạy, gọi nhiều tool một lượt): Claude Desktop không có shell hay sandbox, nên `pkm` phải nhúng một máy chạy script (QuickJS/Lua/Rhai) trên máy khách Windows, mở mặt tấn công mới, vướng antivirus với binary chưa ký, và các chuỗi gọi trong A–D sau khi thiết kế lại dài tối đa 2–3 lượt, trong đó bước "đọc 5 note rồi phán xét" là việc của model, không script được. Giá viết script bằng token cũng xấp xỉ 2 lượt gọi. Từ chối ở giai đoạn này.
- Phần giữ lại ("hybrid"): **hợp phần hoá chuỗi đã biết thành tham số phía server**. `related(query)` chính là "search + graph walk" trong một lượt; `related(node)` là "similar + graph walk". Nếu sau đo đạc D1 vẫn 3 lượt, nấc tiếp theo là `related(..., read_top: 5)` trả luôn thân 5 note đầu (một lượt, cùng số token). Chưa làm ngay; đo trước.
- Cửa CLI: `pkm VAULT find|neighbors|…` đã có (PRD mục 10); thêm `pkm VAULT search|read|related` cùng JSON để Claude Code và plugin có thể script khi cần. Rẻ, không đụng MCP, và là đúng thứ browser-worker làm được cho agent có shell.
- Mượn thêm một thực hành: bài test ghim ngân sách (`tools/list` ≤ 11 KB, `instructions` ≤ 4,5 KB trên kg-solo) để mô tả không phình dần.

## 5. Nên tránh

- Chuyển tiếp nguyên bộ 15 tham số của OHS `search`: tăng 1,5 k token mỗi phiên và thêm cách chọn sai (`related: true` của OHS đi link không loại, trùng và kém `neighbors`).
- `read` qua OHS: hỏng trong lúc OHS index, trái D7 với một tool không cần OHS.
- Lùi về FTS hay `find` **bên trong** `search` khi OHS chưa sẵn sàng (trái D7). Đường lùi đặt ở `instructions` cho agent, không đặt trong tool.
- Mô tả bằng tiếng Việt cho tool mới trong khi 11 tool cũ tiếng Anh: một bộ hai giọng làm model đọc kém hơn; giữ tiếng Anh, chèn từ khoá tiếng Việt như hiện có ("đang", "hiện tại", "cái bài về…").
- Trả `isError: true` cho `read` khi chỉ một id sai: agent sẽ bỏ cả kết quả.
- Để bộ cắt `--max-chars` chặt JSON của `read`: agent nhận JSON hỏng, gọi lại, tốn gấp đôi.
- Giữ nguyên chuỗi "call the OHS search tool first" (`tools.rs` dòng 975, 980): agent sẽ đi tìm một server không còn.

## 6. Lựa chọn khác và cái giá

| Phương án | Được | Mất | Nhận xét |
|---|---|---|---|
| A. Bộ 13 tool như mục 2 (đề xuất) | Ít lượt, ít token, tool cũ nguyên vẹn | Phải giữ 7 tham số và 5 trường khớp OHS khi fork đổi | Cái giá nhỏ: bề mặt tiếp xúc OHS là 7 tên tham số và 4 trường kết quả |
| B. Chuyển tiếp nguyên `search`/`read` của OHS (đúng câu chữ phase 17) | Ít việc nhất, không phải nghĩ tham số | Token mỗi phiên và mỗi câu gần bằng hai cửa; `read` chết khi OHS chưa sẵn sàng; không có `type` trong kết quả tìm | Đạt nghiệm thu "không cao hơn" nhưng bỏ phí lý do làm một cửa |
| C. Một tool `ask(query)` gộp search + related + read | D1 một lượt | Model mất quyền chọn note để đọc; câu A2/A4 chỉ cần tìm cũng trả về 10 k token; khó giải thích khi nào dùng `ask` và khi nào dùng graph | Giữ làm nấc `read_top` của `related` sau khi đo |
| D. Tìm tool lười / code mode | Danh sách gần như rỗng | Thêm 2 lượt mỗi phiên, mờ lựa chọn `find`/`search`, cần sandbox | Từ chối (mục 4) |

## 7. Việc cần làm (theo thứ tự)

1. Sửa plan phase 17: `read` đọc thẳng từ vault; `related` thêm `node`, `seeds`, `snippet`, `title`, đổi `seed_source`; chèn `search`/`read` vào vị trí mục 2 (hoặc nối cuối nếu user muốn so khớp sạch).
2. `pkm-core::tools`: `search` (gọi `Ohs::search`, ghép `type`/`title`, lọc trang ẩn, chờ `starting` tối đa 8 giây); `read` (đọc file, bỏ frontmatter bằng đúng bộ tách của `vault`, cắt theo `max_chars`/`offset`, ngân sách tổng); `related` ba nguồn hạt giống; viết lại hai chuỗi lỗi về OHS.
3. `pkm-core::ohs`: cho phép `notePath` (câu tìm là id), hạn `search` 25 giây có thể cấu hình.
4. `defs.rs`: ba mục mới và `related` sửa; `instructions.rs`: thay đoạn cuối; `is_error` thêm `ohs_error` nếu có hơn 2 khoá.
5. CLI: `pkm VAULT search|read|related` in JSON.
6. Test: hợp đồng lỗi 5 khoá vẫn giữ; `read` với id sai lẫn id đúng; `read` cắt đều dưới `--max-chars`; `search` lọc trang ẩn; bài test ngân sách kích thước `tools/list` và `instructions`; `tests/ohs_fake.rs` cho `notePath`.
7. `pkm mcp install`: ghi mục `pkm`, thôi ghi `ohs` (Q2 đã chốt); `docs/MCP-CHANGELOG.md`; sau khi user cho phép, sửa `vault-agent.md` và install-guide mục 8 của repo đặc tả (hiện cả hai dạy "gọi `ohs` `search` rồi `kg` `related` với `hits`", một cửa sẽ làm agent gọi server không còn).
8. Đo A–D hai cấu hình bằng `--log` và `claude -p`, điền bảng vào báo cáo phase 17.

## 8. Ba rủi ro lớn nhất

1. **Ấn tượng đầu trên Windows**: OHS tải model 130 MB và index 2.000 note khoảng 10 phút sau khi cài; trong lúc đó mọi câu "theo ý" trả `ohs_not_ready`. Giảm: `search` chờ 8 giây khi `starting`; `action` tiếng Việt rõ việc; cân nhắc cho `pkm mcp install` làm ấm index OHS ngay lúc cài (có `--deep` ở `doctor`, chưa có ở `install`); `instructions` dạy agent tiếp tục bằng graph.
2. **Chọn nhầm `find`/`search`/`select`** (gốc của nhóm A, ảnh hưởng mọi nhóm sau). Giảm: ba mô tả nói rõ "NAMES / DESCRIBES / every note of a type", đặt cạnh nhau trong danh sách, quy tắc "find rỗng → search", đo bằng `--log` theo H4; núm sửa là câu chữ, không phải mã.
3. **Trôi hợp đồng với fork OHS** (tên tham số, dạng `results`, `notePath`). Giảm: bề mặt tiếp xúc cố định 7 tham số và 4 trường; `tests/ohs_real.rs` chạy với gói ghim `vantt.3`/`vantt.4`; probe stdio của fork; khi fork đổi thì chỉ sửa một hàm ánh xạ trong `pkm-core::ohs`.

Rủi ro phụ: người dùng giữ MCP `ohs` song song sẽ thấy hai `search` (`ohs:search`, `pkm:search`); câu cuối của `instructions` xử lý; không cấm (đúng PRD 3.1).

## 9. Chỉ user quyết được

1. **`read` đọc thẳng hay chuyển tiếp OHS** (đổi câu chữ phase 17). Đề xuất: đọc thẳng. Bằng chứng đổi ý: nếu plugin hoặc điện thoại cần `read` cho ra đúng dạng `readNotes` của OHS (links/backlinks/markdownLinks) để dùng chung một hợp đồng.
2. **Vị trí tool mới trong `tools/list`**: chèn cạnh `find`/`note` (tốt cho model, thêm ngoại lệ so khớp) hay nối cuối. Đề xuất: chèn.
3. **Đổi `seed_source` `"ohs-cli"` thành `"query"|"node"|"hits"`** và thêm `title`/`snippet` vào `related` (lệch Python có chủ ý). Đề xuất: đổi.
4. **`pkm mcp install` có chạy OHS để index ngay lúc cài không** (thêm 10 phút cài, bớt lỗi lúc dùng). Đề xuất: có, với thanh tiến độ và cờ tắt.
5. **Có mở thêm tham số OHS cho người dùng nâng cao** (`tag`, `frontmatter`, `threshold`) hay giữ 7. Đề xuất: giữ 7 tới khi playbook có câu cần.
6. **Ngôn ngữ mô tả tool**: tiếng Anh như nay hay tiếng Việt. Đề xuất: tiếng Anh, từ khoá tiếng Việt chèn trong câu.
7. **Nấc `related(read_top)`** có làm ở đợt này không. Đề xuất: không; đo D1 trước.

## 10. Giả định đã dùng thay cho câu hỏi

| Giả định | Độ tin | Nếu sai thì |
|---|---|---|
| Tỉ lệ token: ≈ 3,5 ký tự/token cho JSON tiếng Anh, ≈ 2,7 cho văn bản tiếng Việt | trung bình | Các số ở mục 1 và 3 lệch theo tỉ lệ, thứ hạng giữa các phương án không đổi |
| Claude Desktop đưa toàn bộ `tools/list` của mọi server vào ngữ cảnh mỗi phiên và không có bước tìm tool lười riêng | cao | Nếu client tự lười hoá thì lợi ích về kích thước danh sách nhỏ đi, lợi ích về số lượt và token kết quả vẫn còn |
| Hạn chờ một lời gọi tool của Claude Desktop khoảng 60 giây | thấp | Nếu ngắn hơn (30 giây) thì hạ hạn `search` xuống 20 giây |
| OHS stdio nhận `options.notePath` qua đường `search` của `pkm` (schema `SearchOptionsBoundarySchema` có `notePath`, `stdio-server.ts` chuyển sang `SearchOptions`) | cao (đọc mã, chưa chạy) | Nếu `query` rỗng bị từ chối thì gửi id làm `query`; nếu vẫn hỏng thì bỏ `node`, D3 về 3 lượt như hai cửa |
| Fork `v0.17.2-vantt.4` có action `read` stdio (đọc `stdio-server.ts` dòng 168–178) | cao | Không ảnh hưởng vì đề xuất không dùng nó cho MCP `read` |
| Vault bản B có ít note cần `tag`/`frontmatter` filter khi tìm (playbook dùng thư mục và `type`) | trung bình | Mở thêm `tag`/`frontmatter` cho `search` (câu hỏi 5 mục 9) |
| Người dùng cuối không có shell khi chat (Claude Desktop); Claude Code/Agent Client là trường hợp phụ | cao | Nếu Agent Client trong Obsidian là kênh chính thì cửa CLI ở mục 4 lên ưu tiên cao hơn |

Runtime: báo cáo này do Kongming chạy trên `fable` (Claude Fable 5.1), một lượt, không hỏi lại.

## 11. Phản biện chéo với `brainstormer-261102-1826-one-door-mcp-signature.md`

Hai bản gặp nhau ở phần lớn: 13 tool, `read` làm tại chỗ, `search` chọn lọc tham số và bỏ mảng link, `related` nhận câu hỏi và note làm đường chính, từ chối code mode, giữ cửa CLI. Khác nhau ở chi tiết dưới đây.

### Chỗ bản kia hơn, tôi nhận

1. **`compact()` làm rơi hai khoá của `ohs_not_ready`: xác nhận đúng.** `mcp.rs:227–238` lọc `!x.is_null()`, và `tools_call` áp `compact` lên mọi kết quả (`mcp.rs:692`, sau khi `is_error` đã tính ở dòng 691). Ở `starting`/`model_missing`/`broken` thì `progress` và `eta_s` là `null` (`state.rs:59–65`) nên agent chỉ thấy `error`, `state`, `action`. Test `not_ready_error_has_exactly_the_contract_keys_in_order` (`state.rs:326`) kiểm trước `compact` nên không bắt được. Sửa: miễn `compact` cho object có `error == "ohs_not_ready"` (cùng chỗ `is_error` đã có ngoại lệ này) và thêm một test ở tầng `mcp` cho ba trạng thái đó.
2. **Hai bộ tên tham số của OHS**: MCP dùng `snippet_length`, `path`; stdio dùng `snippetLength`, `notePath`, và gộp `query`/`queries` theo cách riêng (`mcp-runtime.ts`: `query` gửi đi là `path ?? query ?? queries[0]`, `options.queries` chỉ khi từ hai câu). Tôi đã dùng tên MCP nhưng chưa nói rõ phép đổi; nhận quy tắc ánh xạ và test từng tham số với OHS giả.
3. **`SERVER_NAME = "kg"` (`mcp.rs:37`) và dòng đầu `instructions` "(kg)"** phải đổi theo Q2; tôi bỏ sót.
4. **`path` mơ hồ qua stdio chỉ còn `{error}`** (MCP của OHS mới trả `candidates`): trả `{"error":"ambiguous path","did_you_mean":[…]}` lấy từ `find`. Nhận.
5. `read` từ chối đường dẫn có `..` hoặc ngoài vault; `pkm doctor` cảnh báo khi còn MCP `ohs` cùng vault; khung bảng `MCP-CHANGELOG.md`; bốn tiêu chí nghiệm thu thêm (ngân sách `tools/list`, D1/D3 ≤ 2 lượt, `read` chạy khi `indexing`, đủ 5 khoá ở mọi trạng thái). Nhận cả.
6. **Số token nền**: bản kia đo từ golden (`tools_list.json` 13.381 ký tự ≈ 3,3 k token cho 11 tool; hai cửa ≈ 6,0 k + 1,2 k `instructions`), chính xác hơn ước lượng từ chuỗi mô tả của tôi (1,9 k). Lấy số của bản kia làm nền; một cửa theo bộ gộp dưới đây ≈ 4,2–4,4 k.
7. `related` nhận thêm `queries` (song ngữ, D2). Nhận.

### Chỗ tôi không đồng ý, kèm bằng chứng

1. **Trường kết quả của `search`**: bản kia giữ `rank`, `tags`, `aliases`, `matchedBy`. `rank` là thứ tự mảng; `matchedBy` là chẩn đoán; `tags`/`aliases` là mảng, và `compact` giữ mảng rỗng (`mcp.rs:226` ghi rõ "danh sách rỗng giữ lại") nên mỗi hit tốn thêm 4 khoá dù trống, ≈ 60–100 token cho 10 hit mà không câu A–D nào dùng. Giữ 5 trường `id, title, type, score, snippet`; thêm lại khi đo thấy thiếu.
2. **Tham số `tag`, `frontmatter`, `paths`, `threshold` của `search`** (bản kia 11 tham số, tôi 7). `frontmatter` làm đúng việc của `select` (`defs.rs:214`: "Notes whose frontmatter matches every given key = value") và tạo hai cửa cho cùng một câu "note có status=todo", là kiểu mơ hồ mà nhóm A/B6 đang đo; `paths` không có câu nào dùng; `threshold` mô tả của chính OHS khuyên để 0 khi cần recall. Giá ≈ 250 token mỗi phiên và thêm cách chọn sai. Giữ 7; `tag` là tham số duy nhất tôi sẵn sàng nhượng nếu bản B dùng tag thật (hỏi user).
3. **Dạng kết quả `read`**: bản kia chép OHS (`found`, `aliases`, `tags`, `links`, `backlinks`, tham số `related`, `snippet_length` mặc định trọn note). `links`/`backlinks` trùng `neighbors` mà không có loại và lớp; `found:false + suggestions` là quy ước thứ hai bên cạnh `error + did_you_mean` đang dùng ở `note` (`tools.rs:593`); mặc định trọn note với 8 note sẽ vượt `--max-chars` 30.000 và bộ cắt chung chặt giữa JSON (`mcp.rs:700–709`). Giữ: `max_chars` mặc định 4000, `offset`, tự giữ ngân sách tổng, lỗi theo từng item với `did_you_mean`. Nhượng **tên tham số `paths`** (chuỗi hoặc mảng) thay `nodes`, vì `vault-agent.md` và playbook 02 đã viết `read` với `paths`.
4. **Vị trí tool mới**: bản kia nối cuối để so khớp sạch, nói thứ tự không ảnh hưởng chọn tool. Không bên nào có số đo; ngoại lệ so khớp chỉ là một dòng trong `parity/exceptions.toml`. Tôi vẫn ưu tiên đặt `search` cạnh `find`, `read` cạnh `note`; để user chọn, không chặn.
5. **Điều kiện xem lại code mode**: bản kia ghi sẵn "mở phase `run` bằng QuickJS, 4–6 ngày" nếu B4/C4/D5 vượt H4. Tôi đề nghị ghi điều kiện nhưng **không ghim giải pháp**: ba câu đó thiếu phép tập hợp cụ thể (bậc của note, giao hai `reach`, lọc `created <`), mỗi thứ là một tham số hoặc tool nhỏ rẻ hơn một trình thông dịch nhúng trên 4 đích với CRT tĩnh, và vẫn hợp Q4.
6. **`title` và `snippet` trong `related`**: bản kia giữ kết quả cũ. Không có `title`, agent phải gọi `note` từng dòng để viết link mở note; không có `snippet` ở hạt giống, agent chọn 5 note để đọc theo điểm mù. Giữ.

### Bộ tool gộp sẽ giao (13 tool)

| Tool | Tham số (đậm = bắt buộc) | Trường kết quả |
|---|---|---|
| `find`, `note`, `neighbors`, `reach`, `paths`, `loops`, `provenance`, `select`, `valid`, `more` | giữ nguyên Python | giữ nguyên |
| `search` | query, queries, path, mode, scope, limit (10), snippet_length (200), system (khi có trang ẩn) | `count`, `mode`, `items[{id, title, type, score, snippet}]`; lỗi `ohs_not_ready` (5 khoá, miễn `compact`), `ohs_error`+`detail`, `ambiguous path`+`did_you_mean` |
| `read` (tại chỗ) | **paths** (chuỗi hoặc mảng, ≤ 20), max_chars (4000, 0 = trọn), offset | `items[{id, title, type, url, chars, truncated, body} \| {id, error, did_you_mean}]`; không bao giờ `ohs_not_ready` |
| `related` | query, queries, path, hits, k (15), types, seeds (10), bộ lọc cạnh cũ | như cũ, thêm `title` mọi dòng, `snippet` ở hạt giống gieo bằng query/path; `seed_source` = `hits` \| `query` \| `path`; ưu tiên hits > path > query |

Ánh xạ sang stdio ở một hàm trong `pkm-core::ohs`: `snippet_length→snippetLength`, `path→query` và `options.notePath`, `queries` gửi khi ≥ 2 câu với `query` đứng đầu. Thứ tự danh sách, `tag`, và điều kiện xem lại code mode là ba điểm còn để user chốt.

Status: DONE
Summary: Đã đối chiếu hai bản; xác nhận lỗi `compact()` làm rơi `progress`/`eta_s`; nhận 7 điểm của bản kia, giữ 6 điểm có bằng chứng, chốt bộ 13 tool gộp.
Concerns/Blockers: Số token vẫn là ước lượng (nền lấy từ golden của bản kia); ba điểm thứ tự tool, `tag`, điều kiện code mode cần user chốt.
