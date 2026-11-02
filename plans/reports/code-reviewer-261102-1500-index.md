# Review: index SQLite, adapter e11, validate (160d9c1..7d14601)

Thời điểm: 2026-11-02 15:00 (Asia/Saigon). Phạm vi: `crates/pkm-core/src/index*`, `validate*`, `adapter*`, `parity/src`. Đối chiếu `scripts/kg/index.py` ở spec-repo. `cargo test -p pkm-core` chạy xanh.

## Tổng quan
Bản port bám sát Python: cùng chia ba transaction (note, cạnh/schema/view, meta), cùng thứ tự đọc DB. Không thấy SQL injection: mọi `format!` trong SQL chỉ ghép tên bảng/view hằng (`NOTE_TABLES`, `schema::TABLES`, `VIEWS`) hoặc tên lấy từ `sqlite_master` (`dump_tables`); dữ liệu note/prop đi qua tham số `?`. Truy cập `v[0]` trong e11 đều có lọc `!is_empty()` trước. Lỗi nghiêm trọng nhất là về tính nguyên tử, và lỗi này thừa hưởng từ Python.

## Cao

### H1. Build hỏng giữa chừng để lại index lệch vĩnh viễn (index.rs:655-679, 693-707, 781-791)
Bảng note (`nodes` với mtime/size mới) được commit ở transaction 1. Cạnh, link, view được tính ở transaction 2. Meta được ghi ở transaction 3.
Tình huống: transaction 1 đã commit, sau đó `derive` lỗi (adapter trả `Err`, ví dụ `e11.rs:603` gặp `data.score` hỏng; lỗi `schema::load`; tiến trình bị kill; máy tắt). Lần build tăng dần sau đọc `nodes` thì thấy mtime đã khớp, nên `changed` và `removed` rỗng. `schema_digest` trong meta cũ vẫn khớp, nên bỏ qua `derive`. Kết quả là `links`/`edges` cũ được giữ mãi, và không còn note nào "đổi" để kích hoạt tính lại. Chỉ `--full` mới chữa được.
Python có cùng lỗi (`index.py:186` và `215` là hai khối `with db` riêng), nên parity không lộ ra.
Cách sửa (giữ được parity về dữ liệu): trước transaction 1, ghi `meta.dirty=1` (hoặc xoá `schema_digest`), rồi chỉ xoá cờ trong transaction meta. Khi build thấy cờ còn thì bắt buộc chạy `derive`. Cách khác: gộp transaction 1 và 2 làm một (chỉ đổi điểm commit, `edges.id` vẫn như cũ).

### H2. Transaction ghi giữ lâu trong khi đọc file; không xử lý BUSY rõ ràng (index.rs:656-678, 618)
`write_note` đọc và parse từng file bên trong transaction ghi. Với vault lớn hoặc ổ mạng/OneDrive trên Windows, write lock bị giữ hàng giây. Dưới WAL, reader (MCP/Claude Desktop) vẫn đọc được, nhưng writer thứ hai (CLI build song song với auto-rebuild của MCP) sẽ đợi theo busy_timeout mặc định 5 s của rusqlite rồi trả `SQLITE_BUSY`. `db.transaction()` là DEFERRED: tiến trình đọc trước (`SELECT ... FROM nodes`) rồi mới nâng lên ghi, nên trong WAL có thể nhận `SQLITE_BUSY_SNAPSHOT`, lỗi này không được retry. Ngoài ra `old` được đọc ngoài transaction, nên hai build song song có thể tính `changed` trên snapshot cũ.
Cách sửa: đọc và parse mọi file trước, rồi mới mở transaction. Dùng `TransactionBehavior::Immediate`. Đặt `busy_timeout` rõ ràng. Thêm khoá tiến trình (file lock cạnh DB) để chặn hai build cùng lúc.

## Trung bình

### M1. `PRAGMA journal_mode=WAL` chạy ở mỗi build (index.rs:45, 619)
Nếu DB đang ở chế độ rollback (do bản cũ hoặc tool khác tạo) và có connection khác đang mở, lệnh đổi sang WAL trả BUSY và build chết. WAL cũng không chạy trên ổ mạng hoặc SMB, nơi Windows hay đặt thư mục vault đồng bộ. Python cũng vậy. Nên bắt lỗi và tiếp tục nếu `journal_mode` không đổi được, hoặc chỉ đặt khi tạo DB mới.

### M2. Phát hiện thay đổi chỉ dựa vào (mtime ms, size) (index.rs:641-651)
Trên FAT/exFAT (USB, thẻ nhớ; mtime có độ phân giải 2 s), một lần sửa giữ nguyên số byte trong cùng cửa sổ 2 s sẽ bị bỏ sót. Công cụ đồng bộ (iCloud/Dropbox) giữ nguyên mtime gốc cũng gây ra chuyện này. Lỗi thừa hưởng từ Python. Chấp nhận được nếu ghi rõ trong tài liệu; `--full` là lối thoát.

### M3. File biến mất giữa `list_notes` và `write_note` làm hỏng cả build (index.rs:799)
Khi Obsidian ghi file (rename atomic) hoặc đang đồng bộ, `vault::read` trả NotFound, toàn bộ build lỗi và transaction 1 rollback. Lỗi này an toàn nhưng gây phiền khi rebuild tự động lúc người dùng đang gõ. Python giống hệt. Gợi ý: coi NotFound là "removed" (lệch nhẹ khỏi Python, cần quyết định).

## Thấp

- L1. `previous()` (index.rs:248-275) mở read-only bằng `SQLITE_OPEN_READ_ONLY` trên DB WAL. Nếu file `-shm` không tạo được (thư mục chỉ đọc), lần mở sẽ lỗi và hàm trả về `default` một cách im lặng, nên `Options::pick` quay về mặc định và có thể đổi `options` dẫn tới full rebuild. Ít khi xảy ra.
- L2. Rename chỉ đổi hoa/thường trên FS không phân biệt hoa thường (Windows/macOS): được xử lý đúng như một cặp removed + changed, vì khoá là chuỗi đường dẫn. Riêng chuẩn hoá NFC/NFD trên macOS phụ thuộc vào `vault::list_notes` (nằm ngoài diff); e11 có `nfc()` cho `data.to`, nhưng `known` lại so với id thô. Nếu id không được chuẩn hoá NFC ở `list_notes`, `fix-link` sẽ báo sai `ok=false` trên APFS với tên tiếng Việt dạng NFD. Cần kiểm tra lại `list_notes`.
- L3. `i64::try_from(size).unwrap_or(i64::MAX)` không gây panic, ổn.

## Không phải lỗi (đã kiểm tra)
- SQL injection: không có. `validate.rs:36-60, 276-287` chỉ ghép SQL hằng. `index.rs:659, 671, 850, 859` dùng tên bảng hằng. `dump_tables` lấy tên từ `sqlite_master` và đặt trong ngoặc kép.
- Panic: `regexp` dùng `from_utf8_lossy`, mẫu lỗi trả `UserFunctionError`, cache có xử lý mutex poison. Regex trong e11 là hằng.
- Phát hiện adapter không có digest: `matches!(adapter_digest, Some(None))` khớp với `not hasattr(adapter,'digest')`.

## Đề xuất (theo thứ tự)
1. Sửa H1: cờ dirty trong meta hoặc gộp transaction 1 và 2.
2. Sửa H2: parse ngoài transaction, dùng transaction IMMEDIATE, thêm file lock cho build.
3. M1: chịu được lỗi khi đặt `journal_mode`.
4. Kiểm tra NFC ở `list_notes` (L2).

## Câu hỏi còn mở
- H1 và M3 sửa xong sẽ lệch khỏi hành vi Python khi gặp lỗi (dữ liệu khi build thành công vẫn y hệt). Có chấp nhận không?
- Có kịch bản MCP tự rebuild song song với CLI không? Nếu có thì H2 là bắt buộc.
