# Báo cáo: `pkm-core::vault` và `pkm-core::schema`

- **Phase:** [phase-08-vault-and-schema.md](../261102-0535-rust-core-port-master/phase-08-vault-and-schema.md)
- **Trạng thái:** xong (`status: done`). Còn chờ một mục nghiệm thu: chạy bộ so khớp trên bản B ở máy user (user hoãn, agent khác chạy sau).

## 1. Kết quả

Bộ so khớp kg-solo so với golden `5a3eb2f`, chạy đúng hai bước CI trên máy (không có Python trong `PATH`, không token, vault dựng bằng `npx` theo `spec.lock`): 17 bảng của phase này báo **khớp**, 0 lệch chưa có tên, mã thoát 0.

| Bảng | Dòng | Kết quả |
|---|---|---|
| `nodes`, `props`, `raw_links`, `links`, `blocks`, `names` | 746, 2566, 1354, 1354, 128, 780 | khớp |
| 11 bảng `schema_*` | 7 tới 50 dòng mỗi bảng (`schema_problems` rỗng) | khớp |

kg-solo không có dạng YAML rủi ro nào, nên tôi đối chiếu thêm với Python trên vault ngẫu nhiên (thư mục tạm, không commit):

| Bộ dữ liệu | Cách so | Lệch |
|---|---|---|
| 25.000 note YAML ngẫu nhiên (bộ sinh của bài thử S1, hạt 1 và 2) | `vault.parse` của Python so với `vault::parse`: `props`, lỗi, link, block, `fold` | 0 |
| 15.000 note link ngẫu nhiên (5 hạt): wikilink, nhúng, link markdown có `%XX` và `../`, fence, block ID, CRLF, khoảng trắng Unicode, tên trùng khác hoa thường, alias | như trên, cộng kết quả giải từng link (khoảng 8.500 link) | 0 |
| 3 vault trên (6.000 note link, 5.000 note YAML) qua `parity.py run --private`, gói `business,capture`, chỉ `capture`, `--no-aliases`, không gói | bảng index do `index.build` của Python dựng so với bảng Rust | 0 ở cả 17 bảng |

## 2. Đã làm

- **Lớp YAML** (`crates/pkm-core/src/vault/yaml.rs` và module con): theo thiết kế ở mục 5 báo cáo bài thử S1.
  - `libyaml.rs`: bọc `unsafe-libyaml` 0.2.11. Đây là chỗ duy nhất có `unsafe`, mỗi khối có chú thích `SAFETY`. Parser đặt mã UTF-8 như PyYAML làm với `str`.
  - `yaml.rs`: composer như `CParser`, viết bằng vòng lặp nên note lồng sâu không làm tràn ngăn xếp. Resolver YAML 1.1 có hai chế độ: `Frontmatter` (bỏ `timestamp`) và `Safe` (cho `schema.yaml`). Constructor như `SafeConstructor`, dựng theo vòng của generator.
  - Mọi object Python nằm trong một mảng, nên alias dùng chung object như Python. Dict tự chứa mình in ra `{...}`, list in ra `[...]`, đúng như Python; bài thử S1 trước đây báo `RecursionError` ở chỗ này.
  - Khoá dict so bằng `==` của Python: số nguyên lớn so chính xác với số thực, `datetime` có múi giờ quy về UTC, `.nan` là một hằng dùng chung.
  - `<<` sửa node tại chỗ như Python, nên `<<` trỏ vòng vẫn dừng; bài thử S1 sẽ đệ quy mãi ở chỗ này.
  - `scalar.rs`: `int` không giới hạn độ lớn (thay cho trần 128 bit của bài thử S1), cùng giới hạn 4300 chữ số, cơ số 60 có phần âm, base64 đúng thuật toán `binascii`. Câu lỗi lấy theo Python 3.14 thật: câu múi giờ không có dấu chấm cuối (bài thử S1 có), lỗi `int()` cắt ở 200 ký tự, `!!timestamp` nhận `\n` ở cuối như `$` của Python.
  - `repr.rs`: `str()` và `repr()` của Python.
- **`vault`** (`vault.rs`, `vault/resolve.rs`, `vault/pystr.rs`):
  - `list_notes(vault, skip)` trả `BTreeMap` theo đường dẫn đã sắp. Khoá là NFC. mtime tính bằng `int(st_mtime * 1000)` giống Python. `skip` là chỗ cắm luật bỏ note của adapter.
  - `read` và `parse` (`Parsed`: `error`, `props`, `links`, `blocks`; thêm `first`, `types`, `aliases`), cùng `fold`, `title`, `folder`, `nfc`, `casefold`.
  - `Resolver` giải link với alias tuỳ chọn.
  - Regex có lookaround hoặc tham chiếu ngược được viết lại, không dùng `fancy-regex`: `MDLINK` bỏ `(?<!!)`, chỗ khớp đứng sau `!` thì tìm lại từ ký tự kế tiếp; `FENCE` tách thành hai nhánh; `NOT_NOTE` thành một hàm. `\s` của Python có thêm U+001C..U+001F, đã thêm vào regex.
  - `pystr.rs` viết lại `str.isspace`, `strip`, `split`, `urllib.parse.unquote` và `posixpath`.
- **`schema`** (`schema.rs`, `schema/schema.yaml`):
  - `schema.yaml` chép nguyên từng byte, nhúng bằng `include_str!`, digest `bbaa9f0197ef` giống `meta` của golden. `.gitattributes` đặt `-text` để Windows không đổi LF.
  - `Source::Embedded` và `Source::File` (vẫn nhận file ngoài), `read_schema`, `digest`, `resolve` (gói, `extends`, `widen`, `lifecycle_types`, R14), `ancestors`, `tables` (11 bảng `schema_*`, theo thứ tự cột của DDL), `load`, hằng `DDL`/`TABLES` để phase index dùng.
  - Schema hỏng (thiếu khoá, sai kiểu, YAML lỗi, gói không có) trả `SchemaError`, không panic.
- **Bộ so khớp:**
  - `parity/src/rust_side/note_tables.rs` dựng dòng của 17 bảng như `index.build` ghi khi build đủ: `nodes` lấy `_first`, `type`, `ntypes`, mtime ghi thành độ lệch như dump Python; `names`; `links` qua `Resolver` với alias theo `no_aliases`; `schema_*` theo `packs`.
  - `RustSide::table` gọi phần này, dựng một lần rồi giữ lại (`OnceCell`). Định dạng dump không đổi.
  - Sửa test `rust_side_reports_every_missing_layer_as_not_available_never_as_match` trong `parity/tests/golden.rs`. Test này giả định Rust chưa làm tầng nào, nên nay phải cho phép các bảng của `vault` và `schema`; mọi mục khác vẫn phải báo "chưa có".
- **Test:**
  - 36 test `vault` (gồm lớp YAML), 7 test `schema`.
  - `tests/yaml_cases.rs` so 58 ca biên của bài thử S1 với kết quả PyYAML đã ghi lại, không cần Python. File ca biên được chép vào `crates/pkm-core/tests/` và có `.gitattributes` `-text`.
  - Giá trị mong đợi trong test viết tay đã được đối chiếu với `vault.py` và `schema.py` ở sha ghim.
- **`spec/PORTING.md`:**
  - Thêm mục "Lệch có chủ ý" `yaml-python-dung-build`.
  - Dòng 17 và 18 ở bảng platform chuyển sang "đã chuyển".
  - Thêm bảng "Test `vault` và `schema`".
- **`parity/exceptions.toml`:** thêm ngoại lệ `yaml-python-dung-build` (tầng `index`, bảng `nodes`, cột `/yaml_error`).
- **Phụ thuộc** (`crates/pkm-core/Cargo.toml`): `unsafe-libyaml` 0.2.11, `regex` 1, `indexmap` 2, `sha1_smol` 1; dev-dependency `serde_json`.

## 3. YAML làm Python dừng build (quyết định của user)

Theo docs/DECISIONS.md, Rust không dừng build mà ghi `YAML lỗi: <tên kiểu ngoại lệ>` cho riêng note đó. Frontmatter của note coi như rỗng, còn thân note vẫn được đọc. Ngoài bốn dạng đã biết, tôi tìm thêm ba dạng cùng loại, đã kiểm bằng Python 3.14 và ghi chung vào một mục:

| Dạng | Ngoại lệ Python | Có từ trước |
|---|---|---|
| `!!bool maybe` | `KeyError` | có |
| `!!timestamp soon` | `AttributeError` | có |
| `!!int ""`, `!!float ""` | `IndexError` | có |
| alias vòng trong list | `RecursionError` | có |
| `!!int "-"` | `IndexError` | mới |
| list lồng khoảng 1000 cấp | `RecursionError` | mới |
| số nguyên hơn 4300 chữ số dựng từ cơ số 2, 8, 16, gặp khi `str()` | `ValueError` | mới |

Rust đặt một ngưỡng chung `MAX_DEPTH = 1000`. Python thì dừng khi list lồng khoảng 995 cấp, còn dict lồng vài nghìn cấp vẫn đọc được. Hai bên chỉ khác nhau ở dict lồng trên 1000 cấp; điểm này đã ghi trong mục lệch có chủ ý.

## 4. Lệch có tên còn lại

Các lệch dưới đây chưa gặp trong kg-solo hay trong bộ ngẫu nhiên, nhưng biết là có thể xảy ra.

- **`!!set`**: Python in phần tử theo thứ tự băm, đổi theo từng lần chạy. Rust giữ thứ tự chèn. Không có cách nào khớp ổn định. Bài thử S1 đã ghi điểm này.
- **Hai file chỉ khác nhau ở dạng NFC/NFD** (chỉ có được trên Linux): Python giữ file mà `os.scandir` trả sau, nên kết quả tuỳ hệ thống file. Rust giữ file đứng sau theo tên đã sắp. Đã ghi ở doc của `list_notes`.
  - Khi thử, công cụ dump Python (`parity/dump-python.py`, hàm `disk_mtime_ms`) báo `FileNotFoundError` trên vault có thư mục tên NFD, vì nó tìm file theo id NFC. Đây là giới hạn của bộ dump, không phải lệch của lõi. Bản B mà có tên NFD trên Linux thì sẽ gặp lỗi này.
- **Giải link luôn theo `posixpath`**, trên mọi nền. Python chạy trên Windows thì dùng `ntpath`, khác ở link chứa `\` hoặc `C:`. Golden dựng trên Linux.
- **Schema tự viết:**
  - Tên type, cạnh, trường phải là chuỗi hoặc số nguyên; dạng khác báo lỗi. Python thì nhận và so bằng `==`.
  - Số thực ghi vào cột TEXT theo `%!.15g` của SQLite. Schema nhúng không có trường hợp này.
- **`int()` và `float()`**: chỉ nhận chữ số ASCII. Python nhận cả chữ số Unicode, ví dụ `!!int ١٢`.

## 5. Kiểm tra

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` và `cargo test --workspace --locked` đều qua trên Linux.
- `cargo clippy --workspace --all-targets --locked -- -D warnings` với `--target x86_64-pc-windows-msvc` và `--target aarch64-apple-darwin` đều sạch. Lệnh này chỉ kiểm biên dịch; test trên Windows và macOS sẽ chạy ở CI.
- Con trỏ chuỗi C được ép kiểu qua `.cast()`, nên vẫn biên dịch được trên aarch64-linux, nơi `c_char` là `u8`.
- Bước so khớp của CI chạy trên máy, Python bị che bằng một lệnh giả luôn thất bại: mã thoát 0, "0 lệch chưa có tên". Dòng "Ngoại lệ không dùng tới: yaml-python-dung-build" là bình thường, vì kg-solo không có dạng YAML đó; không nên xoá ngoại lệ này.

## 6. Việc cho phase sau

- **Phase index:** dùng `vault::list_notes`, `parse`, `Parsed::first/types/aliases`, `title`, `folder`, `fold`, `Resolver`, `schema::load`, `DDL`, `TABLES`, `Cell`. Khi đã có index SQLite, `parity/src/rust_side/note_tables.rs` đổi sang đọc bảng từ DB.
- **Phase MCP:** dùng `schema::read_schema(&Source::Embedded)` để lấy mô tả type, qua `Val::get`, `as_str`.
- **Phase adapter:** cắm luật bỏ note qua tham số `skip` của `list_notes`. Trait `Adapter` nằm ở `adapter.rs`, chưa sửa vì không thuộc phase này.
- `platform` không có hàm NFC hay casefold công khai (`name_key` là hàm riêng). Vì vậy `vault` dùng thẳng hai crate mà `platform` đang dùng (`unicode-normalization`, `caseless`), không viết thêm hàm cục bộ nào tương đương. Nếu muốn gộp về một chỗ thì có thể cho `platform` dùng `vault::nfc` và `vault::casefold`.

## 7. Câu hỏi còn mở

1. Bản B chưa chạy; agent khác sẽ chạy sau. Lệnh: `python3 parity/parity.py run --private <bản B> --adapter e11 --packs capture --no-aliases --edge-keys links --body-edges`. Cần xem 17 bảng của phase này, và lỗi `FileNotFoundError` nếu bản B có tên NFD.
2. Có mở issue ở repo đặc tả cho `yaml-python-dung-build` không? Việc này cần user cho phép.
