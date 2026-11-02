# Báo cáo: lệnh follow

- Trạng thái: xong.
- Chọn `parity/follow.py` (Python chuẩn, `python3 parity/follow.py`), ghi vào `docs/DECISIONS.md`.
- Clone nằm ngoài repo (`$PKM_FOLLOW_CLONE`, `--clone-dir`, hoặc `~/.cache/pkm-follow/spec-repo`); lần hai chỉ `git fetch`.
- Quyền đọc: `gh auth git-credential` truyền qua `-c` cho từng lệnh git, không đổi cấu hình, không token mới.
- Ở sha ghim `5a3eb2f`, `origin/main` (`aaffe2e`) chưa có commit nào chạm đường dẫn theo dõi, nên đầu ra thật là 0 commit (đạt).
- Kiểm chứng nhánh có commit: dùng bản sao ở scratch với ghim `57c68d5` và một sha giả lập trong PORTING; ra 7 commit đúng tập `git log`, commit giả lập đánh dấu đã xử lý, dòng cuối "6 commit, 1 ngày, chưa đạt".
- `cargo fmt --check`, `clippy -D warnings`, `cargo test --workspace` qua. Không workflow CI nào gọi lệnh.
