---
title: "Repo sản phẩm: preset bản B, bố cục apps/packages, pkm setup, plugin"
description: "Biến obsidian-pkm thành repo sản phẩm đủ đồ để dựng bản B trên máy khách: chuyển preset và hướng dẫn, đổi crates/ sang apps/packages, pkm setup, khung plugin, chuyển phần tính E11 vào pkm-core."
status: in-progress
priority: P1
effort: "khoảng 15-18 ngày công (ước theo E12.7 mục 3 của repo nghiên cứu, chưa đo)"
branch: main
tags: [infra, rust, plugin, installer, layout]
blockedBy: []
blocks: []
created: 2026-11-03
---

# Repo sản phẩm: preset bản B, bố cục apps/packages, `pkm setup`, plugin

## Tổng quan

Quyết định ở [docs/DECISIONS.md](../../docs/DECISIONS.md), ba mục ngày 2026-11-03 (tách vai hai repo; cài đặt là `pkm setup`; bố cục `apps/` + `packages/`). Plan này chỉ xếp việc; lý do nằm ở đó, không chép lại.

Trạng thái (2026-11-04): user giao làm mọi bước làm được ngay. Bước 1, 2 xong; bước 4, 5, 6 đang làm (xem "Thực hiện" dưới). Riêng bước 3 (đổi thư mục) đợi `phase-17-one-door` merge vào `main`. Tuần chạy song song (phase 16 của [kế hoạch tổng](../261102-0535-rust-core-port-master/plan.md)) vẫn chạy để user duyệt trước `v0.1.0`; plan này không chặn nó.

Từ 2026-11-03 repo này là nguồn sự thật của lõi; spec ghim `da62ecb` là lần cuối. Script Python của repo nghiên cứu chạy nguyên trạng cho bản B tới khi được chuyển. Muốn đổi hành vi script nào thì chuyển nó sang đây trước.

## Thực hiện (2026-11-04)

Nguồn chuyển: repo nghiên cứu `vantt/obsidian-pkm-automation` ở `9061581` (`scripts/e11/`, bảng việc ở `docs/plans/E12.7-port-rust-js.md` mục 3), fork OHS `vantt/obsidian-hybrid-search` ở `327c995` (đặc tả bản vá `vantt/patches/0003-vector-export.spec.md`, `0004-stdio-parity.spec.md` cho `vectors`, `neighbors`). Gói OHS ghim vẫn là `vantt.4` (đã có P03, P04).

Thứ tự đổi so với bảng phase: bước 4, 5, 6 không đợi bước 3. Chúng viết ở bố cục hiện tại (`crates/`, plugin ở `apps/obsidian-plugin/` là thư mục mới) trên nhánh `product-b`, nhánh này xếp chồng trên `phase-17-one-door` vì cần MCP một cửa, `search` và bộ cài mới. Bước 3 (đổi `crates/` sang `apps/` + `packages/`) vẫn đợi `phase-17-one-door` merge như quyết định 2026-11-03; `git mv` mang theo mã mới. Bước 2 không đụng `crates/` nên làm thẳng trên `main`.

| Bước | Nhánh | Đợt | Trạng thái |
|---|---|---|---|
| 2 preset bản B, hướng dẫn người dùng | `main` | 1 | xong (`6b7e48e`, `3c7dbf8`) |
| 6a `propose` vào `pkm-core` | `product-b` | 1 | xong (`14c024b`, [báo cáo](../reports/fullstack-developer-261104-1100-propose-port.md)): `pkm VAULT propose [--similar] [--create] [--json]`, khớp Python từng byte trên vault bịa và kg-solo (60 đề xuất). Còn: đếm trên bản B |
| 6b `material`, `zones`, cặp note giống nhau (thay `note_knn.mjs`, `similar_pairs.mjs`) qua `vectors`, `neighbors` của OHS | `product-b` | 1 | xong (`63d67e1`, [báo cáo](../reports/fullstack-developer-261104-1100-material-zones-port.md)): `pkm VAULT material`, `pairs`, `zones`; kg-solo với gói OHS thật: `zones.json` giống từng byte, `pairs` cùng tập và điểm, `material` khớp. Còn: golden kg-solo cho ba lệnh (cần OHS giả phát lại vector), đếm trên bản B |
| 5a khung plugin, `workshop_ui`, review log, CSS lưới thư mục, phóng to bảng | `product-b` | 1 | xong (`8a2425d`, [báo cáo](../reports/fullstack-developer-261104-1100-obsidian-plugin.md)): "Dựng xưởng" giống `workshop_ui.py` từng byte trên vault bịa; thư mục chủ đề, inbox là thiết lập (mặc định rỗng). Còn: thử trong Obsidian thật |
| 4 `pkm setup`, plugin trong Release | `product-b-setup` (từ `product-b`) | 2 (cần 2, 5a) | viết xong, đã merge vào `product-b` (`e5f1b1e`, `d78b154`, `e93a34d`; [báo cáo](../reports/fullstack-developer-261104-1130-pkm-setup.md)). Còn: chạy thử Release (`workflow_dispatch`) ra `pkm-obsidian-plugin.zip`, thử trên máy sạch ba nền, user thử trên Windows |
| 5b giao diện plugin cho đề xuất, nguyên liệu, vùng | `product-b` | 2 (cần 5a, 6) | xong (`4a1b5d6`, [báo cáo](../reports/fullstack-developer-261104-1130-plugin-writing-ui.md)): duyệt, áp dụng, hoàn tác, đo duyệt; trang nguyên liệu ghi `uses`; màn hình vùng không đè dòng chấm. Note đề xuất dặn lệnh áp dụng của plugin (`4b367e5`). Còn: thử trong Obsidian thật với `pkm` đã dựng |
| 3 đổi thư mục, `pkm-contract` | sau merge | 3 | chờ `phase-17-one-door` merge |
| 4b theme ghim; bản B không cần Claude Desktop | `product-b` | 2 | xong (quyết định user 2026-11-04, [DECISIONS](../../docs/DECISIONS.md)): Primary 2.10.0 và Style Settings 1.0.9 tải ghim sha256; thiếu Claude Code thì `--apply` dừng; không có Claude Desktop thì chỉ dựng index cho Claude Code, `pkm doctor` chỉ cảnh báo. Không đưa plugin lên kho cộng đồng |
| License khi bán | — | — | đã rà ([báo cáo](../reports/researcher-261104-1416-license-review.md)). Còn: LICENSE/EULA cho repo, `THIRD_PARTY_NOTICES` trong Release (Templater AGPL-3.0; Style Settings, Primary GPL-3.0; font OFL), hỏi Anthropic về đăng nhập gói Pro/Max qua Agent SDK trong sản phẩm bán |

Không làm ở đây (E12.7 mục 3.5, 3.6): `migrate_v3`, `selection_signals`, `board.mjs`, Apps Script; nhập Evernote (`normalize`, `highlight_colours`) chờ user quyết có bán không. Điện thoại (`pkm serve`) không thuộc plan này.

## Kết quả cuối (Goal)

Một tag ra cả binary `pkm` và plugin Obsidian cùng phiên bản. Từ một máy sạch, khách chạy bộ cài rồi `pkm setup VAULT` là có bản B đủ: schema, adapter, mẫu `CLAUDE.md`, template, theme, plugin, MCP, index. Không cần gì từ repo nghiên cứu.

## Bố cục đích

```
apps/
  pkm/               Rust bin: CLI, MCP, doctor, setup, serve
  obsidian-plugin/   TS: giao diện, hành vi trong Obsidian
  setup-gui/         (sau) Tauri, vỏ mỏng gọi `pkm setup --json`
packages/
  pkm-core/          Rust lib: vault, index, graph, related, propose, zones, OHS client
  pkm-contract/      kiểu JSON chung, sinh từ Rust (schemars/ts-rs) ra TS cho plugin và GUI
presets/
  b/                 schema, adapter, CLAUDE.md, template, theme (dữ liệu)
tools/
  parity/, probes/   bộ so khớp, probe (không giao khách)
Cargo.toml           workspace: apps/pkm, packages/pkm-core, tools/parity
pnpm-workspace.yaml  apps/obsidian-plugin, packages/pkm-contract
```

Hiện tại: `crates/pkm`, `crates/pkm-core`, `parity/`, `probes/` ở gốc.

## Các phase

| # | Phase | Cần | Công (ước) |
|---|---|---|---|
| 1 | Ghi quyết định | — | xong |
| 2 | Chuyển preset bản B và hướng dẫn người dùng | user giao | 1–1,5 |
| 3 | Đổi thư mục sang apps/packages, dựng khung plugin | `phase-17-one-door` merge, CI xanh | 2,5 |
| 4 | `pkm setup` thay `setup_vault.py` | 2, 3 | 3 |
| 5 | Plugin: `workshop_ui` trước | 3 | 3,5 |
| 6 | `propose`, `material`, `zones` vào `pkm-core` | 3 | 6 |

Phase 4, 5, 6 chạy song song được sau phase 3. Số công lấy từ bảng E12.7 mục 3 của repo nghiên cứu; chưa đo trên repo này.

### Phase 1: ghi quyết định
Ba mục trong `docs/DECISIONS.md` và mục GATES 2026-11-03 ở repo nghiên cứu. Xong khi cả hai đã vào nhánh chính của mỗi repo.

### Phase 2: chuyển preset bản B và hướng dẫn người dùng
Chép sang `presets/b/` (thư mục mới, không đụng `crates/`, nên không phải đợi phase 3): mẫu `CLAUDE.md` (`scripts/e11/vault-agent.md`, giữ tên `vault-agent.md`), CSS lưới thư mục, theme (nguồn, font OFL, bản đã dựng), `setup.json` (plugin ghim kèm sha256, adapter ACP, cờ MCP, giá trị theme). Chuyển phần playbook dành cho khách thành hướng dẫn người dùng trong `docs/user-guide/`; phần dành cho người nghiên cứu ở lại repo nghiên cứu. Ghi rõ nguồn mỗi file (commit của repo nghiên cứu). Xong khi `presets/b/` đủ để dựng bản B mà không đọc gì từ repo nghiên cứu, và rà bằng grep không thấy nội dung vault thật.

Kết quả (2026-11-04, nguồn `9061581`; chi tiết ở [presets/b/README.md](../../presets/b/README.md)): `schema.yaml` không chép vì file nhúng của `pkm-core` giống từng byte bản của repo nghiên cứu, file nhúng là gốc. Adapter e11 là mã Rust (`adapter/e11.rs`), không có phần dữ liệu. Trang chủ, file `.base`, trang chủ đề và 8 mẫu note là chuỗi trong `workshop_ui.py`, chưa phải file: ra file khi phase 5 chuyển `workshop_ui`. Plugin `e11-review-log`, `e11-table-zoom` là mã JS, gộp vào plugin ở phase 5. Còn mở cho phase 4, 5: theme Primary 2.10.0 và plugin Style Settings chưa script nào cài; `TOPIC_ROOTS`, `INBOXES` của `workshop_ui.py` ghi cứng thư mục của vault nghiên cứu, máy khách cần cấu hình.

### Phase 3: đổi thư mục, dựng khung plugin
Một PR: `git mv crates/pkm apps/pkm`, `git mv crates/pkm-core packages/pkm-core`, `git mv parity tools/parity`, `git mv probes tools/probes`; sửa `Cargo.toml` của workspace, đường dẫn trong `.github/workflows/ci.yml`, `release.yml`, `scripts/release-assets.sh`, `scripts/install.sh`, bộ so khớp (`tools/parity/src/*`, `parity/golden/`), `include_str!` (ví dụ `schema/schema.yaml` của `pkm-core`), README, `docs/`, `spec/PORTING.md`. Cùng PR thêm `pnpm-workspace.yaml`, khung `apps/obsidian-plugin` (manifest, settings, lớp gọi `pkm … --json`) và `packages/pkm-contract` sinh kiểu TS từ Rust. Release ra thêm plugin cùng tag. Chỉ mở PR khi không còn nhánh nào chờ merge. Xong khi CI xanh ba nền (PR chỉ chạy Linux và Windows; chạy tay ma trận đủ có macOS bằng `gh workflow run ci.yml --ref <nhánh>`), so khớp kg-solo 0 lệch chưa có tên, chạy thử Release (`workflow_dispatch`) ra đủ file và checksums khớp.

### Phase 4: `pkm setup` thay `setup_vault.py`
`pkm setup VAULT` dựng bản B từ `presets/b/`: chép preset, tải plugin ghim phiên bản và kiểm sha256, bật plugin, dựng index, tải gói OHS và index lần đầu, cài MCP (dùng lại `mcp install`, `doctor`). Có `--json` cho vỏ GUI sau này. Không đè file người dùng đã sửa. Xong khi trên máy sạch Windows, macOS, Linux, `pkm setup` trên vault fixture ra bản dùng được, `doctor` không có `fail`, và user chạy thử trên Windows.

### Phase 5: plugin, `workshop_ui` trước
Chuyển lệnh "Dựng xưởng" (`workshop_ui`) vào plugin: Trang chủ, file `.base`, mẫu, CSS, trang chủ đề, mẫu dạng file tĩnh. Gộp review log và CSS lưới thư mục. Ghi vào note đang mở qua API Obsidian. Xong khi plugin dựng xưởng trên vault fixture ra kết quả như bản Python (so bằng Obsidian CLI), và có mục playbook.

### Phase 6: `propose`, `material`, `zones` vào `pkm-core`
Phần tính chuyển sang Rust, ra JSON qua `pkm … --json`; phần bấm (nhận, bỏ, dựng trang nguyên liệu, màn hình vùng) vào plugin. Hành vi bản Python là điểm xuất phát, không còn là đặc tả: lệch có lý do thì ghi trong `spec/PORTING.md`. Xong khi mỗi lệnh có test và golden kg-solo riêng ở đây, và chạy trên bản B qua lệnh riêng tư chỉ in số.

## Rủi ro

- **Đường dẫn sau khi đổi thư mục:** CI, `release.yml`, `scripts/release-assets.sh` (đọc `cargo pkgid`), `scripts/install.sh`, bộ so khớp, golden, `include_str!` đều có đường dẫn cứng. Sót một chỗ thì Release hỏng âm thầm. Giảm bằng grep `crates/`, `parity/`, `probes/` trước khi mở PR và chạy thử Release.
- **Xung đột nhánh:** đổi thư mục khi còn nhánh chờ merge thì nhánh đó phải rebase qua `git mv`. Chỉ làm khi không còn nhánh nào chờ.
- **Preset bản B lẫn nội dung vault thật:** `presets/b/` chỉ chứa schema, mẫu, theme, không chứa tên note, đường dẫn note, nội dung hay đường dẫn vault của user. Rà bằng grep trước mỗi commit chạm `presets/`.
- **Hai bản `schema.yaml`:** đã chốt ở phase 2: chỉ một bản, file nhúng của `pkm-core`; `presets/b/` không có bản sao.
- **Plugin và GUI không thành hình sau `v0.1.0`:** khi đó giữ `crates/` và `plugin/` cũng được (DECISIONS, mục bố cục).

<!-- slug: product-layout -->
