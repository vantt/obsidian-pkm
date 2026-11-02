---
title: "Phase 8: vault và schema"
status: todo
priority: P1
effort: "2,5 ngày"
prd: R06
wave: W2
dependencies: [2, 5, "G0"]
needs_user: true
---

# Phase 8: `pkm-core::vault` và `pkm-core::schema` (R06)

## Mục tiêu
Đọc vault và nạp schema cho ra đúng như Python: các bảng `nodes`, `props`, `links`, `raw_links`, `blocks`, `names`, `schema_*` khớp golden kg-solo và khớp bản B. Phase đầu tiên viết lõi, nên chỉ bắt đầu sau cổng G0.

## Đọc trước
- `docs/PRD.md` mục 2.1, 4, 7 (dòng `vault`, `schema`), 16.
- Báo cáo phase 02 (crate YAML, lớp chuẩn hoá giá trị) và mục Q3 trong `docs/DECISIONS.md`.
- `parity/README.md` (phase 05).
- Repo đặc tả ở sha ghim: `scripts/kg/vault.py`, `scripts/kg/schema.py`, `docs/knowledge-graph/schema/schema.yaml`, phần test liên quan trong `scripts/kg/tests/test_kg.py`.

## File được sửa
- `crates/pkm-core/src/vault*`, `crates/pkm-core/src/schema*` và test; `crates/pkm-core/Cargo.toml` (thêm phụ thuộc).
- Phần cắm Rust của bộ so khớp cho tầng bảng nói trên (trong `parity/`, chỉ thêm, không đổi định dạng).
- Thêm dòng vào `spec/PORTING.md`.

## Việc cần làm
- [ ] Tách frontmatter (crate và lớp chuẩn hoá theo Q3), link, block; giải link kiểu Obsidian, alias khi bật (`--no-aliases` tắt).
- [ ] Giữ CRLF hay LF như Python; đường dẫn trong DB ở dạng NFC (dùng `platform` nếu phase 07 đã merge, không thì viết hàm cục bộ và ghi việc gộp lại).
- [ ] Bỏ trang theo luật adapter (giao diện trait `Adapter` tối thiểu; phần e11 ở phase 09).
- [ ] Regex có lookaround: viết lại hoặc dùng `fancy-regex` ở đúng chỗ đó.
- [ ] Schema nhúng bằng `include_str!` và vẫn nhận file ngoài; nạp gói (`--packs`), `extends`; ghi bảng `schema_*`.
- [ ] Chuyển các test Python của `vault` và `schema`.

## Nghiệm thu
- Bộ so khớp tầng bảng (các bảng nêu trên) trên kg-solo so với golden: 0 lệch chưa có tên; chạy cả trong CI.
- `parity --private <bản B>` trên máy user: 0 lệch chưa có tên cho các bảng này.
- `cargo test -p pkm-core` qua trên ba nền.

## Cần user
Cổng G0; chạy bản B.

## Rủi ro
YAML lệch dạng mà phase 02 chưa gặp: thêm vào lớp chuẩn hoá, ghi vào báo cáo; lệch không giải thích được thì gọi `kongming` với dòng lệch và giá trị Python tương ứng.
