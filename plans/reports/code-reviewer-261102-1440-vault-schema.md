# Review: module `vault` + `schema` (2c8b25a..8f64929)

Ngày: 2026-11-02 (Asia/Saigon). Người review: code-reviewer. Không sửa code.

## Phạm vi
- `crates/pkm-core/src/vault.rs`, `vault/{pystr,resolve,yaml}.rs`, `vault/yaml/{libyaml,repr,scalar}.rs`, `schema.rs`, `schema/schema.yaml`, `tests/yaml_cases.rs`, `parity/src/rust_side*.rs` (~4.8k dòng).
- Đối chiếu: `spec-repo/scripts/kg/vault.py`, `schema.py`; chạy thử Python 3.14.4 + PyYAML 6.0.3 (libyaml) so với crate qua binary tạm trong scratchpad.

## Kiểm chứng đã chạy
- Differential 33 ca YAML (anchor trùng, alias vòng, `!!set`, `!!omap`, `<<`, CRLF trong block/quoted scalar, ký tự điều khiển, tag lạ, `!!python/name`, `0o17`, sexagesimal, `1e400`, khoá `1/1.0/true`...): Rust khớp Python từng ca (lỗi và `props`).
- FFI `libyaml.rs`: event luôn được `yaml_event_delete` sau khi chép; parser chỉ `yaml_parser_delete` khi đã init thành công; con trỏ null được kiểm; input sống theo lifetime `'a`. Không thấy lỗi bộ nhớ.

## Phát hiện (theo mức độ)

### High
**H1. Alias bomb làm nổ bộ nhớ khi làm phẳng / in** — `vault.rs:347-367` (`flat`), `vault/yaml.rs:306-400` (`repr_into`).
- Kịch bản: note 338 byte, 7 tầng `&aN [*aN-1 x10]` -> 11.1 triệu props, RSS 1.45 GB (Rust 0.84s). Thêm 1 tầng (vẫn < 400 byte) -> ~14 GB, process index bị OOM-kill; một note độc trong vault đồng bộ làm chết cả lần index. Python cũng nổ (877 MB, 3.5s), nên đây là parity nhưng là DoS thật.
- Composer/constructor thì an toàn (object dùng chung theo chỉ số); chỗ nổ là mở rộng ra `props` và chuỗi `repr`.
- Sửa: ngân sách mở rộng cho mỗi note (ví dụ tổng số lá `flat` hoặc tổng độ dài `repr` > N) -> `Fail::Crash("MemoryError")`, ghi vào "Lệch có chủ ý" của spec/PORTING.md như với RecursionError.

### Medium
**M1. Dict lồng sâu > 1000 lỗi ở Rust, Python đọc bình thường** — `vault/yaml.rs:43` (`MAX_DEPTH`), `:600` (composer), `:388` (repr dict).
- Kịch bản (đã chạy): `a:\n  b: {x: {x: ... 1200 hoặc 3000 tầng}}`. Python 3.14: `error=None`, props `a.b` là chuỗi repr dài 8401/21001 ký tự (repr của dict trong C không bị `sys.getrecursionlimit` chặn). Rust: `YAML lỗi: RecursionError`, mất toàn bộ props của note.
- Với list thì đúng: Python ném `RecursionError` ở `flat` (1200 tầng).
- Sửa: chỉ áp giới hạn 1000 cho chuỗi list lồng mà `flat` đi qua; composer đã là vòng lặp nên không cần giới hạn đó; `repr_into` cần giới hạn cao hơn (chạy trên thread stack lớn hoặc viết lặp) để khớp, hoặc ghi rõ lệch có chủ ý. Mốc biên của list (1000 trong Rust tính cả dict gốc, Python ~990-1000 tuỳ khung gọi) cũng chỉ gần đúng — nên có test biên.

**M2. Gộp khoá trùng là O(n²)** — `vault/yaml.rs:800-825` (`mapping`, `out.iter().position(key_eq)`).
- Đo: 20k khoá top-level hoặc lồng: Rust 0.60s, Python 0.07s. 200k khoá (~3 MB frontmatter) ước ~60s/note; kết hợp `<<` lặp alias còn tệ hơn. Python dùng dict băm nên tuyến tính.
- Sửa: chỉ mục `HashMap<KeyHash, Vec<idx>>` với khoá chuẩn hoá (Str/Bytes/None/Date theo giá trị; số quy về `Num`; DateTime có tz quy về instant; NaN theo id object), chỉ gọi `key_eq` trong bucket.

### Low / Thông tin
**L1. Một file hỏng làm hỏng cả `list_notes`** — `vault.rs:123` (`std::fs::metadata(&path)?`), `:111`.
- Symlink `.md` gãy hoặc file mất giữa lúc liệt kê (Obsidian sync đang ghi) -> `Err` cho cả vault. Python cũng crash (`e.stat()`), nên là parity; nhưng đáng cân nhắc bỏ qua/ghi log từng file ở bản Rust (lệch có chủ ý) vì race với sync là chuyện thường.

**L2. `list_windows_drive` (WSL nhìn ổ Windows) không được port** — `vault.rs:98-108` so với `vault.py:44-50`. Cần xác nhận đây là quyết định đã ghi (không thấy trong phạm vi diff). Khi Rust chạy native trên Windows thì không cần.

**L3. `convert` trả `None` khi `usize::try_from(length)` lỗi** — `vault/yaml/libyaml.rs:128`: sẽ âm thầm bỏ một scalar. Không thể xảy ra trên 64-bit; chỉ ghi nhận.

## Không phải lỗi (đã kiểm)
- CRLF: FM_RE, BLOCK `\r?$`, libyaml chuẩn hoá xuống dòng trong scalar — khớp Python.
- `schema.yaml` nhúng có `.gitattributes -text` nên digest sha1 ổn định trên Windows. `schema.read_schema` Python mở text mode (dịch CRLF) còn Rust đọc thô, nhưng libyaml coi CRLF là ngắt dòng và chuẩn hoá trong scalar -> cùng kết quả.
- NFC/NFD: khoá `list_notes` là NFC, giữ `path` thật — đúng cho macOS.
- `mtime_ms`, `normpath/dirname/join`, `unquote`, `is_attachment`, sort của Resolver: khớp Python theo đọc mã và test.
- Nested deep `[` 5000 tầng: composer lặp, không tràn stack.

## Đề xuất ưu tiên
1. Ngân sách mở rộng cho `flat`/`repr` (H1).
2. Chỉ mục băm cho `mapping` (M2).
3. Quyết định về giới hạn độ sâu dict (M1): khớp Python hoặc ghi lệch có chủ ý + test.
4. Cân nhắc lỗi theo file trong `list_notes` (L1).

## Câu hỏi mở
- L2: bỏ `list_windows_drive` đã được ghi trong docs/DECISIONS.md chưa?
- H1/L1: chủ dự án có chấp nhận lệch có chủ ý so với Python để tránh OOM/crash không?
