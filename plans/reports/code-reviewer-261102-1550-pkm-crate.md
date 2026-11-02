# Review crate `pkm` + `query`/`tools`/`related` (2026-11-02 15:50 ICT)

Phạm vi: `crates/pkm/src/{main,cli,mcp,install,doctor}*`, `crates/pkm-core/src/{query,tools,related}.rs` tại HEAD `56c4b8b`. Không kiểm lại parity với Python. Bỏ qua wiring OHS trong MCP (agent khác đang làm). Không sửa code.

## Kết luận nhanh

Không thấy lỗi phá dữ liệu cấu hình Claude Desktop ở đường chính: đọc bỏ BOM, giữ CRLF, `preserve_order` bật ở cả hai crate, chỉ đụng `mcpServers.kg`, sao lưu `.bak-<UTC>` trước khi ghi, ghi file tạm + `sync_all` + `rename` (Windows: `MoveFileExW REPLACE_EXISTING`), đọc lại và so sha256 sau khi dựng index, kiểm Desktop đang chạy hai lần. stdout của MCP sạch: `pkm-core` không có `println!`, log đi `eprintln!`. `select`/`where` dùng tham số bind, không SQL injection; `hid()` nhúng literal nhưng escape `'` đúng chuẩn SQLite và dữ liệu đến từ index, không từ client.

Các lỗi thật nằm ở độ bền của MCP server (treo vĩnh viễn, DoS bởi truy vấn đồ thị) và vài cạnh Windows.

## High

### H1. Panic trong lần dựng đầu làm mọi `tools/call` treo vĩnh viễn
- `crates/pkm/src/mcp.rs:258-271` (`first_build`), `:331` (`tools_call` gọi `wait_ready(None)`).
- `first_build` chỉ gọi `set_ready()` sau khi `build()` trả về. Nếu `index::build`/`adapter::load`/`open_tools` panic (note hỏng, bug parser, lỗi index ngoài dự kiến), luồng nền chết, `ready` không bao giờ `true`. `initialize`/`tools/list` chờ 20 s rồi chạy tiếp, nhưng `tools/call` chờ không hạn → client treo, Claude Desktop thấy tool không bao giờ trả lời; server không thoát nên không có lỗi hiển thị.
- Sửa: bọc thân `first_build` bằng `std::panic::catch_unwind(AssertUnwindSafe(..))` (hoặc guard `Drop` gọi `set_ready`), ghi `build_error = "panic: ..."`; đổi `wait_ready(None)` thành có hạn và trả `{"error": "index unavailable"}` khi hết hạn.

### H2. `paths`/`loops`/`all_loops` không giới hạn → một lời gọi tool khoá server vô hạn
- `crates/pkm-core/src/query.rs:744-792` (`paths`), `:796` (`loops`), `:861` (`all_loops`); gọi từ `crates/pkm/src/mcp.rs:332-345` trong khi giữ `state` lock trên luồng duy nhất đọc stdin.
- CTE đệ quy liệt kê mọi đường đơn tới `max_len` cạnh; `max_len` lấy nguyên từ model, không trần; không `progress_handler`/`interrupt`, không giới hạn số hàng. Với vault vài nghìn note liên kết dày, `max_len=8..10` là bùng nổ tổ hợp: CPU 100%, RAM tăng tới khi bị OS giết. Vì server đồng bộ một luồng, mọi request sau (kể cả `ping`, huỷ) đều kẹt; Claude Desktop báo timeout rồi server vẫn chạy ngầm.
- Hành vi thừa hưởng Python nhưng trong Python cũng là lỗi; parity không phải lý do giữ.
- Sửa: kẹp `max_len` (ví dụ ≤ 6) ở tầng tool, thêm `LIMIT` vào CTE (ví dụ 10 000 walk) và/hoặc `conn.progress_handler` với hạn thời gian (vd 10 s) trả lỗi `"query too expensive; lower max_len"`.

## Medium

### M1. Panic trong tool trên luồng chính giết server
- `crates/pkm/src/mcp.rs:336-337` (`t.call(&name, args)`), không `catch_unwind`.
- Một panic bất kỳ trong `pkm-core` (slice index, `unwrap` trong đường hiếm) làm thoát process; Claude Desktop hiển thị "server disconnected" và không tự khởi động lại trong phiên. Mutex đã xử lý poison nên chỉ cần bắt panic.
- Sửa: `catch_unwind` quanh `t.call`, trả `isError: true` với "internal error", log stderr.

### M2. `--force` gộp hai ý: bỏ kiểm Desktop đang chạy và chấp nhận exe ở chỗ tạm
- `crates/pkm/src/install.rs:399-403`, `:662-676`.
- Trên Windows, `is_desktop_exe` (`install/desktop.rs:70-73`) coi mọi `claude.exe` ngoài `\.local\bin\`, `\node_modules\`, `\npm\` là Desktop. Claude Code cài qua winget (`...\WinGet\Packages\...`), scoop, hay `%LOCALAPPDATA%\Programs\...` → báo nhầm "Claude Desktop đang chạy". Lối ra duy nhất là `--force`, vốn cũng tắt luôn cảnh báo exe nằm ở Downloads → người dùng lần đầu dễ ghi `command` trỏ vào file tạm, sau đó `kg` hỏng khi dọn Downloads. Ngược lại, người dùng chạy `--force` để qua lỗi exe lại mất bảo vệ Desktop-đang-chạy → Desktop ghi đè cấu hình khi thoát (mất mục `kg`, không mất mục khác).
- Sửa: tách `--force` thành `--allow-exe-location` và `--ignore-running`; hoặc in đường dẫn exe bị coi là Desktop trong lỗi để người dùng biết vì sao; mở rộng danh sách Claude Code (`\winget\`, `\scoop\`).

### M3. Ghi đè symlink và quyền file khi `write_atomic`
- `crates/pkm/src/install/config.rs:347-381`.
- Nếu `claude_desktop_config.json` là symlink (dotfiles đồng bộ trên macOS/Linux), `rename` thay symlink bằng file thường → đích symlink không được cập nhật, đồng bộ dotfiles lệch âm thầm. File tạm tạo bằng `File::create` theo umask (0644), mất quyền 0600 nếu file gốc có `env` chứa token của MCP khác.
- Sửa: `fs::canonicalize(path)` trước khi chọn thư mục/đích rename; sao chép `metadata().permissions()` sang file tạm trước khi rename (Unix).

### M4. File tạm cố định theo PID, không `create_new`
- `config.rs:366-371`. Tên `<file>.tmp-<pid>`; `File::create` truncate nếu trùng (hiếm) và theo symlink nếu có kẻ đặt sẵn. Rủi ro thấp trên máy cá nhân; dùng `OpenOptions::new().write(true).create_new(true)` và hậu tố ngẫu nhiên.

### M5. Hai tiến trình server trên cùng `graph.db`
- `mcp.rs:274-287` (`refresh` build tăng dần mỗi 5 s). Claude Desktop đôi khi spawn server 2 lần; Claude Code + Desktop cùng vault. `busy_timeout` có (`index.rs:309`), WAL có; nhưng nếu build giữ transaction lâu hơn `BUSY_TIMEOUT`, lần refresh kia lỗi → chỉ log, dùng index cũ: chấp nhận được. Cần xác nhận `index::build` full rebuild không xoá/đổi tên file DB khi tiến trình khác đang mở (Windows sẽ lỗi sharing violation). Xem câu hỏi mở.

## Low

- L1. `default_db` băm `vault.to_string_lossy()` phân biệt hoa/thường và dấu `\` cuối (`install.rs:290-306`): `D:\Vault` và `d:\vault\` cho hai DB khác nhau trên Windows; macOS NFD/NFC với tên vault tiếng Việt cũng vậy. Chỉ tốn đĩa và build lại; chuẩn hoá (canonicalize, bỏ `\` cuối, lowercase trên Windows) trước khi băm.
- L2. `shell::powershell` (`install/shell.rs:24-31`) chỉ escape `'`; PowerShell coi `‘ ’ ‚ ‛` (U+2018..201B) cũng là nháy đơn. `--about` có dấu nháy cong (thường gặp khi dán từ Word) sinh lệnh gợi ý hỏng hoặc chèn được biểu thức. Thay cả bốn ký tự đó bằng bản nhân đôi.
- L3. `ConfigDoc::load` (`config.rs:249`) nhận JSON trùng khoá và giữ bản cuối; ghi lại sẽ xoá bản trùng. Claude Desktop cũng lấy bản cuối nên không đổi hành vi, chỉ khác byte.
- L4. `serve` (`mcp.rs:270-304`) đọc dòng không giới hạn (`read_until`); client lỗi gửi dòng khổng lồ thì RAM tăng tương ứng. Client là Claude Desktop tin cậy, rủi ro thấp.
- L5. Cache `more` (`tools.rs:43,199-203`) giới hạn 64 mục theo số lượng, không theo byte; kết hợp H2 một mục có thể rất lớn. Sửa H2 là đủ.

## Đã kiểm, không phải lỗi

- stdout ô nhiễm: không có `print!/println!` trong `pkm-core`; clap chỉ in ra stdout với `--help`/`--version`, không xảy ra khi Desktop gọi.
- stdin EOF: `read_until == 0` → `Ok(())`, `BrokenPipe` → thoát 0; luồng build nền không chặn thoát.
- `where` của `select`: khoá và giá trị đều bind (`query.rs:1001-1029`); khoá `type` không nối chuỗi.
- Đường dẫn Unicode/khoảng trắng trong `command`/`args`: lưu dạng mảng JSON, không ghép chuỗi; non-UTF-8 bị từ chối rõ ràng (`install.rs:466-488`). Handshake dùng `Command::args`, không qua shell.
- MS Store: `Packages\Claude_*\LocalCache\Roaming\Claude` được dò; nhiều file → bắt chọn `--config`.
- PowerShell trả đường dẫn mã hoá OEM làm lỗi ký tự tiếng Việt trong tên user, nhưng mẫu so (`\.local\bin\`...) toàn ASCII nên vẫn đúng.

## Câu hỏi mở

1. `index::build` chế độ full có xoá/đổi tên `graph.db` không? Nếu có, trên Windows sẽ lỗi khi tiến trình server khác (hoặc `doctor`) đang mở DB.
2. Có chủ đích giữ `max_len` không trần vì parity không? Nếu có cần ghi vào PORTING như sai khác chấp nhận.
