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
- **Ngày:** 2026-11-02
- **Lựa chọn:** JSON-RPC 2.0 stdio tự viết (mỗi dòng một bản tin), đồng bộ, chỉ dùng `serde_json` (bật `preserve_order`). Không dùng `rmcp`, không dùng async runtime.
- **Lý do:**
  - Kích thước và phụ thuộc: cùng một tool, bản tự viết 0,68 MB, 12 crate, build release từ sạch 2,5 s; bản `rmcp` 3.8.0 2,79 MB, 56 crate, 16,5 s, và bắt buộc `tokio` (cần cả `time`, thiếu là panic lúc chạy). PRD mục 3 nói không dùng async runtime nếu không cần.
  - Câu chữ: cả hai đều cho kiểm soát đủ `instructions`, mô tả tool, schema tham số (mục `find` giống hệt Python, kể cả thứ tự khoá), và phiên bản giao thức (`rmcp` phải ghi đè `supported_protocol_versions`).
  - Hợp đồng MCP phải giống Python (PRD mục 8), mà ở tầng giao thức `rmcp` khác Python ở năm chỗ: câu báo phương thức lạ (`nope/x` thay vì `method not found: nope/x`), dòng không phải JSON bị bỏ im lặng (Python trả `-32700`), batch bị trả `-32600` không có `id` (Python trả lời từng phần), request trước `initialize` bị từ chối, `server/discover` trả `-32022` (Python `-32601`). Bản tự viết khớp Python ở cả năm chỗ với khoảng 100 dòng, chép thẳng `Server.handle` của Python.
  - `rmcp` đổi nhanh (bản 3.x, đã theo giao thức 2026-07-28); bám nó là thêm một nguồn thay đổi ngoài Python cần đuổi theo.
  - Xem lại khi Python chuyển sang giao thức 2026-07-28 (bỏ `initialize`, thêm `server/discover`) hoặc khi cần MCP qua HTTP.
- **Bằng chứng:** `probes/s2-mcp/` (hai binary `s2-hand`, `s2-rmcp`, test `tests/stdio_session.rs` so mục `find` với bản Python); bản ghi phiên Claude Code 2.1.316 và số đo ở `plans/reports/probe-s2-mcp-261102-1317.md`.

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

## Module `platform`: `caseless`, `std::env::home_dir`
- **Ngày:** 2026-11-02
- **Lựa chọn:** so tên không phân biệt hoa thường bằng `caseless` (khớp `str.casefold` của Python), chuẩn hoá NFC/NFD bằng `unicode-normalization`; thư mục nhà lấy từ `std::env::home_dir` thay vì crate `dirs`; Windows dùng `MoveFileExW` không cờ thay thế (`windows-sys`).
- **Lý do:** `to_lowercase` không giống `casefold` với một số ký tự; `home_dir` đã đúng trên ba nền từ Rust 1.86 nên không cần thêm crate; `std::fs::rename` trên Windows đè đích, khác `os.rename`.
- **Bằng chứng:** `crates/pkm-core/src/platform.rs`, bảng test platform trong `spec/PORTING.md`.
