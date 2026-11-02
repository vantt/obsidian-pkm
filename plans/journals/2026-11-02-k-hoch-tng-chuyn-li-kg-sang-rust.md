---
title: Kế hoạch tổng chuyển lõi kg sang Rust
date: 2026-11-02
summary: Viết plan tổng 17 phase từ PRD làm đầu vào goal cho agent lead
---

# Kế hoạch tổng chuyển lõi kg sang Rust

## Chuyện gì đã làm
Viết `plans/261102-0535-rust-core-port-master/` từ `docs/PRD.md`: 17 phase phủ R00–R14, chia thành các đợt W0–W8, cổng người dùng G0 và các điểm đuổi theo C0–C3. Plan dành cho agent lead điều phối agent con qua `/ak-plan --fast` rồi `/ak-cook`.

## Quyết định
- Tách `platform` khỏi R11 thành phase 07 để chạy ở W1, rút ngắn W6.
- Golden kg-solo (R05) gộp vào phase bộ so khớp, vì golden phải theo định dạng dump của bộ so khớp.
- Phần so chạy trong CI phải viết bằng Rust, vì CI không có Python (Q5).
- C0 nâng ghim ngay sau G0, khi chưa có lõi Rust, nên việc đuổi theo lúc đó rẻ nhất.
- `related` không có `hits` chỉ so được trên máy có gói OHS; CI không có OHS.

## Bước tiếp theo
Agent lead bắt đầu với phase 01, rồi giao song song 7 phase của W1. Trước phase 08 cần user mở cổng G0.

> Historical work record — not durable authority. Prefer docs/specs/ADRs for current decisions.
