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
- **Ngày:** 2026-11-02
- **Lựa chọn:** `unsafe-libyaml` 0.2.11, chỉ dùng luồng sự kiện của parser, cộng một lớp tương thích PyYAML tự viết trong `pkm-core::vault`: composer như `CParser`, resolver YAML 1.1 của PyYAML (tắt timestamp cho frontmatter như `vault.LOADER`, bật cho `schema.yaml`), constructor như `SafeConstructor`, và `str()`/`repr()` của Python để ra `props`. Phần `unsafe` nằm gọn trong một module. Không dùng thẳng crate YAML nào ở mức giá trị.
- **Lý do:** `vault.py` đọc bằng `CSafeLoader`, tức libyaml 0.2.5, và lưu dòng đầu thông báo lỗi YAML vào `nodes.error`; muốn khớp cả giá trị lẫn câu lỗi thì parser phải là libyaml. Crate dùng thẳng (`serde_norway`, `saphyr`) theo YAML 1.2 nên lệch ở bool `yes/no/on/off`, số `010`, `1_000`, `10:30`, `0912…`, `1e3`, khoá trùng, `<<`, tag tường minh và câu lỗi. `saphyr-parser` kèm cùng lớp vẫn lệch vì parser khác libyaml. `libyaml-safer` 0.3.0 (bản an toàn của libyaml) panic khi block scalar là giá trị cuối của frontmatter (issue #6, #7 ở upstream, còn mở). `unsafe-libyaml` đã đóng băng (archived 2024-03) nhưng libyaml 0.2.5 cũng đóng băng và là đúng bản PyYAML dùng. Khi `libyaml-safer` sửa hai issue thì chạy lại bài thử; 0 lệch thì đổi sang để bỏ `unsafe`.
- **Bằng chứng:** bài thử S1 (PRD mục 14 R01), `probes/s1-yaml/`, báo cáo `plans/reports/probe-s1-yaml-261102-1317.md`. Số note mà index thấy khác Python:

  | Ứng viên | kg-solo (746) | 58 ca biên | 25.000 note ngẫu nhiên | Bản B |
  |---|---|---|---|---|
  | `unsafe-libyaml` + lớp | 0 | 0 | 0 | chờ user |
  | `libyaml-safer` + lớp | 0 | 1 | 242 (đều là panic) | |
  | `saphyr-parser` + lớp | 0 | 9 | 6.510 | |
  | `serde_norway` dùng thẳng | 0 | 35 | 20.916 | |
  | `saphyr` dùng thẳng | 0 | 36 | 21.317 | |

  kg-solo không có dạng YAML rủi ro nào (chỉ chuỗi, số nguyên thường, danh sách; ngày được cả hai bên giữ là chuỗi), nên mọi ứng viên đều 0 lệch ở đó; bảng lệch theo loại và cách xử lý ở mục 3 của báo cáo. Bốn dạng làm bản Python dừng hẳn (`!!bool` chữ lạ, `!!timestamp` sai dạng, `!!int ""`, alias tự lồng) để phase 08 chốt cách xử lý.

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
