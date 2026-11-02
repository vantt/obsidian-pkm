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
