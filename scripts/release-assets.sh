#!/bin/sh
# Đóng gói asset phát hành của `pkm`. CI, release và máy dev dùng chung script này, nên quy tắc đặt tên asset chỉ
# nằm ở đây (và phải khớp `detect_target` của scripts/install.sh, `$asset` của scripts/install.ps1).
#
#   sh scripts/release-assets.sh asset-name <target>
#   sh scripts/release-assets.sh package <target> <out-dir>    # sau `cargo build --release --target <target> -p pkm`
#   sh scripts/release-assets.sh plugin <dist-dir> <out-dir>    # sau khi dựng plugin Obsidian (apps/obsidian-plugin)
#   sh scripts/release-assets.sh checksums <dir> <số file>      # viết <dir>/checksums.txt, kiểm đủ số file
#   sh scripts/release-assets.sh stamp <vX.Y.Z> <in> <out>      # đóng dấu phiên bản vào bộ cài
#   sh scripts/release-assets.sh version                        # phiên bản crate `pkm`
#
# Asset là binary trần: `pkm-<target>` (Windows thêm `.exe`). `package` chạy thử binary (`--version`) khi máy này
# chạy được đích đó, kiểm binary Windows không phụ thuộc CRT động (VCRUNTIME140.dll), rồi viết `checksums.txt` cho
# riêng asset đó, để thư mục ra dùng được ngay làm `--local-fixture` của bộ cài.
# Plugin Obsidian là `pkm-obsidian-plugin.zip` (phẳng: main.js, manifest.json, styles.css); `pkm setup` tải nó từ
# Release cùng tag với binary và kiểm theo checksums.txt, nên tên asset phải khớp `PLUGIN_ASSET` của
# crates/pkm/src/setup.rs.
set -eu

say() { printf '%s\n' "$*" >&2; }
die() { say "error: $*"; exit 1; }

asset_name() {
  case "$1" in
    x86_64-pc-windows-msvc) printf 'pkm-%s.exe\n' "$1" ;;
    x86_64-unknown-linux-gnu | x86_64-apple-darwin | aarch64-apple-darwin) printf 'pkm-%s\n' "$1" ;;
    *) die "đích không nằm trong danh sách phát hành: $1" ;;
  esac
}

crate_version() {
  # `cargo pkgid` in `path+file:///…/crates/pkm#0.1.0` hoặc `…#pkm@0.1.0`; phần sau `#`/`@` cuối là phiên bản.
  id="$(cargo pkgid --quiet -p pkm)" || die "cargo pkgid -p pkm lỗi"
  printf '%s\n' "${id##*[#@]}"
}

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
  elif command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | cut -d' ' -f1
  else die "không có sha256sum hay shasum"; fi
}

# Máy này chạy được binary của đích $1 không (để chạy thử). In ra lệnh tiền tố (rỗng hoặc `arch -x86_64`).
runner_for() {
  os="$(uname -s)"; cpu="$(uname -m)"
  case "$1:$os:$cpu" in
    x86_64-unknown-linux-gnu:Linux:x86_64) echo native ;;
    x86_64-pc-windows-msvc:MINGW*:* | x86_64-pc-windows-msvc:MSYS*:*) echo native ;;
    aarch64-apple-darwin:Darwin:arm64) echo native ;;
    x86_64-apple-darwin:Darwin:x86_64) echo native ;;
    x86_64-apple-darwin:Darwin:arm64)
      if arch -x86_64 /usr/bin/true >/dev/null 2>&1; then echo rosetta; else echo none; fi ;;
    *) echo none ;;
  esac
}

smoke() {  # file target version
  mode="$(runner_for "$2")"
  if [ "$mode" = none ]; then
    say "::warning::không chạy thử được $2 trên $(uname -sm); chỉ kiểm lúc cài"
    return 0
  fi
  if [ "$mode" = rosetta ]; then vline="$(arch -x86_64 "$1" --version)"; else vline="$("$1" --version)"; fi \
    || die "$1 --version lỗi"
  case "$vline" in
    "pkm $3 "*) say "chạy thử ($mode): $vline" ;;
    *) die "$1 --version in '$vline', cần bắt đầu bằng 'pkm $3 '" ;;
  esac
}

find_dumpbin() {
  if [ -n "${DUMPBIN:-}" ]; then printf '%s\n' "$DUMPBIN"; return 0; fi
  vswhere="/c/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe"
  [ -x "$vswhere" ] || die "không thấy vswhere.exe để tìm dumpbin; đặt DUMPBIN=<đường dẫn dumpbin.exe>"
  found="$("$vswhere" -latest -products '*' -find 'VC/Tools/MSVC/**/bin/Hostx64/x64/dumpbin.exe' | head -n 1 | tr -d '\r')"
  [ -n "$found" ] || die "vswhere không tìm thấy dumpbin.exe"
  cygpath -u "$found"
}

# Binary Windows phải dùng CRT tĩnh (.cargo/config.toml): không được nhập DLL CRT không có sẵn trên Windows sạch.
check_static_crt() {
  dumpbin="$(find_dumpbin)"
  # Dấu `-` thay cho `/`: shell MSYS đổi đối số bắt đầu bằng `/` thành đường dẫn.
  deps="$("$dumpbin" -nologo -dependents "$1" | tr -d '\r' | awk 'tolower($1) ~ /\.dll$/ {print $1}')"
  [ -n "$deps" ] || die "dumpbin không liệt kê DLL nào cho $1"
  say "DLL mà $(basename "$1") nhập:"
  printf '%s\n' "$deps" | sed 's/^/  /' >&2
  bad="$(printf '%s\n' "$deps" | grep -iE '^(vcruntime|msvcp|ucrtbase|api-ms-win-crt-)' || true)"
  [ -z "$bad" ] || die "binary còn phụ thuộc CRT động: $(printf '%s' "$bad" | tr '\n' ' '); kiểm .cargo/config.toml (target-feature=+crt-static) và RUSTFLAGS"
}

package() {  # target out-dir
  target="$1"; out="$2"
  name="$(asset_name "$target")"
  ext=""; case "$target" in *windows*) ext=".exe" ;; esac
  src="${CARGO_TARGET_DIR:-target}/$target/release/pkm$ext"
  [ -f "$src" ] || die "chưa có $src; chạy: cargo build --release --locked -p pkm --target $target"
  version="$(crate_version)"
  mkdir -p "$out"
  cp "$src" "$out/$name"
  chmod 755 "$out/$name"
  case "$target" in *windows*) check_static_crt "$out/$name" ;; esac
  smoke "$out/$name" "$target" "$version"
  printf '%s  %s\n' "$(sha256_of "$out/$name")" "$name" > "$out/checksums.txt"
  say "đóng gói xong: $out/$name"
}

PLUGIN_ASSET="pkm-obsidian-plugin.zip"

plugin() {  # dist-dir out-dir
  dist="$1"; out="$2"
  for f in main.js manifest.json styles.css; do
    [ -s "$dist/$f" ] || die "thiếu $dist/$f; chạy: pnpm --filter pkm-obsidian-plugin run build"
  done
  version="$(crate_version)"
  # manifest.json do esbuild.config.mjs chép nguyên: kiểm id và phiên bản bằng grep, không cần jq.
  grep -Eq '"id"[[:space:]]*:[[:space:]]*"pkm"' "$dist/manifest.json" || die "$dist/manifest.json không có id pkm"
  version_re="$(printf '%s' "$version" | sed 's/\./\\./g')"
  grep -Eq "\"version\"[[:space:]]*:[[:space:]]*\"$version_re\"" "$dist/manifest.json" \
    || die "phiên bản trong $dist/manifest.json khác phiên bản crate $version"
  command -v zip >/dev/null 2>&1 || die "không có lệnh zip"
  mkdir -p "$out"
  rm -f "$out/$PLUGIN_ASSET"
  # -X: bỏ thuộc tính riêng của máy; -j: phẳng, không thư mục.
  zip -q -X -j "$out/$PLUGIN_ASSET" "$dist/main.js" "$dist/manifest.json" "$dist/styles.css"
  say "đóng gói plugin xong: $out/$PLUGIN_ASSET ($version)"
}

checksums() {  # dir expected-count
  dir="$1"; want="$2"
  rm -f "$dir/checksums.txt"
  tmp="$dir/.checksums.$$"
  : > "$tmp"
  n=0
  for f in $(cd "$dir" && LC_ALL=C ls -1); do
    [ -f "$dir/$f" ] || die "$dir/$f không phải file thường"
    case "$f" in .checksums.*) continue ;; esac
    printf '%s  %s\n' "$(sha256_of "$dir/$f")" "$f" >> "$tmp"
    n=$((n + 1))
  done
  [ "$n" -eq "$want" ] || { rm -f "$tmp"; die "cần $want asset, có $n trong $dir"; }
  mv "$tmp" "$dir/checksums.txt"
  if command -v sha256sum >/dev/null 2>&1; then (cd "$dir" && sha256sum -c --strict checksums.txt >&2)
  else (cd "$dir" && shasum -a 256 -c checksums.txt >&2); fi
  cat "$dir/checksums.txt"
}

stamp() {  # version in out
  case "$1" in v[0-9]*) ;; *) die "phiên bản phải dạng vX.Y.Z: $1" ;; esac
  grep -q '@PKM_VERSION@' "$2" || die "$2 không có chỗ đóng dấu @PKM_VERSION@"
  # Bộ cài chỉ dùng ASCII: PowerShell 5.1 đọc `irm` (GitHub trả application/octet-stream) và file .ps1 không BOM
  # theo bảng mã cũ, chữ có dấu sẽ vỡ.
  if LC_ALL=C grep -n '[^[:print:][:space:]]' "$2" >&2; then die "$2 có ký tự ngoài ASCII (dòng ở trên)"; fi
  sed "s/@PKM_VERSION@/$1/g" "$2" > "$3"
  if grep -q '@PKM_VERSION@' "$3"; then die "$3 vẫn còn @PKM_VERSION@"; fi
}

main() {
  [ $# -ge 1 ] || die "thiếu lệnh; xem đầu file"
  cmd="$1"; shift
  case "$cmd" in
    asset-name) [ $# -eq 1 ] || die "asset-name <target>"; asset_name "$1" ;;
    package) [ $# -eq 2 ] || die "package <target> <out-dir>"; package "$1" "$2" ;;
    plugin) [ $# -eq 2 ] || die "plugin <dist-dir> <out-dir>"; plugin "$1" "$2" ;;
    checksums) [ $# -eq 2 ] || die "checksums <dir> <số file>"; checksums "$1" "$2" ;;
    stamp) [ $# -eq 3 ] || die "stamp <vX.Y.Z> <in> <out>"; stamp "$1" "$2" "$3" ;;
    version) crate_version ;;
    *) die "lệnh lạ: $cmd" ;;
  esac
}

main "$@"
