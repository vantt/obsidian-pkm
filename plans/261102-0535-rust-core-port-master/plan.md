---
title: "Kế hoạch tổng: chuyển lõi kg sang Rust (pkm)"
description: "Bản đồ việc cho agent lead: mọi hạng mục R00–R14 của docs/PRD.md, xếp theo đợt chạy song song, có cổng người dùng và tiêu chí xong đo được."
status: in-progress
priority: P1
effort: "20-24 ngày công + 1 tuần chạy song song"
branch: main
tags: [feature, infra, rust, mcp, parity]
blockedBy: []
blocks: []
created: 2026-11-02
---

# Kế hoạch tổng: chuyển lõi kg sang Rust (`pkm`)

## Tổng quan

Kế hoạch này là đầu vào cho một goal. Agent lead đọc nó để biết phải giao gì, theo thứ tự nào, cái gì chạy song song được, và khi nào thì dừng chờ user. Nội dung kỹ thuật nằm ở [docs/PRD.md](../../docs/PRD.md); plan này không chép lại PRD mà chỉ trỏ tới mục cần đọc. Mỗi phase là một gói việc giao cho một agent con: agent con chạy `/ak-plan --fast <file phase>` để ra plan chi tiết, rồi `/ak-cook` plan đó.

Quyết định đã chốt (D1–D10, Q1, Q5 trong PRD) là ràng buộc, không bàn lại. Q3 (crate YAML) và Q4 (`rmcp` hay tự viết) do phase 02 và 03 chốt. Q2 (giữ tên `kg`) hỏi user ở phase 17.

## Kết quả cuối (Goal)

Goal xong khi đủ bốn mốc. Mỗi mốc có tiêu chí đo được; lead kiểm bằng lệnh, không tin báo cáo suông.

| Mốc | Gồm phase | Xong khi |
|---|---|---|
| M1 Chuẩn bị | 01–07, 14 | CI xanh trên Windows, macOS, Linux; `docs/DECISIONS.md` chốt Q3, Q4; bộ so khớp chạy Python với Python ra 0 lệch; `parity/golden/<sha ghim>/` có đủ; `follow` liệt kê đúng commit |
| M2 Lõi khớp | 08–13 | CI so Rust với golden kg-solo: 0 lệch chưa ghi tên; so khớp bản B trên máy user: 0 lệch chưa ghi tên; 15 test kg, 13 test MCP, 19 test platform đã chuyển đều qua; `pkm mcp install` chạy trên Windows sạch và Claude Desktop gọi được tool (user xác nhận) |
| M3 Chuyển hẳn | 15, 16 | `follow` báo 0 commit chưa chuyển; đủ bốn điều kiện mục 13 PRD; user đồng ý |
| M4 Một cửa | 17 | Agent làm đủ nhóm A–D playbook chỉ với `pkm`; số lượt gọi tool và token mỗi câu không tăng (có số ghi lại) |

Goal chỉ được coi là xong ở M4. Khi gặp cổng người dùng chưa mở, goal dừng ở trạng thái **Blocked: chờ user** kèm danh sách việc user cần làm, không làm vòng.

## Đợt chạy và phụ thuộc

```
W0  P01 khung workspace
     │
W1  P02 YAML(S1) · P03 MCP(S2) · P04 OHS(S3) · P05 so khớp+golden · P06 follow · P07 platform · P14 release   ← 7 việc song song
     │
G0  Cổng bắt đầu viết lõi (user, mục 13 PRD)  +  C0 đuổi theo lần 0 (nâng ghim, dựng lại golden)
     │
W2  P08 vault+schema                (cần P02, P05, G0)
W3  P09 index+adapter+validate      (cần P08)
W4  P10 query+tools+more            (cần P09)   → C1 đuổi theo
W5  P11 related+ohs  ∥  P12 mcp+CLI (P11 cần P10, P04; P12 cần P10, P03)
W6  P13 install+doctor              (cần P12, P07)   → C2 đuổi theo
W7  P15 đuổi theo lần cuối → P16 chạy song song 1 tuần, chuyển hẳn (cần P13, P14, P15)
W8  P17 một cửa                     (cần P16)
```

Đường găng: P01 → P02 → P08 → P09 → P10 → P12 → P13 → P16 → P17. Mọi phase không nằm trên đường này nên chạy song song ngay khi đủ điều kiện.

P15 (đuổi theo Python) không phải một đợt mà là việc lặp ở các điểm C0–C3; xem [phase 15](./phase-15-chase-python-commits.md).

## Các phase

| # | Phase | PRD | Đợt | Cần | Công | Cần user | Model gợi ý |
|---|---|---|---|---|---|---|---|
| 1 | [Khung workspace](./phase-01-workspace-skeleton.md) | R00 | W0 | — | 0,5 | Không | sonnet |
| 2 | [Bài thử S1: YAML](./phase-02-spec-probe-yaml.md) | R01 | W1 | 01 | 1 | Đường dẫn bản B, chạy trên máy user | sonnet |
| 3 | [Bài thử S2: MCP](./phase-03-spec-probe-mcp.md) | R02 | W1 | 01 | 0,5 | Thử Claude Desktop trên Windows | sonnet |
| 4 | [Bài thử S3: OHS stdio](./phase-04-spec-probe-ohs.md) | R03 | W1 | 01 | 0,5 | Không | sonnet |
| 5 | [Bộ so khớp và golden](./phase-05-parity-harness-and-golden.md) | R04, R05 (golden) | W1 | 01 | 2,5 | Chạy bản B | sonnet |
| 6 | [Lệnh follow](./phase-06-follow-command.md) | R05 (follow) | W1 | 01 | 0,25 | Không | haiku |
| 7 | [Module platform](./phase-07-platform-module.md) | R11 (platform) | W1 | 01 | 0,5 | Không | sonnet |
| 8 | [vault và schema](./phase-08-vault-and-schema.md) | R06 | W2 | 02, 05, G0 | 2,5 | Chạy bản B | sonnet, gọi kongming khi kẹt YAML |
| 9 | [index, adapter e11, validate](./phase-09-index-adapter-validate.md) | R07 | W3 | 08 | 3 | Chạy bản B | sonnet |
| 10 | [query, tools, more](./phase-10-query-tools-more.md) | R08 | W4 | 09 | 2,5 | Chạy bản B | sonnet |
| 11 | [related và module OHS](./phase-11-related-and-ohs-module.md) | R09 | W5 | 10, 04 | 1,5 | Không | sonnet |
| 12 | [MCP server và CLI](./phase-12-mcp-server-and-cli.md) | R10 | W5 | 10, 03 | 2 | Không | sonnet |
| 13 | [pkm mcp install và doctor](./phase-13-install-and-doctor.md) | R11 | W6 | 12, 07 | 1 | Thử Windows sạch | sonnet |
| 14 | [Workflow release](./phase-14-release-workflow.md) | mục 12 | W1 | 01 | 0,25 | Không | haiku |
| 15 | [Đuổi theo commit Python](./phase-15-chase-python-commits.md) | R12 | C0–C3 | 06 | 10–20% | User bảo khi nào chạy | sonnet |
| 16 | [Chạy song song và chuyển hẳn](./phase-16-parallel-run-and-cutover.md) | R13 | W7 | 13, 14, 15 | 0,5 + 1 tuần | Có, nhiều | lead tự làm |
| 17 | [Một cửa](./phase-17-one-door-mcp.md) | R14 | W8 | 16 | 1,5 | Q2, sửa repo đặc tả | sonnet |

Phase 07 tách phần `platform` khỏi R11 vì module này độc lập và có sẵn 19 test, làm sớm thì rút ngắn W6. Golden kg-solo (R05) chuyển vào phase 05 vì golden phải ở đúng định dạng dump mà bộ so khớp dùng.

## Cổng người dùng

Lead gom các việc cần user thành một danh sách, hỏi một lần cho mỗi đợt, không hỏi lắt nhắt.

| Cổng | Khi nào | User làm gì |
|---|---|---|
| G0 | Trước P08 | Xác nhận đã thử xong E12.5 và một tuần không đổi `schema.yaml` lẫn hợp đồng tool MCP (mục 13 PRD); cho phép ghi mục GATES ở repo đặc tả |
| Bản B | P02, P05, P08–P10, trước P16 | Cho đường dẫn vault bản B; agent chạy `parity --private` trên máy user, chỉ in số lệch |
| Windows | P03, P13 | Chạy thử Claude Desktop trên Windows và báo kết quả |
| C0–C3 | Các điểm đuổi theo | Đồng ý chạy `follow` và chuyển commit (PRD mục 5.3: làm tay khi user bảo) |
| Chuyển hẳn | P16 | Dùng một tuần, chạy nhóm A–C, F playbook, đồng ý chuyển hẳn |
| Q2, repo đặc tả | P16, P17 | Quyết giữ tên `kg`; cho phép sửa `setup_vault.py`, hướng dẫn cài, GATES, `CLAUDE.md` mẫu ở repo đặc tả |

## Cách lead điều phối

1. **Chọn việc:** phase sẵn sàng là phase có mọi phụ thuộc đã merge và cổng (nếu có) đã mở. Giao mọi phase sẵn sàng cùng lúc, mỗi phase một agent con, mỗi agent một worktree riêng.
2. **Lời giao việc** (theo `.claude/rules/orchestration-protocol.md`): đường dẫn file phase; các mục PRD trong phần "Đọc trước"; file được sửa; tiêu chí nghiệm thu; ràng buộc D1–D10; thư mục làm việc `/home/vantt/projects/obsidian-pkm`, múi giờ Asia/Saigon, viết tài liệu bằng tiếng Việt như PRD; báo cáo vào `plans/reports/`. Yêu cầu agent con chạy `/ak-plan --fast <file phase>` rồi `/ak-cook <plan con>` và kết thúc bằng `Status: DONE | DONE_WITH_CONCERNS | BLOCKED | NEEDS_CONTEXT`.
3. **Nghiệm thu:** lead tự chạy lệnh ở mục "Nghiệm thu" của phase. Phase 08–13 qua thêm một lượt `code-reviewer` trước khi merge.
4. **Merge:** merge theo thứ tự đợt. Hai phase song song cùng sửa `crates/pkm-core/Cargo.toml`, `docs/DECISIONS.md` hoặc `spec/PORTING.md` thì lead merge lần lượt và tự gỡ xung đột; các file này chỉ được thêm mục, không sửa mục của phase khác.
5. **Cập nhật trạng thái:** đổi `status` của phase, ghi `spec/PORTING.md`, cập nhật dòng "Trạng thái" trong `README.md` ở mỗi mốc.
6. **Khi kẹt:** `BLOCKED` hoặc `NEEDS_CONTEXT` thì đổi ngữ cảnh hay phạm vi, không giao lại y nguyên. Hai lần thất bại cùng một việc, hoặc lệch so khớp không giải thích được, thì gọi `kongming` kèm bằng chứng.

## Quy tắc chung cho mọi phase

- Hành vi Python ở sha ghim là đặc tả (PRD mục 4). Thấy Python sai rõ ràng: không sửa lặng lẽ, ghi "Lệch có chủ ý" trong `spec/PORTING.md`, bộ so khớp ghi ngoại lệ có tên, đề xuất issue ở repo đặc tả (mở issue cần user cho phép).
- Không đưa nội dung vault thật (bản B) vào repo, CI, log, báo cáo hay issue.
- CI không cần Python và không cần token (Q5).
- Mỗi phase xong phải có: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` qua.
- Commit theo conventional commit, không nhắc mã phase hay mã finding trong code, tên test hay commit.

## Rủi ro chính

Xem mục 16 PRD. Với lead, ba rủi ro cần theo dõi: YAML đọc khác PyYAML (chặn P08, giảm bằng P02); Python sửa nhanh hơn Rust đuổi (đo bằng `follow`, mục tiêu dưới 5 commit và dưới 7 ngày); mô tả tool MCP lệch câu chữ (so chuỗi ở tầng MCP của bộ so khớp).

<!-- slug: rust-core-port-master -->

## Trạng thái thực hiện (2026-11-03)

Mã của phase 01–14 đã xong tới bộ cài. Đã phát hành thử `v0.1.0-rc.4` từ nhánh `phase-17-one-door` (commit a734727, run 41625393860: `doctor` kiểm Visual C++ Redistributable của gói OHS trên Windows (`ohs_runtime`), `model_missing` nêu DLL thiếu, `serverInfo.version` là phiên bản binary) để máy Windows thử trên Windows Sandbox; kết quả 2026-11-04 trên Sandbox sạch: đạt cả ba tiêu chí (trước khi cài VC++ `doctor` báo `ohs_runtime` warn nêu ba DLL kèm lệnh `winget`, `mcp install` in cùng cảnh báo khi làm ấm, `related` trả `model_missing` với action nêu DLL trước câu của OHS; sau `vc_redist` `ohs_runtime` ok và `related` chạy sau 21 giây; cài 9 s, `mcp install` 33 s). Trước đó `v0.1.0-rc.3` từ nhánh `phase-17-one-door` (commit 535bc55, run 41592247657: MCP một cửa `pkm`, 13 tool; 7 file, checksums khớp) để thử một cửa ngay trong tuần chạy song song; nhánh chưa merge vào main. Trước đó `v0.1.0-rc.2` (pre-release, commit a28c842, run 41477178052: OHS ghim vantt.4 không hạ cấp, `--ohs DIR`, spec ghim da62ecb, các sửa từ báo cáo Windows; 7 file, checksums khớp). Trước đó `v0.1.0-rc.1` (pre-release, commit b8995f7, run 41424573303): đủ 7 file, `sha256sum -c checksums.txt` qua hết trên bản tải về. Trước đó chạy thử Release bằng `workflow_dispatch` (run 41417342596, commit 93d1251) ra đủ 7 file, `sha256sum -c checksums.txt` qua hết, binary Windows không cần `vcruntime140.dll`, `install.ps1` và `install.sh` qua bài thử trên runner; CI xanh trên ba nền, bộ so khớp kg-solo 64 mục, 0 lệch, 0 mục chưa có. Agent Windows đã cài `v0.1.0-rc.1` trên Windows 11 thật và chạy bản B theo [handoff-windows-test.md](./handoff-windows-test.md) ([báo cáo](../reports/windows-tester-261102-1816-windows-install.md)): phase 02, 03, 05, 08, 09, 11 qua; 10 và 13 còn một việc nhỏ.

Thử `v0.1.0-rc.3` trên Windows 2026-11-03 ([báo cáo](../reports/windows-tester-261103-1430-rc3-windows.md)): phase 10 trên bản B (spec `da62ecb`) 54 mục, 0 lệch chưa có tên, 0 mục chưa có, hai lệch CLI của rc.1 đã hết. Phase 13 trên Windows Sandbox (không Python, không VC++ runtime): cài, `mcp install` (tải OHS 26 s), `--check` 13 tool, `doctor` 0 lỗi; `search`/`related` báo `model_missing` tới khi cài Visual C++ Redistributable vì `onnxruntime-node` của gói OHS cần nó (sửa gốc ở fork và `doctor`, xem báo cáo). User đã cài rc.3 và dùng hằng ngày: Claude Desktop và chat trong Obsidian của bản B chạy một MCP `pkm` (`--ohs C:\tools\ohs`), `doctor` 0 lỗi 0 cảnh báo; repo đặc tả đã đổi `CLAUDE.md` mẫu, `setup_vault.py` và hướng dẫn cài sang `pkm` (commit `04eb84f`, `d7351b4`).

| Nhóm | Phase | Còn lại |
|---|---|---|
| Xong | 01, 02, 03, 04, 05, 06, 07, 08, 09, 10, 11, 12, 14 | — |
| Xong, còn việc ở fork | 13 | Windows sạch qua (Sandbox, rc.3); tìm theo nghĩa cần VC++ Redistributable. Phía `pkm` đã xong trên nhánh `phase-17-one-door` (2026-11-03): `doctor` có mục `ohs_runtime` kèm lệnh `winget`, `ohs_not_ready` khi `model_missing` nêu DLL thiếu. Còn ở fork OHS: chép runtime vào gói win32-x64 |
| Đang làm, theo lượt | 15 | hai lượt đuổi theo 2026-11-02: lượt 1 chuyển `223c9c3`; lượt 2 xử lý `24f193a`, `58010ad`, `8f89403`, `6af65b1`, `9f0700b`, ghim `da62ecb`, dựng lại golden; `follow` báo 0. Lượt sau khi user bảo |
| Đang chạy | 16 | tuần chạy song song từ 2026-11-03 với rc.3; user chấm playbook nhóm A–C, F; cuối tuần user duyệt chuyển hẳn |
| Đang chạy | 17 | mã xong ở nhánh `phase-17-one-door` (rc.3); repo đặc tả đã đổi sang `pkm`; còn đo lượt gọi tool và token câu A–D (cùng tuần), rồi merge vào main và ra `v0.1.0` |
