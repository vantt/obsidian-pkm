# PRD: lõi Rust của sản phẩm PKM (`vantt/obsidian-pkm`)

| | |
|---|---|
| Repo này | `vantt/obsidian-pkm` (private): lõi Rust, binary `pkm` |
| Đặc tả chạy được | [`vantt/obsidian-pkm-automation`](https://github.com/vantt/obsidian-pkm-automation) (repo nghiên cứu, Python), **ghim tại `5a3eb2f`** (2026-11-02) |
| Dữ liệu kiểm | [`vantt/obsidian-test-fixture`](https://github.com/vantt/obsidian-test-fixture) `v0.7.0`, preset `kg-solo` (có `.kg-answers.json`); vault xưởng bản B của user (chỉ chạy trên máy user) |
| Schema | v3 cùng gói `business`, `capture` (`docs/knowledge-graph/schema/schema.yaml` của repo đặc tả) |
| Bộ tool MCP | Bộ đã duyệt ở E9.12 (GATES 2026-10-30) cộng `related` (E9.4) |
| Tìm theo nghĩa | OHS qua fork [`vantt/obsidian-hybrid-search`](https://github.com/vantt/obsidian-hybrid-search) (agent khác giữ; PRD ở `vantt/PRD.md` của fork) |
| Người viết code | Agent. Tài liệu này đủ để làm mà không hỏi lại; chỗ còn phải quyết ghi ở mục 15 |

Mục lục: 1 Bối cảnh và quyết định đã chốt · 2 Phạm vi · 3 Kiến trúc · 4 Bản ghim và đặc tả · 5 Đuổi theo bản Python · 6 Bộ so khớp · 7 Chuyển từng module · 8 Hợp đồng MCP · 9 Nối OHS · 10 CLI · 11 Cài cho Claude Desktop · 12 Kiểm thử và CI · 13 Điều kiện bắt đầu và chuyển hẳn · 14 Lộ trình · 15 Quyết định còn mở · 16 Rủi ro.

---

## 1. Bối cảnh và quyết định đã chốt

Sản phẩm: hệ PKM trên Obsidian cho người làm một mình, freelancer, có tiếng Việt. Repo đặc tả (Python) đã chứng minh được lõi graph: index SQLite của vault, schema v3, bộ tool hỏi graph, MCP cho Claude Desktop và chat trong Obsidian. Khách không được phải cài Python, nên lõi phải thành một binary.

Đã chốt (nguồn: `docs/GATES.md`, `docs/plans/E12.7-port-rust-js.md` của repo đặc tả):

| # | Quyết định | Nguồn |
|---|---|---|
| D1 | Lõi viết bằng **Rust**: một crate thư viện `pkm-core`, một binary `pkm` (CLI, MCP, sau này bộ cài và web server). Không chuyển lõi sang JS | GATES 2026-10-31; E12.7 mục 1, 5.1 (làm rõ 2026-11-02) |
| D2 | Bản Python là **đặc tả chạy được**. Rust phải cho kết quả giống Python trên cùng dữ liệu; bộ test Python là danh sách kiểm | GATES 2026-10-31; E12.7 nguyên tắc 4 |
| D3 | **SQL giữ nguyên**: bảng, view validator, recursive CTE chép sang `rusqlite` (SQLite đóng gói sẵn) | E12.7 nguyên tắc 5 |
| D4 | Giao diện người dùng là **plugin Obsidian JS/TS**, gọi `pkm` qua `child_process`, JSON. Không thuộc giai đoạn này | E12.7 mục 1 |
| D5 | **Tìm theo nghĩa giữ OHS** (JS, Node đi kèm gói dựng sẵn của fork). Không tự làm lại trong Rust ở giai đoạn này | user 2026-11-01; E12.7 mục 5 |
| D6 | `pkm-core` có **một module OHS duy nhất**: giữ một tiến trình `ohs serve --stdio`, request có `id`. Mọi chỗ khác gọi qua module này | E12.7 mục 5.1 |
| D7 | Khi OHS chưa sẵn sàng: **không có đường lùi** cho ra kết quả kém; báo trạng thái kèm tiến độ hoặc lỗi kèm việc cần làm; tool MCP trả lỗi có cấu trúc | user 2026-11-02; E12.7 mục 5.1 |
| D8 | Điện thoại: `pkm serve` trên máy giữ vault, điện thoại chỉ là trình duyệt (Tailscale, LAN, token). Không WASM. Không thuộc giai đoạn này | user 2026-11-02; E12.7 mục 4.1 |
| D9 | **Chỉ chuyển lõi `kg` và MCP trước.** Các việc E11 (trang Nguyên liệu, hàng chờ đề xuất, vùng tri thức, giao diện xưởng) chờ E11.7 (4 tuần sản xuất thật) và chờ nhịp sửa chậm lại | trao đổi 2026-11-02; số đo: `scripts/e11` 21 commit, khoảng 5.100 dòng đổi trong 7 ngày; `scripts/kg` 8 commit, khoảng 2.300 dòng, phần lớn là sửa cho Windows |
| D10 | Ghim một phiên bản Python, chuyển, rồi **đuổi theo** thay đổi mới của Python tới khi hai bên khớp, rồi bỏ bản Python cho lõi | user 2026-11-02 |

## 2. Phạm vi

### 2.1. Giai đoạn 1 (repo này làm)
Lõi graph và mọi thứ cần để thay MCP `kg` Python trong Claude Desktop và chat trong Obsidian:

| Python (repo đặc tả, tại `5a3eb2f`) | Dòng | Rust |
|---|---|---|
| `scripts/kg/vault.py` | 162 | `pkm-core::vault` |
| `scripts/kg/schema.py` + `docs/knowledge-graph/schema/schema.yaml` | 167 + 372 | `pkm-core::schema` (schema nhúng bằng `include_str!`, vẫn nhận file ngoài) |
| `scripts/kg/index.py` | 315 | `pkm-core::index` |
| `scripts/kg/validate.py` | 144 | `pkm-core::validate` (SQL view chép nguyên) |
| `scripts/kg/query.py` | 370 | `pkm-core::query` (recursive CTE chép nguyên) |
| `scripts/kg/tools.py` | 104 | `pkm-core::tools` |
| `scripts/kg/related.py` | 180 | `pkm-core::related` (PageRank cá nhân hoá) |
| `scripts/kg/mcp_server.py` | 390 | `pkm::mcp` |
| `scripts/kg/cli.py`, `__main__.py` | 128 | `pkm::cli` (`clap`) |
| `scripts/e11/kg_adapter.py` | 193 | `pkm-core::adapter::e11` (một impl của trait `Adapter`) |
| `scripts/e11/graph_index.py` (phần còn người gọi) | ~100 trong 409 | gộp vào `pkm` CLI; phần còn lại bỏ |
| `scripts/platform_paths.py` (phần không phải WSL) | ~60 trong 163 | `pkm-core::platform`: thư mục cache, `move` không đè, trùng hoa thường và NFC/NFD, thử lại khi file bị giữ |
| `scripts/kg/desktop_config.py` | 167 | `pkm mcp install` (mục 11) |
| Module OHS (mới) | — | `pkm-core::ohs` (mục 9) |

Test của repo đặc tả là danh sách kiểm: `scripts/kg/tests/test_kg.py` (15 test), `test_mcp.py` (13), `fixture_check.py` (các câu có đáp án của kg-solo: 70 phép graph cộng các phép bảng), `scripts/tests/test_platform_paths.py` (19, phần không phải WSL).

### 2.2. Ngoài phạm vi giai đoạn 1
- Việc E11: `material.py`, `propose.py`, `zones.py`, `workshop_ui.py`, `setup_vault.py`, `highlight_colours.py`, `normalize.py`, `migrate_v3.py`, `selection_signals.py`. Ở lại Python tới khi D9 cho phép.
- Plugin Obsidian (JS/TS), `pkm serve` cho điện thoại, bộ cài có giao diện cho khách, ký mã: các giai đoạn sau (E12.7 giai đoạn 2–6).
- Mọi thay đổi trong fork OHS.
- Phần chỉ dành cho WSL của `platform_paths.py` (liệt kê ổ Windows bằng PowerShell, dịch đường dẫn `/mnt/X`): binary chạy native trên từng nền, không cần.

---

## 3. Kiến trúc

```
vantt/obsidian-pkm
├── Cargo.toml                 workspace
├── crates/
│   ├── pkm-core/              thư viện: vault, schema, index, validate, query, tools, related, adapter, ohs, platform
│   └── pkm/                   binary: cli, mcp (stdio), sau này install, serve
├── spec/
│   ├── spec.lock              sha đang ghim của repo đặc tả, phiên bản fixture (mục 4)
│   └── PORTING.md             nhật ký: commit Python nào đã chuyển, ở commit Rust nào
├── parity/                    bộ so khớp (mục 6): script chạy hai bản, so bảng và JSON
├── docs/PRD.md                tài liệu này
└── .github/workflows/         CI (mục 12)
```

Crate gợi ý (chốt khi làm, ghi lý do trong `docs/DECISIONS.md`): `rusqlite` (feature `bundled`), `serde`, `serde_json`, crate YAML (chọn theo bài thử S1, mục 14), `regex` (và `fancy-regex` chỉ nơi cần lookaround), `unicode-normalization`, `clap`, `walkdir` hoặc `std::fs::read_dir`, `dirs`, `sha2`. MCP: crate `rmcp` hoặc JSON-RPC stdio tự viết (giao thức nhỏ; chốt theo bài thử S2). Không dùng async runtime nếu không cần; luồng refresh nền dùng `std::thread`.

### 3.1. MCP mới nói chuyện với OHS thế nào (trả lời câu hỏi 2026-11-02)

Hiện nay (Python): agent (Claude) có **hai** MCP. `ohs` để tìm theo chữ, theo nghĩa và đọc thân note; `kg` để đi graph. Agent tự phối hợp: gọi `ohs` `search`, rồi đưa kết quả làm `hits` cho `kg` `related`. `kg` chỉ tự gọi OHS (qua CLI, mỗi lần một tiến trình) khi `related` chỉ có câu hỏi, không có `hits`.

Ba cách cho bản Rust:

| Cách | Agent thấy | `pkm` gọi OHS | Được | Mất |
|---|---|---|---|---|
| A. Một cửa: agent chỉ nói với `pkm` | Một MCP `pkm`: các tool graph, cộng `search`, `read` chuyển tiếp sang OHS, và `related` nhận thẳng câu hỏi | Có, qua module OHS (một tiến trình stdio sống lâu) | Một server để cài; một lần nạp model; một bộ đường dẫn (hết lỗi lệch NFC/NFD giữa hai server); `related(query)` gộp tìm và đi graph trong **một** lần gọi, ít lượt gọi tool và ít token hơn; trạng thái OHS báo một chỗ | Phải giữ `search`, `read` của `pkm` khớp tuỳ chọn của OHS khi OHS đổi |
| B. Hai cửa như hiện nay | Hai MCP `pkm` và `ohs` | Chỉ khi `related` không có `hits` | Không phải chuyển tiếp gì; tool `ohs` mới của upstream có ngay | Hai server, hai lần nạp model (MCP của OHS và tiến trình con của `pkm`); agent phải gọi `search` rồi `related` (thêm lượt, thêm token); hai chỗ báo lỗi |
| C. `pkm` không gọi OHS | Hai MCP | Không | Đơn giản nhất | `related` không chạy được khi chỉ có câu hỏi; plugin, `material` (sau này), điện thoại đều phải tự nói với OHS: trái D6 |

**Đề xuất: A cho sản phẩm, đi qua B trong lúc chuyển.**
1. **Giai đoạn so khớp (khớp với Python):** giữ đúng hợp đồng của MCP `kg` Python (mục 8), tức cách B. `pkm` thay `kg`; MCP `ohs` vẫn được cài như hiện nay. Module OHS của `pkm` chỉ dùng cho `related` khi không có `hits` (như Python, nhưng một tiến trình sống lâu thay vì mỗi lần một tiến trình).
2. **Sau khi chuyển hẳn:** thêm `search`, `read` (chuyển tiếp mỏng sang OHS qua stdio, giữ tên tham số của OHS) và cho `related` nhận câu hỏi làm đường chính. Bộ cài thôi đăng ký MCP `ohs` cho khách. Người dùng nâng cao vẫn tự thêm MCP `ohs` được; `pkm` không cấm.
3. Ghi chú: `related` của A tiết kiệm token đúng như E11.20 (Bản đồ bài) cần: mỗi ý một câu tìm, một lần gọi.

Chờ user chốt (Q1, mục 15). Cho tới khi chốt, làm theo bước 1 (không đổi gì với Python).

---

## 4. Bản ghim và đặc tả

- `spec/spec.lock`:
  ```toml
  [spec]
  repo = "https://github.com/vantt/obsidian-pkm-automation"
  sha = "5a3eb2f7990cd093b2ee3a1ccd5b46bd46e3ca4e"
  date = "2026-11-02"
  [fixture]
  repo = "https://github.com/vantt/obsidian-test-fixture"
  tag = "v0.7.0"
  preset = "kg-solo"
  seed = "<seed dùng trong scripts/kg/tests của repo đặc tả>"
  ```
- Đọc đặc tả theo thứ tự: mã Python ở sha đó → test Python → `docs/knowledge-graph/install-guide-kg-mcp.md` (cách dùng MCP) → `docs/capture-to-producer/playbook/04-graph-kg.md` (núm vặn) → GATES các mục 2026-10-29 tới 2026-11-02.
- **Hành vi là đặc tả, không phải câu chữ của mã.** Chỗ nào Python có lỗi rõ ràng: không sửa lặng lẽ trong Rust. Ghi vào `spec/PORTING.md` mục "Lệch có chủ ý", mở issue ở repo đặc tả, và bộ so khớp ghi ngoại lệ có tên.
- Không đọc mã Python để dịch từng dòng khi Rust có cách tự nhiên hơn; chỉ cần kết quả như nhau (mục 6).

## 5. Đuổi theo bản Python

Python vẫn sửa trong lúc chuyển. Cách đuổi, ngược chiều với cách fork OHS đuổi upstream:

1. **Đường dẫn theo dõi** (thay đổi ở đây là việc của repo này):
   `scripts/kg/**`, `scripts/e11/kg_adapter.py`, `scripts/e11/graph_index.py`, `scripts/platform_paths.py`, `docs/knowledge-graph/schema/schema.yaml`, `docs/knowledge-graph/install-guide-kg-mcp.md` (hợp đồng dùng), `scripts/kg/tests/**`.
2. **Lệnh** `cargo xtask follow` (hoặc `parity/follow.py`): fetch repo đặc tả, liệt kê commit từ `spec.lock` tới `main` chạm đường dẫn theo dõi, in mỗi commit một dòng (sha, ngày, tiêu đề, file chạm, số dòng). `--issues` mở mỗi commit một issue nhãn `port` trong repo này (bỏ commit đã có issue).
3. **Workflow** `follow.yml`: chạy hằng ngày; có commit mới thì mở issue như trên. Repo đặc tả là **private**: workflow cần secret `SPEC_READ_TOKEN` (fine-grained PAT, chỉ quyền đọc `contents` của `vantt/obsidian-pkm-automation`). CI so khớp (mục 12) dùng cùng secret để checkout repo đặc tả ở sha ghim.
4. **Chuyển một commit:** đọc diff và test đi kèm → sửa Rust → chạy bộ so khớp với đặc tả **ở sha của commit đó** → ghi `spec/PORTING.md` (sha Python, commit Rust, lệch có chủ ý nếu có) → đóng issue. Chuyển theo thứ tự thời gian; commit chỉ đổi phần WSL hoặc phần Python thuần (ví dụ `utf8_stdio`) thì đánh dấu "không áp dụng" kèm lý do.
5. **Nâng ghim:** khi mọi commit tới sha X đã chuyển và bộ so khớp ở X sạch, đặt `spec.lock` = X trong cùng PR.
6. **Đo khoảng cách:** `follow` in số commit chưa chuyển và số ngày từ commit cũ nhất chưa chuyển. Mục tiêu: dưới 5 commit, dưới 7 ngày.
7. **Quy tắc hội tụ** (bắt buộc để có ngày gặp nhau): khi một module đã khớp và đã chuyển hẳn (mục 13), tính năng mới của module đó viết **ở Rust trước** (hoặc cả hai bên trong cùng ngày, Python chỉ để giữ chạy được). Ghi quy tắc này vào GATES của repo đặc tả lúc chuyển hẳn.

## 6. Bộ so khớp (`parity/`)

Chạy cả hai bản trên cùng vault, so kết quả. Đây là tiêu chuẩn "khớp" duy nhất.

| Tầng | So gì | Cách so |
|---|---|---|
| Index | Mọi bảng dữ liệu: `nodes`, `props`, `links`, `raw_links`, `blocks`, `names`, `edges`, `meta`; mọi bảng `schema_*`; view `v3_deviations` | Dump mỗi bảng thành dòng chuẩn hoá (sắp theo khoá, bỏ cột thời gian dựng), so từng dòng. `mtime` cho lệch 1 ms (E12.4: PowerShell và `stat` làm tròn khác nhau) |
| Tool | JSON của `find`, `note`, `neighbors`, `reach`, `paths`, `loops`, `provenance`, `select`, `valid`, `more`, `related` | Bộ câu: (1) mọi câu trong `.kg-answers.json` của kg-solo (như `fixture_check.py`); (2) bộ câu sinh tự động trên bản B (mỗi loại note vài note, mỗi tool vài tham số, gồm `as_of`, `statuses`, `accepted_only`, `system`, `limit`); so JSON sau khi chuẩn hoá thứ tự danh sách không có thứ tự. `related`: so tập note và thứ hạng, điểm lệch tối đa 1e-6 |
| MCP | Bắt tay (`initialize`, `tools/list`): tên, mô tả, schema tham số của tool; khối `instructions` | So chuỗi; lệch có chủ ý ghi tên |
| CLI | Đầu ra của mỗi lệnh ở mục 10 trên kg-solo | So JSON hoặc văn bản |
| Hiệu năng | Thời gian `build` đủ, `build` tăng dần không đổi gì, một vòng refresh MCP, mỗi tool | Rust không chậm hơn Python; ghi số |

- kg-solo chạy trong CI. Bản B chỉ chạy trên máy user (`parity --private <vault>`), chỉ in số dòng lệch theo bảng và theo tool, không in nội dung note.
- Đầu ra: `parity/report.json` và bảng tóm tắt; mã thoát khác 0 khi có lệch chưa được ghi là "có chủ ý".

## 7. Chuyển từng module: điểm cần chú ý

| Module | Chú ý | Rủi ro |
|---|---|---|
| `vault` | Tách frontmatter, link, block; giải link kiểu Obsidian (alias khi bật); giữ CRLF hay LF; NFC cho đường dẫn trong DB; bỏ trang theo luật adapter. `vault.py` có regex lookaround: viết lại hoặc dùng `fancy-regex` cục bộ | **Cao: YAML.** PyYAML theo YAML 1.1 (`yes`/`no` thành bool, ngày tự đổi kiểu), crate Rust thường theo 1.2. Đọc khác là ra cạnh khác. Bài thử S1 trước khi chọn crate; nếu cần, viết lớp chuẩn hoá kiểu giá trị theo đúng cách Python đối xử |
| `schema` | Nạp gói, `extends`, bảng `schema_*` | Thấp |
| `index` | Tăng dần theo mtime, hash; hai lớp cạnh; cửa sổ hiệu lực; nguồn gốc (`origin`, `proposed_by`, `extracted_by`); adapter thêm cạnh (folder, proposal, ohs) | Trung bình: thứ tự ghi và chuẩn hoá giá trị |
| `validate`, `query` | SQL chép nguyên; phần Rust chỉ ghép câu và lọc | Thấp |
| `tools` | JSON phải khớp từng khoá, kể cả tóm tắt kết quả dài (`count`, `by_type`, `handle`) và `more` | Trung bình |
| `related` | PageRank cá nhân hoá, trọng số theo lớp (`WEIGHT`), `alpha`, `eps`, `k` | Số thực: so thứ hạng và sai số |
| `mcp` | Refresh nền (mặc định 5 s), log JSONL, giới hạn ký tự, `instructions` ghép từ `--about`, schema, số đếm trong DB, adapter | Trung bình: giữ đúng mô tả tool (agent phụ thuộc vào câu chữ) |
| `adapter::e11` | Cạnh `topic` theo trang chủ đề (`toc`, tên `(chủ đề).md`), cạnh từ hàng chờ đề xuất, cạnh OHS từ file cặp gần nghĩa, danh sách trang hệ thống | Trung bình: `kg_adapter.py` còn đổi theo E11 |
| `platform` | Thư mục cache theo OS (`%LOCALAPPDATA%\pkm`, `~/Library/Application Support/pkm`, `~/.cache/pkm`); `move` không đè; trùng hoa thường và NFC/NFD; thử lại khi file bị giữ (OneDrive, Obsidian) | Thấp; mang theo 19 test |

## 8. Hợp đồng MCP (giai đoạn so khớp)

- Tên server, tên tool, mô tả tool, schema tham số, dạng kết quả: **giống Python** ở sha ghim. Tool: `find`, `note`, `neighbors`, `reach`, `paths`, `loops`, `provenance`, `select`, `valid`, `more`, `related`.
- Cờ khởi động giống Python: `--about`, `--packs`, `--adapter`, `--no-aliases`, `--edge-keys`, `--body-edges`, `--db`, `--refresh-every`, `--ohs-cli`, `--ohs-node`, `--obsidian-vault`, `--today`, `--max-chars`, `--log`, `--limit`, `--system`. Cờ OHS đổi nghĩa: trỏ tới gói OHS (thư mục có `node` và `dist`), không còn gọi CLI mỗi lần (mục 9).
- `instructions` ghép giống Python (mô tả vault, mô tả từng loại trong schema, số note và quan hệ, đoạn của adapter, khối cố định).
- Link `obsidian://open` trong kết quả như Python.
- Lỗi OHS chưa sẵn sàng (D7): tool cần OHS trả `{"error":"ohs_not_ready","state":"indexing|starting|model_missing|broken","progress":0.64,"eta_s":180,"action":"…"}`. Tool không cần OHS chạy bình thường.
- Sau khi chuyển hẳn (theo Q1): thêm `search`, `read`; mọi thay đổi hợp đồng ghi vào `docs/MCP-CHANGELOG.md`.

## 9. Nối OHS (`pkm-core::ohs`)

- Tìm gói OHS: cờ `--ohs` (thư mục gói dựng sẵn của fork: `node/`, `app/dist/`), biến `PKM_OHS_DIR`, hoặc thư mục mặc định của bộ cài. Không thấy thì mọi tool cần OHS trả `ohs_not_ready` với `state: broken`, `action` chỉ cách cài.
- Khởi động `node app/dist/src/cli.js serve --stdio` với `OBSIDIAN_VAULT_PATH` (và `OBSIDIAN_DB_PATH` khi fork có bản vá P02), chờ `{"ready":true}`; giữ sống; mọi request có `id`; nhiều request đồng thời; khởi động lại khi chết (giới hạn số lần, sau đó `broken`); tắt khi `pkm` thoát (đóng stdin, chờ có giới hạn).
- Trạng thái: `starting` (chưa `ready`), `indexing` (lấy tiến độ từ `status` của OHS), `model_missing`, `broken`, `ready`.
- Upstream đã có `search` qua stdio với đủ tuỳ chọn và request đồng thời theo `id`; `read`, `vectors`, `neighbors`, `reindex` cần bản vá P04 của fork. Giai đoạn so khớp chỉ cần `search` (cho `related` không có `hits`).
- Chuẩn hoá đường dẫn trả về từ OHS về NFC trước khi tra trong index (lỗi đã gặp: lệch NFC/NFD giữa OHS và ổ đĩa).
- Không bao giờ đọc thẳng file DB hay `node_modules` của OHS.

## 10. CLI

Giống `scripts/kg/cli.py` ở sha ghim, đổi `python3 scripts/kg VAULT` thành `pkm VAULT`:
`build [--full]`, `stats`, `validate [--rule] [--limit] [--json]`, `find`, `note`, `neighbors`, `reach`, `paths`, `loops`, `provenance`, `select`, `valid`, `mcp`. Tuỳ chọn chung: `--db`, `--refresh`, `--limit`, `--adapter`, `--system`, `--packs`, `--no-aliases`, `--edge-keys`, `--body-edges`. Cách đọc vault nhớ trong index sau lần build đầu, như Python.

Thêm: `pkm --version` (in phiên bản và sha đặc tả đã khớp), `pkm mcp install` (mục 11), `pkm doctor` (kiểm DB, gói OHS, cấu hình Claude Desktop, đường dẫn dài trên Windows).

## 11. Cài cho Claude Desktop (`pkm mcp install`)

Thay `desktop_config.py`: không còn chép 15 file Python, không cần Python.
- Tìm file cấu hình Claude Desktop (`%APPDATA%\Claude\claude_desktop_config.json`, bản Store, macOS `~/Library/Application Support/Claude/`).
- Dựng index lần đầu vào thư mục dữ liệu của `pkm` (ngoài vault).
- Ghi mục `kg` (giữ tên `kg` để Project instructions cũ vẫn đúng) với `command` là đường dẫn đủ của `pkm`, `args` là vault và cờ đọc của vault đó.
- `--check` (bắt tay lại như Claude Desktop sẽ làm), `--dry-run`, `--remove`. Từ chối ghi khi Claude Desktop đang chạy (như Python), báo rõ.

## 12. Kiểm thử và CI

- Test Rust theo đúng danh sách test Python (mục 2.1), cộng test đơn vị riêng cho phần Rust tự viết.
- CI (`.github/workflows/ci.yml`): `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` trên Windows, macOS, Linux; dựng kg-solo bằng `npx github:vantt/obsidian-test-fixture#v0.7.0 generate …`; chạy bộ so khớp với Python ở sha ghim (Python 3.12 và PyYAML trên runner).
- `follow.yml` (mục 5). `release.yml`: khi có tag `v*`, build `pkm` cho `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`, kèm sha256. Chưa ký mã (giai đoạn sau).
- Không đưa nội dung vault thật vào repo, CI, log hay issue.

## 13. Điều kiện bắt đầu và chuyển hẳn

**Bắt đầu viết Rust khi cả hai đúng** (ghi một mục GATES ở repo đặc tả lúc bắt đầu, vì đây là nới cổng "chờ E11.7" riêng cho lõi):
1. User đã thử xong E12.5 (MCP `kg` native trên Claude Desktop, hỏi được các câu mục 4.4 của hướng dẫn).
2. Một tuần không đổi `schema.yaml` và không đổi hợp đồng tool MCP ở repo đặc tả.

Việc chuẩn bị không phải chờ: bài thử S1–S3, khung workspace, bộ so khớp chạy với bản Python, `follow`.

**Chuyển hẳn (bỏ bản Python cho lõi) khi đủ:**
1. Bộ so khớp sạch trên kg-solo (CI) và bản B (máy user) ở sha ghim mới nhất.
2. `follow` báo 0 commit chưa chuyển.
3. Chạy song song một tuần: Claude Desktop và chat trong Obsidian trỏ vào `pkm`; user dùng bình thường và chạy lại nhóm A–C, F của playbook thử thách kg (`docs/capture-to-producer/playbook/07-kg-challenges.md` của repo đặc tả), điểm không thấp hơn khi dùng Python.
4. User đồng ý.

Sau khi chuyển hẳn: `setup_vault.py` và hướng dẫn cài ở repo đặc tả trỏ sang `pkm`; `scripts/kg` ở repo đặc tả đóng băng (chỉ sửa lỗi); áp quy tắc hội tụ (mục 5.7).

## 14. Lộ trình

| ID | Việc | Cần | Công (ngày) | Xong khi |
|---|---|---|---|---|
| R00 | Khung workspace, `spec.lock`, CI rỗng ba nền, `DECISIONS.md`, `PORTING.md` | — | 0,5 | CI xanh |
| R01 | Bài thử S1: crate YAML so với PyYAML trên toàn bộ frontmatter của kg-solo và bản B (bản B ở máy user, chỉ in số lệch) | R00 | 1 | Chọn crate, liệt kê mọi chỗ lệch và cách xử lý |
| R02 | Bài thử S2: MCP Rust nhỏ (một tool) bắt tay với Claude Desktop trên Windows và với Claude Code | R00 | 0,5 | Gọi được tool; chốt `rmcp` hay tự viết |
| R03 | Bài thử S3: `pkm` gọi `ohs serve --stdio` của upstream, request đồng thời theo `id`, chuẩn hoá NFC | R00 | 0,5 | Ra kết quả `search` đúng |
| R04 | Bộ so khớp chạy với bản Python (Python so với chính nó cho ra 0 lệch), bộ câu sinh tự động cho bản B | R00 | 2 | `parity` chạy được trên kg-solo và bản B |
| R05 | `follow` + `follow.yml` | R00 | 0,5 | Liệt kê đúng commit từ `5a3eb2f` |
| R06 | `vault` + `schema` | R01 | 2,5 | Bảng `nodes`, `props`, `links`, `raw_links`, `blocks`, `names`, `schema_*` khớp |
| R07 | `index` (cạnh, lớp, tăng dần) + `adapter::e11` + `validate` | R06 | 3 | Mọi bảng và `v3_deviations` khớp |
| R08 | `query` + `tools` + `more` | R07 | 2,5 | JSON mọi tool trừ `related` khớp |
| R09 | `related` + `ohs` module | R08, R03 | 1,5 | `related` khớp (có và không có `hits`); trạng thái OHS đúng D7 |
| R10 | `mcp` (refresh, log, instructions, giới hạn) + CLI | R08, R02 | 2 | Bắt tay khớp; test MCP Python chuyển sang đều qua |
| R11 | `platform` + `pkm mcp install` + `pkm doctor` | R10 | 1,5 | Cài trên Windows sạch, Claude Desktop gọi được tool |
| R12 | Đuổi theo các commit Python phát sinh trong lúc làm | R05 | 10–20% công trên | `follow` về 0 |
| R13 | Chạy song song một tuần trên máy user, chuyển hẳn | R11, R12 | 0,5 + 1 tuần dùng | Mục 13 đủ |
| | **Cộng** | | **khoảng 18–22 ngày** | |

Song song được: R01–R05 sau R00; R06 chờ R01.

## 15. Quyết định còn mở
| # | Câu hỏi | Đề xuất | Ai quyết |
|---|---|---|---|
| Q1 | MCP mới: một cửa (agent chỉ nói với `pkm`, `pkm` chuyển tiếp OHS) hay hai cửa (agent nói với `pkm` và `ohs`) | Hai cửa trong giai đoạn so khớp; một cửa sau khi chuyển hẳn (mục 3.1) | User |
| Q2 | Giữ tên MCP `kg` hay đổi thành `pkm` | Giữ `kg` tới khi chuyển hẳn (Project instructions, `CLAUDE.md` của vault, playbook đang gọi tên `kg`); đổi sau cùng đợt với Q1 nếu muốn | User |
| Q3 | Crate YAML | Theo kết quả R01 | Agent |
| Q4 | `rmcp` hay JSON-RPC tự viết | Theo kết quả R02 | Agent |
| Q5 | Repo đặc tả có chạy được Rust trong CI của nó không (để so khớp hai chiều) | Không cần; repo này chạy Python ở sha ghim | Agent |

## 16. Rủi ro
| Rủi ro | Ảnh hưởng | Giảm thiểu |
|---|---|---|
| YAML đọc khác PyYAML | Cạnh khác, tool khác | R01 trước khi viết `vault`; lớp chuẩn hoá giá trị; bộ so khớp trên bản B thật |
| Python sửa nhanh hơn Rust đuổi kịp | Không bao giờ chuyển hẳn | Điều kiện bắt đầu (mục 13); đo khoảng cách (mục 5.6); quy tắc hội tụ |
| `kg_adapter.py` còn đổi theo E11 | Đuổi theo tốn công | Tách adapter thành trait, phần e11 nhỏ; commit chỉ đổi adapter chuyển riêng |
| Mô tả tool MCP lệch câu chữ | Agent chọn tool khác, trả lời khác | So chuỗi trong bộ so khớp tầng MCP |
| OHS đổi giao thức stdio | `related` hỏng | Module OHS duy nhất; probe trong fork OHS; test với bản OHS ghim |
| Đường dẫn Windows (dài, NFD, hoa thường), antivirus chặn binary chưa ký | Không chạy trên máy khách | Mang theo test `platform`; `pkm doctor`; ký mã ở giai đoạn đóng gói |
