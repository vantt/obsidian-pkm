---
title: "Phase 1: Khung workspace"
status: todo
priority: P1
effort: "0,5 ngày"
prd: R00
wave: W0
dependencies: []
needs_user: false
---

# Phase 1: Khung workspace (R00)

## Mục tiêu
Dựng workspace Cargo theo kiến trúc mục 3 PRD, CI rỗng xanh trên ba nền, các file nhật ký, và điền `seed` trong `spec/spec.lock`. Mọi phase sau đứng trên phase này.

## Đọc trước
- `docs/PRD.md` mục 3 (kiến trúc), 4 (spec.lock), 12 (CI).
- `spec/spec.lock` hiện có.
- Repo đặc tả ở `5a3eb2f`: `scripts/kg/tests/` để tìm seed của fixture kg-solo (clone bằng `gh`, máy đã có quyền).

## File được sửa
- Tạo: `Cargo.toml` (workspace), `crates/pkm-core/{Cargo.toml,src/lib.rs}`, `crates/pkm/{Cargo.toml,src/main.rs}`, `rust-toolchain.toml`, `.gitignore`, `.github/workflows/ci.yml`, `docs/DECISIONS.md`, `spec/PORTING.md`.
- Sửa: `spec/spec.lock` (chỉ trường `seed`).
- `src/lib.rs` khai báo sẵn mọi module của mục 3 (`vault`, `schema`, `index`, `validate`, `query`, `tools`, `related`, `adapter`, `ohs`, `platform`) dưới dạng file rỗng, để các phase sau không cùng sửa `lib.rs`.

## Việc cần làm
- [ ] Workspace hai crate: `pkm-core` (lib), `pkm` (bin, in `--version` kèm sha đặc tả đọc từ `spec/spec.lock` lúc build).
- [ ] Khai báo module rỗng cho mọi module của `pkm-core`; `crates/pkm/src/` có `cli`, `mcp` rỗng.
- [ ] `ci.yml`: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` trên `windows-latest`, `macos-latest`, `ubuntu-latest`. Chưa có bước so khớp (phase 05 thêm).
- [ ] `docs/DECISIONS.md`: khung, mục Q3, Q4 để trống; mỗi quyết định một mục (ngày, lựa chọn, lý do, bằng chứng).
- [ ] `spec/PORTING.md`: bảng (sha Python, ngày, tiêu đề, commit Rust, trạng thái: đã chuyển / không áp dụng / lệch có chủ ý) và mục "Lệch có chủ ý".
- [ ] Tìm seed thật mà `scripts/kg/tests` dùng, điền vào `spec.lock`. Không tìm thấy thì báo `NEEDS_CONTEXT`, không bịa.

## Nghiệm thu
- `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` qua tại máy.
- `cargo run -p pkm -- --version` in phiên bản và `5a3eb2f`.
- CI xanh trên cả ba nền (lead kiểm bằng `gh run list`).
- `spec/spec.lock` có `seed` khác rỗng; báo cáo ghi file Python làm bằng chứng.

## Rủi ro
Thấp. Đẩy branch để chạy CI là hành động ra ngoài: lead xác nhận remote trước lần push đầu.
