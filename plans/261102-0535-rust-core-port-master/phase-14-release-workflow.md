---
title: "Phase 14: Workflow release"
status: done
priority: P2
effort: "0,25 ngày"
prd: "mục 12 (release.yml)"
wave: W1
dependencies: [1]
needs_user: false
---

# Phase 14: `release.yml`

## Mục tiêu
Khi có tag `v*`, build `pkm` cho bốn đích và đính kèm sha256, để user cài được binary ở phase 13 và 16 mà không phải tự build. Chưa ký mã.

## Đọc trước
- `docs/PRD.md` mục 12.

## File được sửa
- Tạo: `.github/workflows/release.yml`. Không sửa `ci.yml`.

## Việc cần làm
- [x] Đích: `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`.
- [x] Mỗi binary kèm file sha256; đăng lên GitHub Release của tag.
- [x] Thêm kích hoạt `workflow_dispatch` chỉ build và lưu artifact (không tạo Release) để thử mà không cần tag.

## Nghiệm thu
- Chạy `workflow_dispatch` ra đủ bốn artifact và sha256 khớp (`sha256sum -c`).
- Không đẩy tag thật khi chưa có user đồng ý (tạo Release là hành động ra ngoài).
