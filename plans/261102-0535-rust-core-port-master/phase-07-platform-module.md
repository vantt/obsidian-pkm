---
title: "Phase 7: Module platform"
status: done
priority: P2
effort: "0,5 ngày"
prd: "R11 (platform)"
wave: W1
dependencies: [1]
needs_user: false
---

# Phase 7: `pkm-core::platform` (phần platform của R11)

## Mục tiêu
Chuyển phần không phải WSL của `scripts/platform_paths.py`: thư mục cache theo OS, `move` không đè, trùng hoa thường và NFC/NFD, thử lại khi file bị giữ. Kèm 19 test chuyển từ Python. Làm sớm vì độc lập; phase 08 (đường dẫn NFC) và phase 13 (thư mục dữ liệu) dùng lại.

## Đọc trước
- `docs/PRD.md` mục 2.1 (dòng `platform_paths.py`), 2.2 (phần WSL ngoài phạm vi), 7 (dòng `platform`).
- Repo đặc tả ở sha ghim: `scripts/platform_paths.py`, `scripts/tests/test_platform_paths.py`.

## File được sửa
- `crates/pkm-core/src/platform*` và test của nó. Thêm phụ thuộc vào `crates/pkm-core/Cargo.toml` nếu cần (`dirs`, `unicode-normalization`).

## Việc cần làm
- [x] Thư mục cache và dữ liệu: `%LOCALAPPDATA%\pkm`, `~/Library/Application Support/pkm`, `~/.cache/pkm`, giống quy ước của Python.
- [x] `move` không đè đích; báo lỗi rõ khi đích đã có.
- [x] Phát hiện trùng tên khác hoa thường và khác dạng NFC/NFD.
- [x] Thử lại có giới hạn khi file bị giữ (OneDrive, Obsidian trên Windows).
- [x] Chuyển 19 test không phải WSL; test nào chỉ dành cho WSL ghi "không áp dụng" trong `spec/PORTING.md`.

## Nghiệm thu
- `cargo test -p pkm-core platform` qua trên cả ba nền trong CI.
- `spec/PORTING.md` có bảng 19 test Python ↔ test Rust tương ứng.
