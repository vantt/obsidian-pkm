#!/bin/sh
# Kiểm vòng đời của scripts/install.sh trên thư mục asset cục bộ (không cần mạng), trong HOME tạm:
# cài, chạy lại (không làm gì), khối PATH, từ chối khi checksum sai, từ chối đè binary lạ, gỡ cài.
#
#   sh scripts/test-installer.sh <thư mục asset có checksums.txt> <vX.Y.Z>
#
# Thư mục asset là đầu ra của `scripts/release-assets.sh package` (hoặc artifact pkm-assets của release).
# `A && B || fail` ở dưới là có chủ ý: fail khi A hoặc B sai.
# shellcheck disable=SC2015
set -eu

say() { printf '%s\n' "$*" >&2; }
fail() { say "FAIL: $*"; exit 1; }

[ $# -eq 2 ] || fail "dùng: sh scripts/test-installer.sh <thư mục asset> <vX.Y.Z>"
FIXTURE="$(cd "$1" && pwd)"; VERSION="$2"
INSTALLER="$(cd "$(dirname "$0")" && pwd)/install.sh"

WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT INT TERM
export HOME="$WORK/home"; mkdir -p "$HOME"; : > "$HOME/.bashrc"
unset ZDOTDIR XDG_CONFIG_HOME XDG_CACHE_HOME PKM_VERSION PKM_INSTALL_DIR PKM_RELEASE_REPO PKM_RELEASE_BASE_URL \
  PKM_DRY_RUN PKM_NO_MODIFY_PATH PKM_UNINSTALL PKM_YES PKM_MCP_INSTALL PKM_LOCAL_FIXTURE || true
export SHELL=/bin/bash
BIN="$HOME/.local/bin"
# PATH của tiến trình test không có thư mục cài, để bộ cài phải thêm khối PATH.
PATH="$(printf '%s' "$PATH" | tr ':' '\n' | grep -vxF "$BIN" | paste -sd: -)"; export PATH

run() { sh "$INSTALLER" --local-fixture "$FIXTURE" --version "$VERSION" "$@"; }

say "== --help thoát 0"
sh "$INSTALLER" --help >/dev/null || fail "--help"

say "== cờ lạ thoát 2"
rc=0; sh "$INSTALLER" --bogus 2>/dev/null || rc=$?; [ "$rc" = 2 ] || fail "cờ lạ trả $rc, cần 2"

say "== bản chưa đóng dấu từ chối khi thiếu --version"
rc=0; sh "$INSTALLER" --local-fixture "$FIXTURE" 2>/dev/null || rc=$?; [ "$rc" = 1 ] || fail "thiếu --version trả $rc"

say "== --dry-run không ghi gì"
run --dry-run
[ ! -e "$BIN" ] || fail "--dry-run đã tạo $BIN"

say "== cài"
run
"$BIN/pkm" --version | grep -q "^pkm ${VERSION#v} " || fail "pkm --version sai"
[ -f "$BIN/.pkm-managed" ] || fail "thiếu marker"
grep -qxF '# >>> pkm >>>' "$HOME/.profile" || fail "thiếu khối PATH trong .profile"
grep -qxF '# >>> pkm >>>' "$HOME/.bashrc" || fail "thiếu khối PATH trong .bashrc"
# shellcheck source=/dev/null
( . "$HOME/.profile"; case ":$PATH:" in *":$BIN:"*) ;; *) exit 1 ;; esac ) || fail ".profile không đưa $BIN vào PATH"

say "== chạy lại: không làm gì, không nhân đôi khối PATH"
before="$(cksum < "$BIN/pkm")"
run 2>"$WORK/rerun.log"
grep -q 'nothing to do' "$WORK/rerun.log" || fail "chạy lại không báo 'nothing to do'"
[ "$(cksum < "$BIN/pkm")" = "$before" ] || fail "chạy lại đã đổi binary"
[ "$(grep -cxF '# >>> pkm >>>' "$HOME/.profile")" = 1 ] || fail "khối PATH bị nhân đôi"

say "== checksum sai: từ chối, binary cũ giữ nguyên"
bad="$WORK/bad"; mkdir -p "$bad"; cp "$FIXTURE"/* "$bad/"
asset="$(awk 'NR==1{print $2}' "$FIXTURE/checksums.txt")"
printf 'garbage' >> "$bad/$asset"
rm -f "$BIN/.pkm-managed.tmp"
sed -i.bak 's/^version=.*/version=v0.0.0/' "$BIN/.pkm-managed" && rm -f "$BIN/.pkm-managed.bak"   # buộc tải lại
rc=0; sh "$INSTALLER" --local-fixture "$bad" --version "$VERSION" 2>"$WORK/bad.log" || rc=$?
[ "$rc" = 1 ] && grep -q 'checksum mismatch' "$WORK/bad.log" || fail "checksum sai không bị từ chối (rc=$rc)"
[ "$(cksum < "$BIN/pkm")" = "$before" ] || fail "checksum sai đã đổi binary"

say "== cài lại khi marker ghi bản khác (cập nhật)"
run
grep -qx "version=$VERSION" "$BIN/.pkm-managed" || fail "marker không cập nhật"

say "== --mcp-install với vault không có: lỗi rõ, pkm vẫn cài"
rc=0; run --mcp-install "$WORK/no-such-vault" 2>"$WORK/mcp.log" || rc=$?
[ "$rc" = 1 ] && grep -q 'vault folder not found' "$WORK/mcp.log" || fail "--mcp-install vault sai (rc=$rc)"

say "== không có TTY và không --yes: gỡ cài từ chối"
rc=0; run --uninstall </dev/null 2>/dev/null || rc=$?
if [ -e /dev/tty ] && (: </dev/tty) 2>/dev/null; then say "   (có TTY, bỏ qua bước này)"; else [ "$rc" = 1 ] || fail "gỡ không --yes trả $rc"; fi

say "== gỡ cài"
run --uninstall --yes
[ ! -e "$BIN/pkm" ] || fail "binary còn sau khi gỡ"
[ ! -e "$BIN/.pkm-managed" ] || fail "marker còn sau khi gỡ"
! grep -qF '# >>> pkm >>>' "$HOME/.profile" "$HOME/.bashrc" || fail "khối PATH còn sau khi gỡ"

say "== binary lạ ở đích: từ chối đè"
mkdir -p "$BIN"; printf '#!/bin/sh\necho mine\n' > "$BIN/pkm"; chmod 755 "$BIN/pkm"
rc=0; run 2>"$WORK/foreign.log" || rc=$?
[ "$rc" = 1 ] && grep -q 'not installed by this script' "$WORK/foreign.log" || fail "đè binary lạ (rc=$rc)"
[ "$("$BIN/pkm")" = mine ] || fail "binary lạ bị đổi"

say "OK: install.sh qua mọi bước"
