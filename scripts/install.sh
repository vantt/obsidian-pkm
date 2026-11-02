#!/bin/sh
# Installer for pkm on macOS and Linux (Windows: scripts/install.ps1).
#
#   curl -fsSL https://github.com/vantt/obsidian-pkm/releases/latest/download/install.sh | sh
#   curl -fsSL https://github.com/vantt/obsidian-pkm/releases/latest/download/install.sh | sh -s -- --uninstall
#
#   # while the repository is private (needs the GitHub CLI, logged in with `gh auth login`):
#   gh release download --repo vantt/obsidian-pkm --pattern install.sh --output - | sh
#
# Downloads the bare binary pkm-<target> and checksums.txt of one release, refuses unless the
# binary's own checksums.txt line matches, runs it once, then moves it over the old one.
# Each asset is tried anonymously first; on HTTP 401/403/404 (private repository) it falls back to
# `gh release download`. Re-running updates in place; the same version is a no-op. Asset names come from
# scripts/release-assets.sh (asset_name), which the release workflow uses.
# Kept ASCII-only on purpose (see release-assets.sh stamp).
# main "$@" is the LAST line so a truncated `curl | sh` never runs a partial script.
set -eu

APP="pkm"
DEFAULT_REPO="vantt/obsidian-pkm"
DEFAULT_VERSION='@PKM_VERSION@'   # stamped by the release workflow
MARKER=".${APP}-managed"
MAX_BYTES=134217728               # 128 MiB per download
BLOCK_BEGIN="# >>> ${APP} >>>"
BLOCK_END="# <<< ${APP} <<<"

usage() {
  cat <<EOF
Install, update or remove ${APP} (knowledge-graph tools and the 'kg' MCP server for an Obsidian vault).

Usage:
  curl -fsSL https://github.com/${DEFAULT_REPO}/releases/latest/download/install.sh | sh -s -- [options]
  gh release download --repo ${DEFAULT_REPO} --pattern install.sh --output - | sh -s -- [options]
  sh install.sh [options]

While ${DEFAULT_REPO} is private, anonymous downloads fail and the script falls back to the
GitHub CLI: install gh (https://cli.github.com) and run 'gh auth login' first.

Options (each also as an env var):
  --version <vX.Y.Z>     install this version              PKM_VERSION        (default: this release)
  --install-dir <dir>    where the binary goes             PKM_INSTALL_DIR    (default: ~/.local/bin)
  --mcp-install <vault>  then run 'pkm mcp install <vault>' PKM_MCP_INSTALL   (close Claude Desktop first)
  --no-modify-path       only print PATH advice            PKM_NO_MODIFY_PATH=1
  --dry-run              show what would happen            PKM_DRY_RUN=1
  --uninstall            remove the binary and PATH lines  PKM_UNINSTALL=1    (keeps index and data)
  -y, --yes              don't ask for confirmation        PKM_YES=1
  --repo <owner/name>    GitHub repository of the releases PKM_RELEASE_REPO   (default: ${DEFAULT_REPO})
  --release-base-url <u> https URL that holds download/<tag>/<asset>   PKM_RELEASE_BASE_URL
                         (default: https://github.com/<repo>/releases)
  --local-fixture <dir>  take assets from <dir> instead of downloading (tests)   PKM_LOCAL_FIXTURE
  -h, --help             show this help
Other env: PKM_ALLOW_ROOT=1

PATH: unless --no-modify-path, a fenced '${BLOCK_BEGIN}' block is added to ~/.profile, ~/.bashrc,
~/.zshrc (and fish conf.d) when the install dir is not on PATH yet; --uninstall removes it.

Exit codes: 0 ok, 1 failed, 2 usage error.
After installing:  pkm mcp install "/path/to/vault"   then   pkm doctor
EOF
}

say() { printf '%s\n' "$*" >&2; }
die() { say "error: $1"; [ -z "${2:-}" ] || say "  why: $2"; [ -z "${3:-}" ] || say "  fix: $3"; exit 1; }
usage_err() { say "error: $1"; say "  fix: sh install.sh --help"; exit 2; }

parse_args() {
  VERSION="${PKM_VERSION:-$DEFAULT_VERSION}"
  INSTALL_DIR="${PKM_INSTALL_DIR:-${HOME:?HOME is not set}/.local/bin}"
  REPO="${PKM_RELEASE_REPO:-$DEFAULT_REPO}"
  BASE="${PKM_RELEASE_BASE_URL:-}"
  DRY_RUN="${PKM_DRY_RUN:-0}"; NO_PATH="${PKM_NO_MODIFY_PATH:-0}"
  UNINSTALL="${PKM_UNINSTALL:-0}"; YES="${PKM_YES:-0}"
  MCP_VAULT="${PKM_MCP_INSTALL:-}"; FIXTURE="${PKM_LOCAL_FIXTURE:-}"
  while [ $# -gt 0 ]; do
    case "$1" in
      --version|--install-dir|--mcp-install|--local-fixture|--repo|--release-base-url)
        if [ $# -lt 2 ] || [ -z "$2" ]; then usage_err "$1 needs a value"; fi
        case "$1" in
          --version) VERSION="$2" ;;
          --install-dir) INSTALL_DIR="$2" ;;
          --mcp-install) MCP_VAULT="$2" ;;
          --local-fixture) FIXTURE="$2" ;;
          --repo) REPO="$2" ;;
          --release-base-url) BASE="$2" ;;
        esac
        shift ;;
      --version=*) VERSION="${1#*=}" ;;
      --install-dir=*) INSTALL_DIR="${1#*=}" ;;
      --mcp-install=*) MCP_VAULT="${1#*=}" ;;
      --local-fixture=*) FIXTURE="${1#*=}" ;;
      --repo=*) REPO="${1#*=}" ;;
      --release-base-url=*) BASE="${1#*=}" ;;
      --dry-run) DRY_RUN=1 ;;
      --no-modify-path) NO_PATH=1 ;;
      --uninstall) UNINSTALL=1 ;;
      -y|--yes) YES=1 ;;
      -h|--help) usage; exit 0 ;;
      *) usage_err "unknown option: $1" ;;
    esac
    shift
  done
  case "$VERSION" in
    @*@|v[0-9]*) ;;
    [0-9]*) VERSION="v$VERSION" ;;   # accept 1.2.3 and v1.2.3
    *) usage_err "invalid version: $VERSION (expected vX.Y.Z)" ;;
  esac
  case "$VERSION" in *[!A-Za-z0-9._+@-]*) usage_err "invalid version: $VERSION" ;; esac
  case "$REPO" in
    */*/*|/*|*/|*[!A-Za-z0-9._/-]*) usage_err "invalid --repo: $REPO (expected owner/name)" ;;
    */*) ;;
    *) usage_err "invalid --repo: $REPO (expected owner/name)" ;;
  esac
  [ -n "$BASE" ] || BASE="https://github.com/$REPO/releases"
  BASE="${BASE%/}"
  case "$BASE" in https://*) ;; *) usage_err "release base URL must be https: $BASE" ;; esac
  case "$INSTALL_DIR" in /*) ;; *) usage_err "--install-dir must be an absolute path: $INSTALL_DIR" ;; esac
  INSTALL_DIR="${INSTALL_DIR%/}"
  if [ "$(id -u)" = 0 ] && [ "${PKM_ALLOW_ROOT:-0}" != 1 ]; then
    die "refusing to run as root" "pkm installs per user" "re-run as your user, or set PKM_ALLOW_ROOT=1"
  fi
}

# Single naming rule, same as scripts/release-assets.sh asset_name.
detect_target() {
  case "$(uname -s)" in
    Linux) os=linux ;;
    Darwin) os=darwin ;;
    MINGW*|MSYS*|CYGWIN*) die "Windows shell detected" "this script is for macOS and Linux" \
      "in PowerShell: gh release download --repo $REPO --pattern install.ps1 --output - | Out-String | iex" ;;
    *) die "unsupported OS: $(uname -s)" "no prebuilt binary" "build from source: cargo install --git https://github.com/$REPO --locked pkm" ;;
  esac
  cpu="$(uname -m)"
  # A shell running under Rosetta reports x86_64 on Apple silicon; prefer the native build.
  if [ "$os" = darwin ] && [ "$cpu" = x86_64 ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || echo 0)" = 1 ]; then
    cpu=arm64
  fi
  case "$os:$cpu" in
    linux:x86_64|linux:amd64) TARGET=x86_64-unknown-linux-gnu ;;
    darwin:x86_64) TARGET=x86_64-apple-darwin ;;
    darwin:arm64|darwin:aarch64) TARGET=aarch64-apple-darwin ;;
    *) die "unsupported platform: $(uname -sm)" "no prebuilt binary" "build from source: cargo install --git https://github.com/$REPO --locked pkm" ;;
  esac
  ASSET="${APP}-${TARGET}"
}

# Anonymous HTTPS download with wall-clock timeout and size cap. Returns 0 when saved, 3 when the
# server answered 401/403/404 (private repository or missing asset), and dies on anything else.
fetch() {  # url dest
  if command -v curl >/dev/null 2>&1; then
    code="$(curl --proto '=https' --tlsv1.2 -sSL --connect-timeout 10 --max-time 300 --max-filesize "$MAX_BYTES" \
      -o "$2" -w '%{http_code}' "$1")" || die "download failed: $1" "network error or file too large" "check your connection and retry"
    case "$code" in
      2??) return 0 ;;
      401|403|404) rm -f "$2"; return 3 ;;
      *) die "download failed: $1 (HTTP $code)" "" "retry later" ;;
    esac
  elif command -v wget >/dev/null 2>&1; then
    # busybox wget has no --https-only and no portable way to read the status; HTTPS is enforced on
    # BASE above, and any failure is treated like 404 so the gh fallback can still run.
    if command -v timeout >/dev/null 2>&1; then timeout 310 wget -q -O "$2" "$1"; else wget -q -O "$2" "$1"; fi \
      || { rm -f "$2"; return 3; }
    [ "$(wc -c < "$2")" -le "$MAX_BYTES" ] || die "download larger than allowed: $1"
  else
    return 3
  fi
}

# Download through the GitHub CLI, which uses the user's own login (works for private repositories).
gh_fetch() {  # name dest
  command -v gh >/dev/null 2>&1 || die "cannot download $1 anonymously from $BASE" \
    "the repository $REPO is private (or the asset is missing), and the GitHub CLI 'gh' is not installed" \
    "install gh (https://cli.github.com), run 'gh auth login', then run this installer again"
  gh auth status >/dev/null 2>&1 || die "cannot download $1 anonymously from $BASE" \
    "the repository $REPO is private (or the asset is missing), and gh is not logged in" \
    "run 'gh auth login', then run this installer again"
  gh release download "$VERSION" --repo "$REPO" --pattern "$1" --output "$2" --clobber \
    || die "gh release download $VERSION --repo $REPO --pattern $1 failed" "no access to $REPO, or release $VERSION has no $1" \
      "check with: gh release view $VERSION --repo $REPO"
  [ "$(wc -c < "$2")" -le "$MAX_BYTES" ] || die "download larger than allowed: $1"
}

VIA_GH=0
get_asset() {  # name dest
  if [ -n "$FIXTURE" ]; then
    [ -f "$FIXTURE/$1" ] || die "local fixture has no $1: $FIXTURE"
    cp "$FIXTURE/$1" "$2"
    return 0
  fi
  if [ "$VIA_GH" = 0 ]; then
    rc=0; fetch "$BASE/download/$VERSION/$1" "$2" || rc=$?
    [ "$rc" = 0 ] && return 0
    say "note: anonymous download of $1 was refused (private repository?); trying the GitHub CLI"
    VIA_GH=1
  fi
  gh_fetch "$1" "$2"
}

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
  elif command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | cut -d' ' -f1
  elif command -v openssl >/dev/null 2>&1; then openssl dgst -sha256 "$1" | awk '{print $NF}'
  else die "no SHA-256 tool found" "" "install coreutils or openssl"; fi
}

verify() {  # exact filename match, exactly one entry
  expected="$(awk -v f="$ASSET" '{n=$2; sub(/^\*/,"",n); if (n==f) {print $1; c++}} END{if (c!=1) exit 1}' "$TMP/checksums.txt")" \
    || die "checksum entry for $ASSET missing or duplicated" "the release is incomplete" "try again later or pick another --version"
  case "$expected" in *[!0-9a-fA-F]*|"") die "invalid checksum entry for $ASSET" ;; esac
  [ "${#expected}" -eq 64 ] || die "invalid checksum entry for $ASSET"
  actual="$(sha256 "$TMP/$ASSET")"
  [ "$actual" = "$(printf '%s' "$expected" | tr 'A-F' 'a-f')" ] \
    || die "checksum mismatch for $ASSET" "download corrupted or tampered" "retry; report it if it persists"
  say "checksum ok: $ASSET"
}

# Version line of a pkm binary must start with "pkm <version without v> ".
runs_as_version() {  # bin
  out="$("$1" --version 2>/dev/null)" || return 1
  case "$out" in "$APP ${VERSION#v} "*) return 0 ;; *) return 1 ;; esac
}

installed_version() {
  [ -f "$INSTALL_DIR/$MARKER" ] || return 0
  sed -n 's/^version=//p' "$INSTALL_DIR/$MARKER" | head -n 1
}

install_bin() {
  chmod 755 "$TMP/$ASSET"
  runs_as_version "$TMP/$ASSET" \
    || die "the downloaded binary does not run here or reports another version" "wrong platform, old glibc, or corrupt file" "report your OS/CPU: $(uname -sm)"
  target="$INSTALL_DIR/$APP"
  if [ -e "$target" ] && [ ! -f "$INSTALL_DIR/$MARKER" ]; then
    die "$target exists and was not installed by this script" "" "remove it, or use --install-dir <other dir>"
  fi
  if [ "$DRY_RUN" = 1 ]; then say "dry-run: would install $ASSET $VERSION to $target"; return 0; fi
  mkdir -p "$INSTALL_DIR"
  rm -f "$target.rollback"
  [ -f "$target" ] && cp -p "$target" "$target.rollback"
  cp "$TMP/$ASSET" "$INSTALL_DIR/.$APP.new.$$" && chmod 755 "$INSTALL_DIR/.$APP.new.$$"
  mv -f "$INSTALL_DIR/.$APP.new.$$" "$target"            # rename: no "Text file busy"
  if ! runs_as_version "$target"; then
    if [ -f "$target.rollback" ]; then mv -f "$target.rollback" "$target"; else rm -f "$target"; fi
    die "the new binary failed to start; previous state restored"
  fi
  rm -f "$target.rollback"
  (umask 077; printf '%s-managed-v1\nversion=%s\n' "$APP" "$VERSION" > "$INSTALL_DIR/$MARKER")
  say "installed: $("$target" --version) -> $target"
}

on_path() { case ":${PATH:-}:" in *":$INSTALL_DIR:"*|*":$INSTALL_DIR/:"*) return 0 ;; *) return 1 ;; esac; }

# Shell startup files that get the PATH block. Only files that exist, plus ~/.profile and,
# for a zsh user, ~/.zshrc (created if missing). fish gets its own file in conf.d.
rc_files() {
  printf '%s\n' "$HOME/.profile"
  [ -f "$HOME/.bashrc" ] && printf '%s\n' "$HOME/.bashrc"
  zrc="${ZDOTDIR:-$HOME}/.zshrc"
  if [ -f "$zrc" ]; then printf '%s\n' "$zrc"
  else case "${SHELL:-}" in */zsh) printf '%s\n' "$zrc" ;; esac; fi
  return 0
}
fish_file() { printf '%s\n' "${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/$APP.fish"; }

strip_block() {  # file -- remove our fenced block, keep everything else byte for byte
  [ -f "$1" ] || return 0
  grep -qxF "$BLOCK_BEGIN" "$1" || return 0
  awk -v b="$BLOCK_BEGIN" -v e="$BLOCK_END" '$0==b{skip=1;next} skip&&$0==e{skip=0;next} !skip{print}' "$1" > "$1.$APP-tmp.$$"
  cat "$1.$APP-tmp.$$" > "$1" && rm -f "$1.$APP-tmp.$$"    # keep the file's inode, owner and mode
}

path_advice() {
  say "PATH: $INSTALL_DIR is not on your PATH. Add this line to your shell profile:"
  say "  export PATH=\"$INSTALL_DIR:\$PATH\""
}

modify_path() {
  on_path && return 0
  if [ "$NO_PATH" = 1 ]; then path_advice; return 0; fi
  case "$INSTALL_DIR" in
    *[\"\$\`\\]*) say "PATH: install dir has shell metacharacters; not editing profiles"; path_advice; return 0 ;;
  esac
  if [ "$DRY_RUN" = 1 ]; then
    say "dry-run: would add $INSTALL_DIR to PATH in: $(rc_files | tr '\n' ' ')"; return 0
  fi
  rc_files | while IFS= read -r rc; do
    strip_block "$rc"
    {
      printf '%s\n' "$BLOCK_BEGIN"
      # shellcheck disable=SC2016 # $PATH must stay literal: it expands when the profile runs
      printf 'case ":${PATH}:" in *":%s:"*) ;; *) export PATH="%s:$PATH" ;; esac\n' "$INSTALL_DIR" "$INSTALL_DIR"
      printf '%s\n' "$BLOCK_END"
    } >> "$rc"
    say "PATH: added $INSTALL_DIR in $rc"
  done
  ff="$(fish_file)"
  if [ -d "$(dirname "$(dirname "$ff")")" ]; then
    mkdir -p "$(dirname "$ff")"
    printf '%s\nfish_add_path -g "%s"\n%s\n' "$BLOCK_BEGIN" "$INSTALL_DIR" "$BLOCK_END" > "$ff"
    say "PATH: added $INSTALL_DIR in $ff"
  fi
  say "PATH: open a new terminal, or run now:  export PATH=\"$INSTALL_DIR:\$PATH\""
}

unmodify_path() {
  rc_files | while IFS= read -r rc; do
    if [ -f "$rc" ] && grep -qxF "$BLOCK_BEGIN" "$rc"; then strip_block "$rc"; say "PATH: removed block from $rc"; fi
  done
  ff="$(fish_file)"
  if [ -f "$ff" ] && grep -qxF "$BLOCK_BEGIN" "$ff"; then rm -f "$ff"; say "PATH: removed $ff"; fi
  return 0
}

confirm() {  # ask via /dev/tty (stdin is the script under curl|sh); no TTY -> require --yes
  [ "$YES" = 1 ] && return 0
  if (: </dev/tty >/dev/tty) 2>/dev/null; then
    printf '%s [y/N] ' "$1" > /dev/tty; read -r ans < /dev/tty || ans=""
    case "$ans" in y|Y|yes|YES) return 0 ;; *) say "cancelled"; exit 1 ;; esac
  fi
  die "confirmation needed but no terminal is available" "" "re-run with --yes"
}

# Data directory of pkm (index, OHS bundle): platform::pkm_dir() in crates/pkm-core/src/platform.rs.
data_dir() {
  case "$(uname -s)" in
    Darwin) printf '%s\n' "$HOME/Library/Application Support/$APP" ;;
    *) printf '%s\n' "${XDG_CACHE_HOME:-$HOME/.cache}/$APP" ;;
  esac
}

uninstall() {
  [ -f "$INSTALL_DIR/$MARKER" ] || die "no install made by this script in $INSTALL_DIR" "" "use --install-dir <dir> if you installed elsewhere"
  say "will remove: $INSTALL_DIR/$APP and the PATH lines this script added"
  say "will keep:   index and data in $(data_dir)  (delete that folder yourself to remove them)"
  if [ -x "$INSTALL_DIR/$APP" ]; then
    say "will keep:   the 'kg' entry in Claude Desktop. To remove it first, run:"
    say "               \"$INSTALL_DIR/$APP\" mcp install --remove"
  fi
  if [ "$DRY_RUN" = 1 ]; then say "dry-run: nothing removed"; return 0; fi
  confirm "Remove $APP?"
  rm -f "$INSTALL_DIR/$APP" "$INSTALL_DIR/$APP.rollback" "$INSTALL_DIR/$MARKER"
  unmodify_path
  say "removed $APP"
}

mcp_install() {
  [ -n "$MCP_VAULT" ] || return 0
  bin="$INSTALL_DIR/$APP"
  if [ "$DRY_RUN" = 1 ]; then say "dry-run: would run: \"$bin\" mcp install \"$MCP_VAULT\""; return 0; fi
  [ -d "$MCP_VAULT" ] || die "vault folder not found: $MCP_VAULT" "" "pass the folder that holds your Obsidian vault"
  say "running: \"$bin\" mcp install \"$MCP_VAULT\""
  "$bin" mcp install "$MCP_VAULT" \
    || die "pkm is installed, but 'pkm mcp install' failed (see above)" "" "fix the cause, then run: \"$bin\" mcp install \"$MCP_VAULT\""
}

next_steps() {   # same steps as the README quick start
  bin="$INSTALL_DIR/$APP"
  say ""
  if [ -n "$MCP_VAULT" ]; then
    say "Next: start Claude Desktop and ask it about your vault. If something is wrong, run:"
    say "  \"$bin\" doctor"
  else
    say "Next: quit Claude Desktop completely, then run:"
    say "  \"$bin\" mcp install \"/path/to/your/vault\""
    say "  \"$bin\" doctor"
  fi
}

main() {
  parse_args "$@"
  if [ "$UNINSTALL" = 1 ]; then uninstall; return 0; fi
  case "$VERSION" in @*@) die "this copy of the installer has no version" "it was not downloaded from a release" \
    "pass --version vX.Y.Z, or use the install.sh attached to a release of $REPO" ;; esac
  if [ -n "$FIXTURE" ] && [ ! -d "$FIXTURE" ]; then usage_err "--local-fixture is not a directory: $FIXTURE"; fi
  detect_target
  if [ "$(installed_version)" = "$VERSION" ] && [ -x "$INSTALL_DIR/$APP" ] && runs_as_version "$INSTALL_DIR/$APP"; then
    say "$APP $VERSION is already installed in $INSTALL_DIR; nothing to do"
  else
    TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT INT TERM
    get_asset "$ASSET" "$TMP/$ASSET"
    get_asset checksums.txt "$TMP/checksums.txt"
    verify
    install_bin
  fi
  modify_path
  mcp_install
  [ "$DRY_RUN" = 1 ] || next_steps
}

main "$@"
