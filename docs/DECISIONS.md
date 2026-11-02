# Quyết định kỹ thuật

Nhật ký các lựa chọn PRD để agent tự chốt khi làm (crate, công cụ, cách làm). Quyết định đã chốt từ trước (D1–D10) và câu hỏi còn mở cho user nằm ở [PRD.md](PRD.md) mục 1 và 15; ở đây chỉ ghi phần chốt trong lúc làm.

Mỗi quyết định một mục, theo khung:

```markdown
## <tiêu đề>
- **Ngày:** YYYY-MM-DD
- **Lựa chọn:** chọn gì
- **Lý do:** vì sao, đã cân nhắc gì
- **Bằng chứng:** số đo, bài thử, file, commit
```

Đổi một quyết định thì thêm mục mới trỏ về mục cũ, không sửa đè.

## Q3: Crate YAML
- **Ngày:**
- **Lựa chọn:**
- **Lý do:**
- **Bằng chứng:** (bài thử S1, PRD mục 14 R01)

## Q4: `rmcp` hay JSON-RPC tự viết
- **Ngày:**
- **Lựa chọn:**
- **Lý do:**
- **Bằng chứng:** (bài thử S2, PRD mục 14 R02)

## Ghim toolchain Rust
- **Ngày:** 2026-11-02
- **Lựa chọn:** `rust-toolchain.toml` ghim `1.97.1` (kèm `rustfmt`, `clippy`); edition 2024. CI cài đúng bản đó bằng `rustup toolchain install`, không dùng action cài toolchain riêng.
- **Lý do:** CI chạy `clippy -D warnings`. Theo `stable` thì mỗi bản Rust mới có thể thêm lint và làm CI đỏ dù mã không đổi. Nâng toolchain là một commit riêng, có chủ ý.
- **Bằng chứng:** `rust-toolchain.toml`, `.github/workflows/ci.yml`.

## Thư viện CLI: `clap`
- **Ngày:** 2026-11-02
- **Lựa chọn:** `clap` 4, feature `derive`.
- **Lý do:** PRD mục 3 và 10 đã gợi ý. CLI có nhiều lệnh con và tuỳ chọn chung (giống `scripts/kg/cli.py` dùng `argparse`), viết tay không đáng.
- **Bằng chứng:** `crates/pkm/Cargo.toml`.

## `pkm --version` lấy sha đặc tả lúc build
- **Ngày:** 2026-11-02
- **Lựa chọn:** `crates/pkm/build.rs` đọc `[spec].sha` trong `spec/spec.lock` và đưa vào binary. Kết quả in ra có dạng `pkm 0.1.0 (spec <sha đủ 40 ký tự>)`. Build dừng nếu thiếu sha hoặc sha không phải 40 ký tự hex. Parser chỉ đọc dạng `key = "value"`, không thêm crate TOML vào build-dependency.
- **Lý do:** `spec.lock` là nguồn duy nhất của sha đang ghim (PRD mục 4), nên nâng ghim thì `--version` tự đổi theo. In sha đủ 40 ký tự để không nhầm khi so.
- **Bằng chứng:** `crates/pkm/tests/version.rs` so kết quả `pkm --version` với sha đọc từ `spec.lock`.

## Seed của fixture kg-solo: 11
- **Ngày:** 2026-11-02
- **Lựa chọn:** `spec.lock` ghi `[fixture].seed = 11`.
- **Lý do:** Test của repo đặc tả sinh vault mà không truyền `--seed`, nên dùng giá trị mặc định của preset. Cỡ vault cũng là mặc định (`--clients 30 --days 365`, 746 note).
- **Bằng chứng:** repo đặc tả ở `5a3eb2f`: `scripts/kg/tests/test_kg.py` (lớp `Fixture`, `setUpClass`) chạy `otf.mjs generate --preset kg-solo --out …`, không có `--seed`. Fixture `v0.7.0` (`2a514a8`): `presets/kg-solo.mjs` dòng 14 `seed: { default: 11 }` và dòng 97 `build({ seed = 11, … })`. `docs/knowledge-graph/bench/r2/README.md` mục 1 của repo đặc tả cũng ghi "hạt giống 11".

## Lệnh `follow`: `parity/follow.py`
- **Ngày:** 2026-11-02
- **Lựa chọn:** `python3 parity/follow.py` (Python chuẩn, không phụ thuộc ngoài), không dùng `cargo xtask`. Bản clone repo đặc tả nằm ngoài repo này: `$PKM_FOLLOW_CLONE`, `--clone-dir`, hoặc `~/.cache/pkm-follow/spec-repo`. Quyền đọc lấy từ `gh auth git-credential` chỉ cho lệnh git đó, không đổi cấu hình máy.
- **Lý do:** Bộ so khớp cũng cần Python để chạy bản đặc tả, nên không thêm crate hay thành viên workspace chỉ để chạy một lệnh làm tay. Không đụng `Cargo.lock`, không làm CI build thêm.
- **Bằng chứng:** `parity/follow.py`; chạy hai lần liên tiếp, tập sha khớp `git log <sha ghim>..origin/main -- <đường dẫn theo dõi>`.
