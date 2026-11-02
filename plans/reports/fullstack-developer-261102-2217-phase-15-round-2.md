# Đuổi theo lượt 2: từ `223c9c3` tới `da62ecb`

Ngày 2026-11-02. Lượt đuổi theo thứ hai của phase 15, user cho chạy.

## Kết quả

`spec.lock` ghim `da62ecb31032ab02afe0f58bee5ed4acc4d9eb78` (trước: `223c9c3`), là đầu `origin/main` của repo đặc tả lúc làm. Golden dựng lại ở `parity/golden/da62ecb31032ab02afe0f58bee5ed4acc4d9eb78/`, thư mục golden cũ đã xoá; khác golden cũ đúng một dòng `spec_sha` trong `manifest.json`. `python3 parity/follow.py`: 0 commit chưa chuyển. Khoảng cách trước lượt: 5 commit chạm đường dẫn theo dõi, dưới 1 ngày (đúng ngưỡng 5 commit của mục tiêu).

Từ `223c9c3` tới `da62ecb` có 19 commit; 14 commit còn lại chỉ đổi docs, plan, hay `scripts/ohs/` (công cụ từ điển và bộ câu riêng của OHS), ngoài đường dẫn theo dõi.

## Từng commit

| Sha | Xử lý |
|---|---|
| `24f193a` | **Đã chuyển.** `kg_adapter.SYSTEM` thêm `_ohs/`: `adapter::e11::SYSTEM` thêm `_ohs/` (commit Rust `3211180`), meta `hidden` của adapter có thêm mục này; test `adapter::tests::counts` và `is_system` kiểm `_ohs/lexicon.md` ẩn, `_ohs.md` không. Phần `ohs_settings` (mẫu bỏ qua `OBSIDIAN_IGNORE_PATTERNS`) đã có trên `main` từ `2fd131b`: `PACK_IGNORE_PATTERNS` giống từng byte `OHS_IGNORE`. `WSLENV`, `setup_vault.py`, `.mjs`, `propose.py`, `zones.py`: không áp dụng |
| `58010ad` | Đã có (`328716f`): công tắc `PACK_IGNORE_ON` như `OHS_IGNORE_ON` |
| `8f89403` | Đã có: `PACK_IGNORE_ON = true` và mẫu `.obsidian/**,templates/**,*.canvas,_ohs/**`, đối chiếu lại với `platform_paths.py` ở `da62ecb`, không sửa |
| `6af65b1` | Đã có: Rust chỉ chạy gói dựng sẵn của fork (`OhsPackage::from_dir` đòi node và `cli.js` của gói; bộ cài tải Release `v0.17.2-vantt.4` của fork). Test mới `test_folder_without_bundle_points_at_missing_files` ứng với `package_requires_node_and_cli` |
| `9f0700b` | Không áp dụng: repo đặc tả bỏ `desktop_config.py` vì `pkm mcp install` đã thay nó; Rust không có gì để bỏ |

Không có lệch có chủ ý mới. Golden không đổi nội dung vì kg-solo chạy không adapter.

## Kiểm

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`: sạch.
- `cargo test --workspace --locked`: qua hết.
- Golden ở `da62ecb` (`parity.py golden --sha`): dump Python hai lần khớp (64 mục, 0 lệch chưa có tên, 0 mục chưa có), `fixture_check` graph 70/70, table 11/11.
- Rust với golden mới, chạy như CI (vault `npx github:vantt/obsidian-test-fixture#v0.7.0 generate --preset kg-solo --seed 11 --out <tmp>/kg-solo`, `parity check --dump-out <tmp>/rust-dump`): 64 mục, 0 lệch chưa có tên, 0 mục chưa có, mã 0.
- CI trên `main` sau khi đẩy: xem mục cuối.

## File đổi

- `crates/pkm-core/src/adapter/e11.rs`, `crates/pkm-core/src/adapter/tests.rs`
- `spec/spec.lock`, `parity/golden/` (thay thư mục golden)
- `spec/PORTING.md`: năm dòng nhật ký; bảng test `platform` (24 test: dòng 21 đổi theo `6af65b1`, thêm dòng 24 `test_ignore_patterns_follow_the_switch`); mục bộ cài ghi việc bỏ `desktop_config.py` và sửa tên asset ghim còn ghi `vantt.3` thành `vantt.4` (khớp `install::ohs_bundle`)
- `docs/DECISIONS.md`: mục "OHS: một chỗ cài mỗi máy…" ghi phần adapter đã chuyển
- `plans/261102-0535-rust-core-port-master/phase-15-chase-python-commits.md`, `plan.md` (dòng trạng thái phase 15)

## Câu hỏi còn mở

- Công tắc `PACK_IGNORE_ON` là hằng lúc biên dịch, nên test Rust chỉ kiểm nhánh đang bật; Python giả cả nhánh tắt bằng mock. Đủ cho hiện tại; nếu sau này cần tắt mà không dựng lại binary thì phải thêm cấu hình lúc chạy.
