# Review: module `pkm-core::ohs` (nhánh `worktree-agent-a94d3311e16f541ad`)

Phạm vi: `crates/pkm-core/src/ohs.rs`, `ohs/process.rs`, `ohs/state.rs`, `ohs/package.rs` (đọc lướt), `tests/ohs_fake.rs`, `tests/support/*`. Đối chiếu PRD mục 9, D7, phase-11. Chỉ đọc, không sửa code, chưa chạy test.

## Đánh giá chung
Thiết kế đúng hướng: một luồng giám sát duy nhất spawn/đóng tiến trình, waiter được huỷ khi stdout đóng (request đang chờ nhận `Disconnected` ngay, không treo), `Proc::drop` kill + reap làm lưới an toàn, hình dạng lỗi D7 đúng 5 khoá và đúng tên trạng thái. Nhưng có hai lỗi thật làm OHS treo cứng và `pkm` treo khi tắt, cả hai trong điều kiện sản xuất (Windows tiếng Việt).

## High

### H1. Luồng đọc stderr dừng khi gặp byte không phải UTF-8, rồi OHS treo vì pipe stderr đầy
`process.rs:113-122` (`BufReader::lines()` + `let Ok(line) = line else { break }`).
- Kịch bản: Node/onnxruntime/thư viện native in cảnh báo theo code page hệ thống (Windows tiếng Việt CP1258) hoặc lỗi chứa byte rác. `lines()` trả `Err(InvalidData)`, luồng thoát, không ai đọc stderr nữa. Pipe trên Windows chỉ khoảng 4 KB, trên Linux 64 KB: khi đầy, mọi lần `console.error` của OHS chặn, event loop của Node đứng. stdout vẫn mở nên không có `Event::Exited`; supervisor không phát hiện; mọi `search` hết hạn sau 60 s, `state()` báo `starting` (status timeout) mãi mãi. Không khởi động lại, không `broken`.
- Sửa: đọc bằng `read_until(b'\n')` + `String::from_utf8_lossy`, và luôn đọc tới EOF (lỗi đọc thì vẫn tiếp tục hút cho đến EOF/lỗi I/O thật). Thêm test fake in byte `0xFF` ra stderr rồi in tiếp > 64 KB.

### H2. Ghi stdin giữ mutex vô hạn; `close()` cần cùng mutex đó, nên tắt `pkm` treo vĩnh viễn
`process.rs:191-194` (`write_all` + `flush` khi giữ `lock(&self.stdin)`), `process.rs:236` (`close` lấy `lock(&self.stdin)` trước khi tới bước kill), `ohs.rs:354-361` (`stop` join supervisor không giới hạn).
- Kịch bản: OHS còn sống nhưng ngừng đọc stdin (event loop kẹt, ví dụ do H1, hoặc nhúng đồng bộ dài). Request tích lại tới khi pipe stdin đầy; một caller kẹt trong `write_all` giữ mutex. Các caller sau kẹt ở `lock` (bỏ qua `request_timeout`: timeout chỉ áp ở `recv_timeout`, sau bước ghi). Supervisor đang `observe` cũng kẹt ở `call`, hoặc khi nhận `Shutdown` thì kẹt ở `close()` dòng 236. `Drop for Ohs` join supervisor không hạn: `pkm` không thoát được, Claude Desktop phải giết nó; node vẫn bị kẹt.
- Sửa: tách đường kill khỏi mutex stdin. Ví dụ `close()` dùng `try_lock` cho stdin; nếu bận thì bỏ qua bước đóng stdin, chờ `timeout` rồi `child.kill()` (kill làm write đang chặn trả EPIPE và nhả mutex). Ngoài ra nên ghi stdin bằng một luồng writer riêng nhận qua channel, để `call` không bao giờ chặn quá `timeout`.

## Medium

### M1. Luồng đọc stdout cũng `break` khi gặp UTF-8 lỗi: một dòng kết quả hỏng giết cả tiến trình
`process.rs:126-127`. Một snippet chứa byte lỗi (Node thường thay bằng U+FFFD nên hiếm, nhưng log của thư viện native in thẳng vào fd 1 thì có) làm reader coi là đã thoát, supervisor kill sau `EXIT_GRACE` và khởi động lại; lặp lại theo cùng câu tìm là cạn hết số lần khởi động lại, rồi `broken`. Sửa giống H1: dùng `from_utf8_lossy`, dòng không parse được thì coi là `Line::Other`.

### M2. Không phát hiện tiến trình chết nếu stdout vẫn mở (process con của node giữ pipe)
`ohs.rs:506-531` chỉ dựa vào `Event::Exited` (EOF stdout); không gọi `try_wait` định kỳ. Nếu OHS spawn tiến trình con thừa kế fd 1 (worker, trình tải model), node chết nhưng EOF không tới: request chờ hết `request_timeout`, không có lần khởi động lại nào. Thêm vào đó `kill()` chỉ giết node, không giết cây tiến trình (Windows không có Job Object, Unix không có process group), nên tiến trình cháu có thể mồ côi. Sửa: trong vòng `recv_timeout(poll_interval)`, gọi `child.try_wait()`; Windows gán node vào Job Object có `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`; Unix dùng `process_group(0)` và kill `-pgid`. Cần xác nhận OHS có spawn tiến trình con hay không.

### M3. Mỗi `search` gửi thêm `status` trước; OHS bận thì báo sai `starting`
`ohs.rs:310-314`, `ohs.rs:181`. Khi OHS đang tải nặng (nhiều request đồng thời, index lại), `status` quá `status_timeout` (5 s) và bị coi là `OhsState::Starting`: agent nhận `ohs_not_ready/starting` với action "đang khởi động" dù tiến trình đã chạy nhiều giờ. Như vậy D7 báo sai trạng thái, đồng thời độ trễ tăng gấp đôi số request. Sửa: lưu trạng thái lần `observe` gần nhất (supervisor đã poll), chỉ hỏi lại khi cũ hơn N giây; timeout của status lúc đang `Running` nên báo theo trạng thái lần trước, không lùi về `starting`.

### M4. `CallError::Remote` ở `status` được coi là `broken` (trạng thái cuối) dù supervisor vẫn chạy
`ohs.rs:182-184` cùng `state.rs:50-52`. Một lỗi `status` thoáng qua làm `wait_ready` (CLI) dừng ngay và báo người dùng "cài lại gói OHS". Hoặc phân loại là `starting` khi tiến trình vẫn `Running`, hoặc để supervisor chuyển `Phase::Broken` thật sự sau vài lần lỗi liên tiếp.

## Low

- L1. `process.rs:113,124`: `thread::spawn` panic nếu OS không tạo được luồng. Lúc đó `child` là biến cục bộ, `Child` drop không kill, nên node mồ côi. Dùng `thread::Builder::spawn` và kill/wait khi lỗi.
- L2. `ohs.rs:280-289`: `wait_ready` có thể vượt `timeout` thêm tới `status_timeout` (mỗi vòng gọi `state()`). Chấp nhận được, nên ghi rõ trong doc.
- L3. Tắt trong lúc supervisor đang `observe` (`ohs.rs:526`) chậm thêm tới `status_timeout` trước khi xử lý `Shutdown`. Có giới hạn nên chỉ ghi nhận.
- L4. D7: `progress`/`eta_s` là `null` ngoài `indexing`, trong khi ví dụ ở PRD 8 có số. Hợp lý, nhưng nên ghi vào `docs/MCP-CHANGELOG.md`/spec để người gọi biết là nullable.

## Test có thể flaky trên CI (Windows/macOS)
- `tests/ohs_fake.rs:392-405` (`restart_budget_is_refilled…`): `stable_after` 50 ms so với tiến trình sống 150 ms sau khi in `ready`. `ran` được đo từ lúc supervisor *xử lý* `Ready`. Trên runner Windows chậm, việc nhận `Ready` trễ hơn khoảng 100 ms thì `ran` < 50 ms, `max_restarts: 1` làm module thành `broken` và test treo tới hết `WAIT`. Nên nới khoảng cách (sống 500 ms, `stable_after` 50 ms).
- `tests/ohs_fake.rs:208-212`: ngưỡng < 1000 ms cho 6 request song song, mỗi request có thêm một vòng `status`. Trên runner Windows bị nghẽn thì mong manh. Nên đặt khoảng 2000 ms, hoặc so với tổng tuần tự (1050 ms + lề) theo cách tương đối.
- `tests/support/process.rs:6-23`: `alive(pid)` có thể dương tính giả nếu PID bị tái dùng ngay (Windows tái dùng PID nhanh). Hiếm; chấp nhận được.
- `tests/ohs_fake.rs:244`: eta dựa vào `sleep(1000)` và mẫu đầu do supervisor lấy. Khoảng `1..=5` đủ rộng, ổn.

## Không phải lỗi (đã kiểm)
- Request đang chờ khi tiến trình chết: reader `map.clear()` (process.rs:142-146), sender bị drop nên `recv_timeout` trả `Disconnected` ngay; `register` sau khi `closed` thì trả lỗi ngay. Không treo.
- Sự kiện của thế hệ cũ bị bỏ qua nhờ khớp `generation`; `Phase::Stopped` không bị supervisor ghi đè (`Shared::set`).
- `pkm` bị giết cứng: stdin của node nhận EOF, OHS tự thoát (theo giao thức), nên không mồ côi ở trường hợp thường.
- Supervisor panic: Arc<Proc> drop khi unwind, `Proc::drop` kill + reap.

## Câu hỏi còn mở
- OHS (fork 0.17.2-vantt.1) có spawn tiến trình con hoặc ghi log native thẳng ra fd 1/2 không? Câu trả lời quyết định mức của M1/M2.
