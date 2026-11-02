# Báo cáo: `pkm-core::index`, `adapter::e11`, `validate`

- **Phase:** [phase-09-index-adapter-validate.md](../261102-0535-rust-core-port-master/phase-09-index-adapter-validate.md)
- **Trạng thái:** xong (`status: done`). Còn một mục nghiệm thu chờ user: chạy bộ so khớp trên bản B.

## 1. Kết quả

Tôi chạy bước so khớp như CI (`cargo run -q --locked -p pkm-parity -- check --vault <kg-solo>`). Cả 20 mục của tầng index đều báo **khớp**, không có lệch chưa có tên, mã thoát 0. Trước phase này, ba mục `edges`, `meta`, `v3_deviations` còn báo "chưa có". Các tầng `answers`, `tools`, `mcp`, `cli` vẫn "chưa có" vì thuộc phase sau.

| Bảng | Dòng | Kết quả |
|---|---|---|
| `edges` | 726 | khớp |
| `meta` | 16 | khớp |
| `v3_deviations` | 14 | khớp |
| 17 bảng của phase 08 (nay đọc từ DB SQLite) | như trước | khớp |

Ngoài golden, tôi so trực tiếp DB của Rust với DB của Python ở sha ghim, chặt hơn bộ so khớp. Phép so gồm `sqlite_master.sql`, mọi bảng theo rowid và `meta` (công cụ trong scratchpad, không commit). Kết quả giống hết ở mọi bước sau:

- build đủ;
- build không đổi gì;
- build tăng dần sau khi sửa vault (đổi status, xoá note, thêm note có alias trùng, đổi tên note);
- build tăng dần lần hai;
- build đủ lại.

Các bước này chạy trên các vault và cách đọc sau:

| Vault | Cách đọc |
|---|---|
| kg-solo | mặc định |
| kg-solo | `--no-aliases --edge-keys links --body-edges` |
| kg-solo | không gói, `--body-edges` |
| vault cài lỗi của `test_kg.py` | mặc định |
| vault e11 của `test_kg_adapter.py` | `--adapter e11 --packs capture --no-aliases --edge-keys links --body-edges`; khi sửa vault thì đổi file cặp, sổ có `\r\n`, dòng YAML lỗi, `rules` là chuỗi, đề xuất mới, trang chủ đề mới, thư mục lồng sâu, đổi status đề xuất |

Câu SQL của cả 19 view (18 view trong `VIEWS` và `v3_deviations`) giống Python từng byte. `edges.id` và rowid của `links` cũng trùng với Python, kể cả sau build tăng dần.

Chỉ hai bảng `schema_field` và `schema_link` có thứ tự rowid khác. Lý do là Python chèn các dòng này theo thứ tự của `set()`, thứ tự đó đổi theo `PYTHONHASHSEED`. Nội dung hai bảng vẫn giống, và bộ so khớp so bảng như tập có lặp nên không tính đây là lệch.

Tôi cũng chạy `parity.py run --private` trên vault e11 đã sửa, có `--adapter e11`: cả 20 mục index đều khớp.

Đầu ra của lệnh `validate`, `validate --json`, `validate --rule R8 --limit 2 --json` dựng từ `validate::Report` giống từng byte với `cli/*.out` của golden. Phase CLI chỉ còn việc nối cờ vào.

**Hiệu năng** trên kg-solo (746 note), bản release, đo trong cùng tiến trình, lấy trung vị:

| | Python | Rust |
|---|---|---|
| Build đủ | 58,6 ms | 21,2 ms |
| Build tăng dần không đổi gì | 2,4 ms | 2,1 ms |

Bộ so khớp chạy bản debug (`cargo run`), nên số nó in (khoảng 110 ms và 7 ms) chỉ để tham khảo.

## 2. Đã làm

- **`index`** (`crates/pkm-core/src/index.rs`, `index/py.rs`):
  - DDL chép nguyên.
  - `connect` có hàm SQL `regexp` theo nghĩa `re` của Python: `$` khớp cả trước `\n` cuối, `\Z` đổi thành `\z`, `[`, `&`, `~` trong lớp ký tự là chữ thường. Mẫu đã dịch được đệm lại.
  - `build` đủ và tăng dần theo mtime và cỡ file, như Python; Python không dùng hash. Thứ tự ghi và các câu đọc lại DB giống Python.
  - `derive` tính hai lớp cạnh, cửa sổ hiệu lực, nguồn gốc, cạnh claim, rồi bỏ cạnh trùng.
  - `Ctx` (`add`, `drop_where`) cho adapter.
  - `Options` (`key`, `pick`) và `previous`: lần build sau nhớ gói, adapter và cách đọc của lần trước.
  - `BuildInfo` giữ các khoá `meta` theo đúng thứ tự Python; `summary` cho ra hai dòng `build` in.
  - `dump_tables` đọc mọi bảng ra cho bộ so khớp.
  - `json.dumps` với `ensure_ascii` cho `meta.edges`, `float()` của Python.
  - Lỗi trả về `IndexError`, không panic.
- **`validate`** (`validate.rs`):
  - `VIEWS` ghép từ đúng các mảnh của Python (`_ANC`, `_KNOWN`, `_OK_SRC`, `_OK_DST`).
  - Các hàm `create_views`, `rules`, `counts`, `deviations` (có `rule` và `limit` như `if limit:`).
  - `report` cho lệnh `validate`, với `to_text` và `to_json`.
- **`adapter`** (`adapter.rs`, `adapter/e11.rs`):
  - Trait `Adapter` gồm `name`, `spec`, `hidden`, `instructions`, `digest`, `edges`, và hàm `load`.
  - Impl `E11` chép theo `kg_adapter.py`: cạnh `topic` theo thư mục, đề xuất, cặp OHS, nguồn gốc từ sổ và manifest, bỏ link trong thân note đề xuất, `SYSTEM`, `is_system`, `E11::new(similar)`.
  - Sổ, manifest và file cặp được đệm theo (mtime, cỡ), như `_cached`.
  - Phần e11 nằm trong file riêng, nên commit chỉ đổi adapter có thể chuyển độc lập.
- **Phụ thuộc** (`crates/pkm-core/Cargo.toml`):
  - `rusqlite` 0.41 (`bundled`, `functions`) chỉ thêm ở `pkm-core`; bộ so khớp đọc DB qua `pkm_core::index`.
  - `chrono` (`clock`) để lấy giờ máy cho `meta.built_at` (`datetime.now().isoformat`) trên ba nền.
- **Bộ so khớp** (chỉ thêm):
  - `parity/src/rust_side/index_tables.rs` build đủ rồi build tăng dần vào DB tạm, dump mọi bảng và chuẩn hoá như `dump-python.py`. Dump ghi `nodes.mtime` thành độ lệch, thay giá trị đổi theo lần chạy trong `meta` bằng `<volatile>`, thay đường dẫn bằng `<VAULT>` và `<DB>`.
  - `RustSide::table` lấy dòng từ DB. Với 17 bảng của phase 08, nó còn đối chiếu với bảng dựng thẳng từ vault (`note_tables.rs`, không sửa) và báo lỗi nếu hai bên khác nhau.
  - `perf` trả `build_full_ms` và `build_noop_ms`.
  - Sửa test `rust_side_reports_every_missing_layer_as_not_available_never_as_match` theo hiện trạng: mọi bảng index đã có, perf có hai số đo build.
- **Test:**
  - 13 test `index` và 2 test `index::py`, 10 test `validate`, 13 test `adapter`.
  - Chuyển từ `test_kg.py` và `test_kg_adapter.py`; bảng đối chiếu nằm trong `spec/PORTING.md`.
  - `planted_deviations_equal_python` kiểm đủ 33 chỗ lệch và số đếm đúng như Python trả.
  - `fixture_planted_errors` cần `PKM_KG_SOLO`. Tôi đã chạy với kg-solo và test qua.
- **`spec/PORTING.md`:**
  - Thêm hai mục "Lệch có chủ ý": adapter chỉ nạp theo tên ngắn, không nạp file `.py`; sổ e11 bỏ qua mọi dòng có YAML lỗi.
  - Thêm mục "Module `index`, `validate`, `adapter::e11`", gồm giới hạn đã biết và bảng test.
  - Không cần ngoại lệ nào mới trong `exceptions.toml`.

## 3. Kiểm tra

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` và `cargo test --workspace --locked` đều qua trên Linux.
- `cargo clippy` với `--target x86_64-pc-windows-msvc` và `--target aarch64-apple-darwin` sạch.
  - Máy không có trình biên dịch C chéo cho SQLite đóng gói, nên tôi dùng một `CC` và `AR` giả chỉ tạo file rỗng. Lệnh này chỉ kiểm mã Rust, không link.
  - Test trên Windows và macOS chạy ở CI, nơi runner có trình biên dịch C thật.

## 4. Cần biết cho phase sau

- **Phase `query`, `tools`:** mở DB bằng `index::connect(db, true)`; kết nối nào cũng có `regexp`.
  - `adapter::load(previous(db, "adapter_spec", ""))` trả adapter, `Adapter::hidden()` trả danh sách trang hệ thống.
  - Các test Python còn lại (lớp `Api`, `HiddenAndLayers`, `GraphIndexCli`, `OverMcp`, `Fixture.test_answer_key`) đã ghi "chưa chuyển" trong `spec/PORTING.md`.
- **Phase CLI, MCP:**
  - `Options::pick` và `previous(db, "packs" | "adapter_spec", "")` giữ cách đọc của lần build trước.
  - `BuildInfo::summary(db)` và `BuildInfo::entries()` dùng cho `build` và `build --json`.
  - `validate::report(...).to_text()` và `to_json()` dùng cho `validate`. JSON phải in với thụt lề 1 (`PrettyFormatter::with_indent(b" ")`) và xuống dòng cuối. Tôi đã kiểm cách in này cho ra giống golden.
- **API của `vault` cần nhưng chưa công khai.** Tôi không sửa `vault` mà viết bản cục bộ:
  - `vault::WIKI` (regex) chép vào `adapter/e11.rs`.
  - `pystr::is_space` chép vào `index/py.rs`.
  - `Resolver.by_name`: `fix-link` chỉ cần biết `to` có phải note của vault không, nên tra trong danh sách note thay cho `by_name`.
  - Nếu muốn gộp về một chỗ, có thể cho `vault` công khai `WIKI` và `is_space`.
- **Gợi ý (ngoài quyền sửa của tôi):**
  - Đặt `[profile.dev.package.libsqlite3-sys] opt-level = 3` trong `Cargo.toml` của workspace để số đo của bộ so khớp (bản debug) gần với bản release hơn.
  - Ghi lựa chọn `rusqlite` 0.41 và `chrono` vào `docs/DECISIONS.md`.
  - Vì `bundled`, CI mất thêm thời gian biên dịch SQLite.

## 5. Câu hỏi còn mở

1. Bản B chưa chạy (user hoãn). Lệnh chạy: `python3 parity/parity.py run --private <bản B> --adapter e11 --packs capture --no-aliases --edge-keys links --body-edges`. Cần xem đủ 20 mục index.
2. Python dừng cả lần build khi `data.score` của một đề xuất không đọc được thành số, hoặc file cặp, sổ, manifest hỏng JSON. Rust làm giống: `build` trả lỗi. Có muốn đổi sang chỉ báo lỗi cho riêng mục đó, như cách user đã chọn cho YAML không? Việc này cần user quyết.
3. Có mở issue ở repo đặc tả cho `_log_keys` (bắt cả `ValueError`) không? Việc này cần user cho phép.

## 6. Sửa theo review mã (`plans/reports/code-reviewer-261102-1500-index.md`)

Các lỗi sau đã sửa trong một commit riêng. Mọi chỗ hành vi khác Python khi build gặp lỗi đều ghi trong mục "Lệch có chủ ý" của `spec/PORTING.md`. Build thành công vẫn cho DB giống Python từng byte: tôi so lại kg-solo, vault cài lỗi và vault e11, mỗi vault qua 5 bước build như mục 1. Bộ so khớp kg-solo vẫn 0 lệch chưa có tên.

- **H1:** giao dịch ghi bảng note ghi thêm `meta.dirty`, chỉ xoá khi ghi `meta` cuối build. Lần build sau còn thấy cờ này thì tính lại cạnh. Test `failed_build_is_rederived_next_time` cố ý cho adapter lỗi để kiểm.
- **H2:** sửa bốn chỗ.
  - Đọc và parse note trước khi mở giao dịch.
  - Giao dịch ghi dùng `IMMEDIATE`; `busy_timeout` đặt rõ 30 s.
  - Thêm khoá file `<db>.lock` cạnh DB (`File::try_lock`, chờ tối đa 5 phút), nên mỗi lúc chỉ một build. Vì vậy `old` và `meta` đọc dưới khoá là nhất quán.
  - Test: `concurrent_builds_take_turns`.
- **M1:** chỉ chuyển sang WAL khi DB chưa ở WAL, và không chờ khoá khi chuyển. Không chuyển được thì build tiếp ở chế độ cũ (`wal_switch_failure_is_not_fatal`).
- **M2:** ghi giới hạn của cách nhận note đổi bằng (mtime, cỡ) và lối thoát `--full` vào PORTING.
- **M3:** file mất giữa lúc liệt kê và lúc đọc (`NotFound`) được coi như note đã xoá (`vanished_file_counts_as_removed`).
- **L1:**
  - `previous` không mở được DB ở chế độ chỉ đọc thì thử mở đọc ghi, không tạo DB mới.
  - Thêm `try_previous`, trả lỗi thay vì giá trị mặc định (API chỉ thêm, không đổi).
- **L2:** id note luôn ở dạng NFC và `data.to` được đưa về NFC trước khi so. Test `fix_link_target_in_nfd_matches_nfc_note_id` dùng tên file NFD.
