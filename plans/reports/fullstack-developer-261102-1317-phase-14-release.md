# Báo cáo triển khai: workflow release

## Kết quả
Đã tạo `.github/workflows/release.yml`. Chưa chạy được trên GitHub nên phase giữ `in-progress`, ô nghiệm thu chưa tick.

## Thiết kế
- Kích hoạt: push tag `v*` và `workflow_dispatch`. Quyền mặc định `contents: read`.
- Job `build` (ma trận 4 đích): linux-gnu trên ubuntu, `x86_64-apple-darwin` và `aarch64-apple-darwin` trên macos-latest (arm64, biên dịch chéo), windows-msvc trên windows-latest. Thiết lập toolchain giống `ci.yml` (`rustup toolchain install` đọc `rust-toolchain.toml`), thêm `rustup target add`, rồi `cargo build --release --locked --package pkm --target ...`.
- Mỗi đích tạo `pkm-<target>[.exe]` và `pkm-<target>[.exe].sha256` (tên file tương đối bên trong, nên `sha256sum -c` chạy được; macOS dùng `shasum -a 256` dự phòng, định dạng giống nhau). Upload artifact `pkm-<target>`.
- Job `release`: chỉ chạy khi ref là tag `v*`, quyền `contents: write` chỉ ở job này, tải artifact, chạy `sha256sum -c`, rồi `gh release create "$TAG" ... --generate-notes --verify-tag` bằng `GITHUB_TOKEN`. Với `workflow_dispatch` job này bị bỏ qua nên không tạo Release.
- Action: checkout@v7, upload-artifact@v7, download-artifact@v8 (bản mới nhất tra qua `gh api`), rust-cache@v2. Không dùng secret nào khác.

## Kiểm tra cục bộ
- YAML parse OK (2 job: build, release). `actionlint` không có trên máy.
- `cargo build --release --locked --package pkm --target x86_64-unknown-linux-gnu` thành công.
- Các đích khác và bước đóng gói chưa kiểm được cục bộ (hook của môi trường chặn đường dẫn `target/`).

## Lead cần chạy sau khi merge vào main
```
gh workflow run release.yml --ref main
gh run list --workflow release.yml --limit 1            # lấy RUN_ID
gh run watch RUN_ID --exit-status
mkdir -p /tmp/rel && gh run download RUN_ID --dir /tmp/rel
cd /tmp/rel && for d in pkm-*/; do (cd "$d" && sha256sum -c ./*.sha256); done
ls /tmp/rel   # phải đủ 4 thư mục pkm-<target>
```
Kỳ vọng: 4 artifact, mỗi cái gồm binary và `.sha256` đều báo `OK`; không có Release nào được tạo. Nếu đạt, tick ô nghiệm thu và đặt status `done`.

## Lưu ý
- Job `release` chưa thử được (cần tag thật, đã không tạo tag).
- `--verify-tag` yêu cầu tag có sẵn trên remote, luôn đúng với push tag.
