# Port `material`, `zones`, `similar_pairs`/`note_knn` vào `pkm-core` (bước 6 của plan 261103-product-layout)

Nhánh `product-b-material` (từ `origin/product-b` @ `561aafe`), commit code `63d67e1`.

## Kết quả

- Client OHS có thêm `Ohs::vectors` và `Ohs::neighbors` (action stdio `vectors`, `neighbors` của fork), cùng cổng `ready`, cùng lỗi `ohs_not_ready`, không đường lùi. Phản hồi là `VectorReply { meta, records }`, đường dẫn (cả `neighbors[].path`) chuẩn hoá NFC. `fake-ohs` trả hai action này từ cấu hình `vectors`.
- Module mới: `pkm_core::material`, `pkm_core::zones`, `pkm_core::pairs` (lọc note, K láng giềng, cặp gần nghĩa từ vector OHS).
- CLI: `pkm VAULT material DRAFT`, `pkm VAULT pairs`, `pkm VAULT zones` (chi tiết: `pkm VAULT material --help`). Cả ba in JSON ra stdout; OHS thiếu thì in `ohs_not_ready` (đủ năm khoá) hoặc `{"error":"ohs_error","detail"}` và thoát 1.
- Lệch có chủ ý và bảng test: mục mới cuối `spec/PORTING.md`.

## Đối chiếu với bản Python (kg-solo, gói OHS thật `0.17.2-vantt.4`, Linux)

- `zones`: `.e11/zones.json` giống từng byte file của `zones.py` (đầu vào từ `note_knn.mjs --k 15`, cùng index), 33 vùng; trang chỉ khác dòng `generated_by`.
- `pairs`: 1000 cặp (`--min 0.9 --per-note 100 --min-words 0`): cùng tập cặp, cùng điểm. Với tham số mặc định, file giống từng byte. Chiều `a`/`b` và thứ tự cặp cùng điểm có thể khác vì thứ tự note là thứ tự OHS trả.
- `material`: giống hết `items`, `missing`, `edges`, `topics`, `queries`, `draft_seed` trên hai bản nháp (chỉ hit cho sẵn; ba câu tìm qua OHS thật). Chỉ khác khoá trong `methods`.

## File

- `crates/pkm-core/src/ohs.rs`: `vectors`, `neighbors`, `VectorReply`, tách `call_ready` và `request_options`.
- `crates/pkm-core/src/{material,zones,pairs}.rs` cùng `*/tests.rs`; `crates/pkm-core/src/lib.rs` (ba dòng module).
- `crates/pkm-core/tests/support/fake_ohs.rs`, `crates/pkm-core/tests/ohs_fake.rs` (3 test mới), `crates/pkm-core/tests/ohs_vectors_real.rs` (mới, `#[ignore]`, đã chạy qua).
- `crates/pkm/src/cli.rs`: một dòng rẽ nhánh trong `run()`, cộng một khối `mod writing` riêng ở cuối file, trước `mod tests`. Không đụng `OPS`, `VaultArgs`, `USAGE`, nên ít đụng lệnh `propose` của agent kia.
- `crates/pkm/tests/cli_writing.rs` (mới, 5 test với OHS giả).
- `spec/PORTING.md`.

## Kiểm tra

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`: đều qua.
- Test với gói OHS thật: `PKM_OHS_DIR=<gói> PKM_OHS_TEST_VAULT=<kg-solo> cargo test -p pkm-core --test ohs_vectors_real -- --ignored` qua.
- Không còn tiến trình `node` hay `fake-ohs` nào sau khi chạy (đã kiểm bằng `pgrep`).

## Hợp đồng JSON cho plugin

Lỗi chung của cả ba lệnh: thoát 1, stdout là `{"error":"ohs_not_ready","state","progress","eta_s","action"}` hoặc `{"error":"ohs_error","detail"}`. Lỗi khác in `pkm: <lệnh>: <lý do>` ra stderr và thoát 1.

### `pkm VAULT material "Bài viết/X" [--k 12] [--per-query 10] [--hits FILE] [--db] [--no-refresh] [--ohs] [--ohs-wait]`

```json
{
 "draft": "Bài viết/X.md",
 "queries": [["tên bài", "X"], ["<tiêu đề mục>", "<đoạn, ≤400 ký tự>"]],
 "methods": {"ohs-hits": 1, "ohs": 3},
 "items": [
  {"id": "A/b.md", "type": "note|null", "found_by": "ohs|graph|used", "rank": 0.0123, "used": false,
   "sections": ["<nhãn câu đã khớp>"], "ohs_score": 0.875,
   "layer": "accepted|proposed",
   "via": [{"from": "...", "rel": "...", "to": "...", "status": "accepted|proposed", "claim": "?",
            "proposed_by": "...|null", "score": 0.7, "origin": "...|null"}],
   "highlights": 2, "first_highlights": ["≤160 ký tự"], "wise": "?", "use": "?", "gist": "?", "why": "?"}
 ],
 "missing": ["hit không có trong index"],
 "edges": {"accepted": 1469, "proposed": 9},
 "draft_seed": true,
 "topics": [{"id": "Vườn/Vườn (chủ đề).md", "hits": 3}]
}
```

`sections` và `ohs_score` chỉ có ở note tìm theo chữ (`found_by: ohs`). `layer` và `via` chỉ có ở note graph đưa tới. Mục `found_by: used` không có `rank`. Thứ tự: note tìm theo chữ (tối đa `k`), rồi note đã dùng mà hai danh sách không có, rồi note graph đưa tới (tối đa `k`).

Phần plugin làm (bản Python: `render`, `item_lines`, `ticked_in`, `add_uses`, `apply_ticks`, `open_in_obsidian` trong `material.py`):
- trang `_Xưởng/Nguyên liệu/<tên bài>.md`, nhóm theo vai (bảng `ROLES`), mục "Đã dùng", mục "Graph đưa tới" kèm chuỗi bước (bước máy gợi ý in nghiêng, kèm `proposed_by` và `score`), mục "Cửa vào chủ đề", mục các đoạn đã tìm;
- ô `[x]` thành mục trong `uses` của bản nháp. Phải sửa được danh sách rỗng, danh sách một dòng, danh sách khối, CRLF; phần còn lại của file giữ nguyên từng byte. Không thêm trùng, không thêm chính bản nháp, chỉ thêm note có trong index;
- ghi `_E11/material-log.jsonl`, mỗi dòng `{at, draft, rel: "uses", note, status: "accepted", by: "human", via}`;
- mở trang trong Obsidian.

### `pkm VAULT pairs [--min 0.85] [--max 60] [--per-note 2] [--min-words 40] [--write | --out FILE]`

stdout (và file khi có `--write`, ghi vào `VAULT/_E11/similar-pairs.json`): `[{"a": "...", "b": "...", "score": 0.912}]`, điểm giảm dần. Dòng tóm tắt ra stderr. Lệnh này không có phần plugin; `propose --similar` đọc file đó.

### `pkm VAULT zones [--k 15] [--min-words 8] [--max-size 60] [--min-size 5] [--write] [--apply [--top 20]]`

```json
{
 "generated": "2026-11-04",
 "zones": [
  {"label": "từ · từ", "score": 21.4, "notes": 14, "core": ["≤8 note chính"], "all": ["mọi note, note chính trước"],
   "with_highlights": 3, "highlights": 7, "highlight_weight": 15, "colours": {"làm được": 4, "chưa màu": 3},
   "own_ideas": 2, "folders": 3, "top_folders": ["≤3"], "posts": ["Bài viết/..."], "years": ["2020", "2024"],
   "picks": [{"text": "đoạn tô", "colour": "làm được"}]}
 ],
 "stats": {"notes": 591, "knn": 535, "hand_edges": 465, "zones": 33, "covered": 589, "size_min": 5,
           "size_median": 14, "size_max": 60, "with_posts": 0, "single_folder": 9}
}
```

File `.e11/zones.json` là `{generated, zones}` (cần `--write` hoặc `--apply`). Trang `_Xưởng/Gợi ý bài viết.md` (`--apply`) chỉ được ghi khi chưa có. Màn hình vùng của plugin thay trang này: danh sách theo điểm, dòng **Chấm** (`đáng viết`, `không`, `bất ngờ`), link "Viết bài từ vùng này" tạo `Bài viết/<label bỏ ·>` với frontmatter `type: post`, `status: ý tưởng` và mục "Nguồn" liệt kê `core`. Bản Rust của trang là `zones::page`.

## Việc còn lại

- Plan phase 6 đòi "golden kg-solo riêng" cho mỗi lệnh. Chưa làm được: cả ba lệnh cần vector hay tìm kiếm của OHS, CI không có OHS, và bộ so khớp (`parity/`) nằm ngoài phần file được giao. Hướng làm: golden dựng từ file vector cố định của kg-solo, cho `fake-ohs` phát lại.
- "Chạy trên bản B qua lệnh riêng tư chỉ in số": chưa chạy. User chạy `pkm VAULT_B zones --db VAULT_B/.e11/graph.db --no-refresh` (hoặc index `.kg` sau khi build) rồi đối chiếu dòng tóm tắt ở stderr với `zones.py`.
- `USAGE` của `pkm --help` và `VAULT_ABOUT` chưa nêu ba lệnh mới (cố ý, để không đụng chỗ agent `propose` cũng sửa). Nên thêm một dòng sau khi merge.

## Câu hỏi còn mở

- `pairs --write` ghi vào `_E11/similar-pairs.json` (hàng chờ). Adapter `e11` còn đọc `.e11/similar-pairs.json` (tập rộng). Có cần cờ ghi vào chỗ đó không, hay `--out` là đủ?
- Mặc định index của `material`/`zones` là `.kg/graph.db`, như mọi lệnh `pkm`. Bản B hiện dùng `.e11/graph.db` của `graph_index.py`. Khi `pkm setup` (phase 4) dựng bản B thì nên chốt một chỗ.
