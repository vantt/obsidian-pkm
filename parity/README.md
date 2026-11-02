# Bộ so khớp (`parity/`)

Tiêu chuẩn "khớp" duy nhất giữa bản Rust và bản Python ở sha ghim ([docs/PRD.md](../docs/PRD.md) mục 6). Hai nửa:

- **Phía Python, chỉ chạy trên máy:** `parity.py` (điều phối) và `dump-python.py` chạy repo đặc tả trên một vault, ghi kết quả ra một **thư mục dump** trung lập ngôn ngữ.
- **Phần so, viết bằng Rust** (package `pkm-parity`, binary `parity`): so hai dump, hoặc chạy bản Rust rồi so với golden. CI chỉ dùng phần này, không cần Python, không cần token (Q5).

| File | Vai trò |
|---|---|
| `parity.py` | `golden` (dựng golden), `run` (Python so với Rust hoặc với chính nó, trên kg-solo hoặc `--private`) |
| `dump-python.py` | Chạy Python của repo đặc tả trên một vault, ghi dump |
| `src/` | `parity compare`, `parity check`, `parity rust-dump`, `parity fixture-args` |
| `src/rust_side.rs` | Phần cắm Rust: mỗi phase lõi nối tầng của mình vào đây |
| `golden/<sha>/` | Dump của Python trên kg-solo ở sha đó (CI so với thư mục của sha trong `spec/spec.lock`) |
| `exceptions.toml` | Ngoại lệ có tên |
| `report.json` | Báo cáo lần chạy gần nhất (không commit) |

Máy chạy phía Python cần Python 3.11+, PyYAML, git, node 20+ (npx) và quyền đọc repo đặc tả (`gh auth login`, `gh auth setup-git`).

## Chạy trên máy với kg-solo

```bash
python3 parity/parity.py run --against python   # Python ở sha ghim so với chính nó: phải ra 0 lệch
python3 parity/parity.py run                    # Python ở sha ghim so với bản Rust hiện tại
python3 parity/parity.py run --sha <sha>        # ở một sha bất kỳ của repo đặc tả (chuyển một commit, PRD 5.4)
```

Repo đặc tả clone một lần vào `~/.cache/pkm-parity/spec-<sha>/` (hoặc `--spec-dir DIR`, checkout có sẵn đã ở đúng sha). kg-solo sinh bằng `npx github:vantt/obsidian-test-fixture#<tag>` theo tag, preset, seed của `spec/spec.lock` (hoặc `--otf DIR`, checkout có sẵn của fixture). Dump nằm trong thư mục tạm, xoá sau khi so; `--keep DIR` (ngoài repo) để giữ lại mà xem.

## Chạy trên bản B (vault thật, chỉ trên máy user)

```bash
python3 parity/parity.py run --private ĐƯỜNG_DẪN_BẢN_B \
  --adapter e11 --packs capture --no-aliases --edge-keys links --body-edges [--against python]
```

`--private`: bảng tóm tắt và `report.json` chỉ có số đơn vị và số lệch theo từng bảng, op, tool, file MCP, lệnh CLI; không có tên note, giá trị, dòng hay tên vault. Lỗi của dump Python chỉ in loại lỗi. Index dựng vào thư mục tạm, không ghi gì vào vault; dump tạm xoá sau khi so. Không có câu đáp án (`.kg-answers.json`) nên tầng `answers` vắng; bộ câu tool tự sinh từ chính vault (mỗi type vài note, mỗi tool vài tham số, gồm `as_of`, `statuses`, `accepted_only`, `system`, `limit`, `more`). Bản B không bao giờ có golden trong repo.

## Dựng golden

```bash
python3 parity/parity.py golden                 # sha trong spec/spec.lock
python3 parity/parity.py golden --sha <sha>     # khi nâng ghim (phase 15): ghi parity/golden/<sha đủ>/
```

Lệnh dump Python hai lần (lần hai chạy lại đúng bộ câu của lần một), so hai dump, chỉ ghi golden khi 0 lệch và `fixture_check` của repo đặc tả đúng hết câu. Golden không có `perf.json` (số đo phụ thuộc máy). Dựng lại ở cùng sha cho golden giống từng byte. Nâng ghim: golden mới và `spec.lock` đi cùng một PR (PRD mục 5.5).

## CI

`.github/workflows/ci.yml` trên ba nền: `parity fixture-args` in tham số `npx` theo `spec/spec.lock`, `npx … --out $RUNNER_TEMP/kg-solo` dựng vault, rồi `parity check --vault $RUNNER_TEMP/kg-solo` chạy bản Rust và so với `golden/<sha ghim>/`. Vault phải nằm trong thư mục cùng tên với golden (`kg-solo`, tên vault có trong `instructions` và link `obsidian://`), tree hash của fixture phải khớp manifest golden. `related` chỉ có câu hỏi (không `hits`) cần OHS nên không có trong bộ câu; phần đó so trên máy có gói OHS.

Tầng MCP và CLI chạy chính binary `pkm` (`src/rust_side/binary.rs`), nên binary phải được dựng trước: CI dựa vào bước `cargo test --workspace` chạy trước `parity check`; chạy tay thì dựng `pkm` trước (hoặc đặt `PKM_BIN`), không thì hai tầng này báo "chưa có".

## Đọc kết quả

Mỗi mục (bảng, op, tool, file MCP, lệnh CLI) có một trong ba kết quả: **khớp**, **lệch** (kèm số lệch chưa có tên và có tên), **chưa có** (bản Rust chưa làm tầng đó; không bao giờ tính là khớp). Mỗi chỗ lệch chỉ đúng `file:dòng` trong dump mẫu, cùng cột hoặc JSON pointer và giá trị hai bên. Mã thoát: 0 khi không có lệch chưa có tên, 1 khi có, 2 khi lỗi (dump hỏng, sai vault). `--strict` coi "chưa có" là lỗi (dùng khi mọi tầng Rust đã xong). Hiệu năng (`build` đủ, `build` tăng dần không đổi gì, một vòng refresh MCP, từng tool) chỉ ghi số, không chặn.

## Định dạng dump (phiên bản 1)

| File | Nội dung |
|---|---|
| `manifest.json` | `implementation`, `spec_sha`, vault (`name`, fixture: preset, seed, `tree_hash`), cách đọc vault (`run`), bảng (cột, khoá chính, số dòng), `not_available` (`tầng:mục` hoặc `tầng:*`) |
| `tables/<bảng>.jsonl` | Mọi bảng của index trừ `sqlite_*`, và view `v3_deviations`; mỗi dòng một object theo thứ tự cột; sắp theo mọi cột. Dump sau khi build đủ rồi build tăng dần không đổi gì |
| `answers/answers.jsonl` | Mỗi câu graph và bảng của `.kg-answers.json`: `id` (`<case>#<i>`), `op`, `query`, `result` (như `fixture_check.run`) hoặc `raised` |
| `tools/calls.jsonl` | Bộ câu tool theo thứ tự chạy trên một phiên: `id`, `tool`, `args` (tên tham số MCP), `result` (như `Server.call`, trước khi MCP thu gọn) hoặc `raised` |
| `mcp/initialize.json`, `mcp/tools_list.json`, `mcp/instructions.txt` | Bắt tay MCP; `instructions` tách ra file văn bản để so theo dòng |
| `cli/commands.json`, `cli/<id>.out`, `cli/<id>.err` | Mỗi lệnh CLI (mục 10 PRD): `args` (không gồm VAULT và `--db`), mã thoát, `kind` (`json` hoặc `text`), stdout, stderr khi mã khác 0 |
| `perf.json` | Số đo (ms); không có trong golden |

Đường dẫn tuyệt đối của vault và DB trong mọi chuỗi thay bằng `<VAULT>`, `<DB>`. `nodes.mtime` ghi thành độ lệch (ms) so với mtime đọc từ đĩa lúc dump, vì vault dựng lại ở máy khác có mtime khác. Dòng JSONL của câu sắp khoá. Python ghi giá trị đổi theo lần chạy (thời điểm và thời gian dựng) thành `<volatile>` để golden không đổi khi dựng lại.

**Thứ tự liệt kê note được ghim** (`dump-python.py --listing sorted`, mặc định): Python chèn dòng theo thứ tự liệt kê file của hệ điều hành, nên `edges.id` (có trong JSON của `neighbors`, `note`, `provenance`) và kết quả `related` (thứ tự đẩy PageRank, đường `via`, điểm ở chữ số thứ năm) đổi theo máy. Đo trên kg-solo với `--listing reverse`: lệch ở `edges` (593 dòng), `neighbors`, `more`, `related`, CLI `neighbors-filter`; mọi phần khác không đổi. Ghim theo đường dẫn đã sắp (như NTFS, APFS trả) thì golden dựng lại được ở mọi máy và bản Rust có một thứ tự xác định để theo.

## Chuẩn hoá và dung sai (`src/normalize.rs`)

- `meta`: giá trị của `built_at`, `ms` bỏ qua. CLI: `built_at`, `ms` (của `build --json`, `stats`) bỏ qua; `; ms {…}` trong văn bản `build` bỏ qua.
- `nodes.mtime` lệch tối đa 1 ms. Số thực lệch tối đa 1e-6; số nguyên và số thực là hai kiểu khác nhau.
- Dòng bảng so như tập có lặp; dòng lệch ghép theo khoá chính (hoặc theo số cột giống) để báo đúng dòng.
- Câu đáp án: danh sách kết quả không có thứ tự, như `norm` của `fixture_check.py`.
- Tool: `aliases`, `blocks` không có thứ tự; `related`: `seeds` và `related` sắp theo điểm giảm dần rồi id trước khi so (so tập note, thứ hạng, điểm).
- CLI `stats`: `edges_by_origin`, `edges_by_rel` không có thứ tự.
- MCP: so chuỗi.

## Ngoại lệ có tên

Chỗ Rust cố ý làm khác Python (PRD mục 4): một mục trong `exceptions.toml` (tầng, mục, trường bị tha) và một mục cùng tên trong phần "Lệch có chủ ý" của `spec/PORTING.md`; thiếu mục ở `PORTING.md` thì `parity` từ chối chạy. Lệch được tha vẫn đếm ở cột "Có tên"; ngoại lệ không còn dùng tới được in ra để xoá.

## Nối tầng Rust (phase 08–12)

Mỗi phương thức của `RustSide` trong `src/rust_side.rs` (`table`, `answer`, `tool`, `mcp`, `cli`, `perf`) đang trả `None`, nên mọi mục báo "chưa có". Phase làm xong một tầng thì cho phương thức tương ứng trả `Some(...)` bằng cách gọi `pkm-core` (hoặc binary `pkm` cho CLI), giữ đúng định dạng trên; không đổi định dạng dump hay golden. Bộ câu luôn lấy từ dump mẫu, nên Rust chạy lại đúng các câu Python đã chạy.
