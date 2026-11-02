# Bài thử S1: đọc frontmatter như PyYAML

Crate riêng, không thuộc workspace. So cách các crate YAML của Rust đọc frontmatter với cách bản Python ở sha ghim đọc (`scripts/kg/vault.py`: `CSafeLoader`, bỏ resolver timestamp). Kết quả và quyết định: [`plans/reports/probe-s1-yaml-261102-1317.md`](../../plans/reports/probe-s1-yaml-261102-1317.md), mục Q3 trong [`docs/DECISIONS.md`](../../docs/DECISIONS.md).

## Chạy

Cần Rust (toolchain ghim của repo), Python 3 có PyYAML, và repo đặc tả checkout ở sha trong `spec/spec.lock`.

```sh
# Một vault: in số lệch theo loại, không in nội dung note (an toàn cho vault riêng)
cargo run --release --manifest-path probes/s1-yaml/Cargo.toml -- \
  compare --spec <repo đặc tả> <thư mục vault>

# Thêm --show để in từng note lệch kèm giá trị (chỉ dùng cho kg-solo hoặc bộ ca thử)
```

Dòng đầu báo cáo cho biết PyYAML trên máy có libyaml (`CSafeLoader`) hay không. Không có thì bản Python tự ghi lỗi YAML bằng câu khác, và báo cáo in cảnh báo.

## File

- `pyyaml-dump.py`: phía Python. Gọi thẳng `vault.list_notes`, `vault.read`, `vault.parse` của repo đặc tả, xuất JSON có kiểu.
- `src/py11.rs`: lớp tương thích PyYAML (compose, resolve YAML 1.1, `SafeConstructor`) đặt trên luồng sự kiện của parser.
- `src/pyvalue.rs`: kiểu giá trị Python và `str()`/`repr()` mà `vault.flat` dùng để ra props.
- `src/events.rs`: luồng sự kiện từ `unsafe-libyaml`, `libyaml-safer`, `saphyr-parser`.
- `src/naive.rs`: dùng thẳng `serde_norway` và `saphyr`, không có lớp tương thích, để đo cái giá của việc không làm gì.
- `cases/` và `cases.pyyaml.jsonl`: 58 ca biên sinh bằng `make-cases.py`, kèm kết quả của PyYAML. `cargo test` so lớp tương thích với file này, không cần Python.
- `make-fuzz.py`: sinh N note ngẫu nhiên từ các token YAML dễ lệch, để so khác biệt trên số lớn.
