#!/usr/bin/env python3
"""Liệt kê commit của repo đặc tả (bản Python) chưa chuyển sang Rust.

Chạy tay khi user bảo (docs/PRD.md mục 5.3); không workflow CI nào gọi lệnh này.
Chỉ dùng thư viện chuẩn, `git` và quyền `gh` sẵn có trên máy; không cần token mới.

    python3 parity/follow.py [--clone-dir DIR] [--repo URL]

Bản clone nằm ngoài repo này: `$PKM_FOLLOW_CLONE`, hoặc
`${XDG_CACHE_HOME:-~/.cache}/pkm-follow/spec-repo`. Lần sau dùng lại bản clone cũ.
"""

import argparse
import os
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Đường dẫn theo dõi, PRD mục 5.1. Danh sách duy nhất.
WATCHED_PATHS = [
    "scripts/kg/",
    "scripts/e11/kg_adapter.py",
    "scripts/e11/graph_index.py",
    "scripts/platform_paths.py",
    "docs/knowledge-graph/schema/schema.yaml",
    "docs/knowledge-graph/install-guide-kg-mcp.md",
    "scripts/kg/tests/",
]

TARGET_COMMITS = 5  # mục tiêu: dưới 5 commit chưa chuyển
TARGET_DAYS = 7  # mục tiêu: dưới 7 ngày kể từ commit cũ nhất chưa chuyển

# Dùng lại quyền `gh` đã đăng nhập, chỉ cho lệnh git này, không đổi cấu hình máy.
GH_CREDENTIALS = ["-c", "credential.helper=", "-c", "credential.helper=!gh auth git-credential"]


def die(msg: str):
    print(f"lỗi: {msg}", file=sys.stderr)
    sys.exit(1)


def git(args: list, cwd=None, auth: bool = False) -> str:
    cmd = ["git"] + (GH_CREDENTIALS if auth else []) + args
    try:
        res = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, encoding="utf-8")
    except FileNotFoundError:
        die("không tìm thấy lệnh git")
    if res.returncode != 0:
        die(f"`git {' '.join(args)}` thất bại: {res.stderr.strip()}")
    return res.stdout


def read_lock(key: str, section: str = "spec") -> str:
    """Đọc `key = "value"` trong mục [section] của spec/spec.lock."""
    current = None
    for line in (ROOT / "spec" / "spec.lock").read_text(encoding="utf-8").splitlines():
        line = line.split("#", 1)[0].strip()
        m = re.fullmatch(r"\[(\w+)\]", line)
        if m:
            current = m.group(1)
            continue
        m = re.fullmatch(rf'{key}\s*=\s*"([^"]*)"', line)
        if m and current == section:
            return m.group(1)
    die(f"spec/spec.lock thiếu [{section}].{key}")


def handled_shas() -> list:
    """Sha (rút gọn hoặc đủ) ở cột đầu bảng trong spec/PORTING.md."""
    shas = []
    for line in (ROOT / "spec" / "PORTING.md").read_text(encoding="utf-8").splitlines():
        if not line.startswith("|"):
            continue
        cell = line.split("|")[1].strip().strip("`")
        if re.fullmatch(r"[0-9a-f]{7,40}", cell):
            shas.append(cell)
    return shas


def ensure_clone(clone: Path, repo: str) -> None:
    if (clone / ".git").is_dir():
        git(["fetch", "--prune", "origin"], cwd=clone, auth=True)
        return
    clone.parent.mkdir(parents=True, exist_ok=True)
    git(["clone", "--no-checkout", repo, str(clone)], auth=True)


def list_commits(clone: Path, base: str) -> list:
    fmt = "%x1e%H%x1f%aI%x1f%s"
    out = git(
        ["log", "--reverse", "--numstat", f"--format={fmt}", f"{base}..origin/main", "--"] + WATCHED_PATHS,
        cwd=clone,
    )
    commits = []
    for rec in out.split("\x1e")[1:]:
        head, _, stats = rec.partition("\n")
        sha, date, title = head.split("\x1f", 2)
        files, lines = [], 0
        for s in stats.splitlines():
            parts = s.split("\t")
            if len(parts) == 3:
                files.append(parts[2])
                lines += sum(int(n) for n in parts[:2] if n.isdigit())
        commits.append({"sha": sha, "date": datetime.fromisoformat(date), "title": title, "files": files, "lines": lines})
    return commits


def main() -> None:
    default_clone = os.environ.get("PKM_FOLLOW_CLONE") or str(
        Path(os.environ.get("XDG_CACHE_HOME") or Path.home() / ".cache") / "pkm-follow" / "spec-repo"
    )
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--clone-dir", default=default_clone, help="nơi giữ bản clone repo đặc tả (ngoài repo này)")
    ap.add_argument("--repo", default=read_lock("repo"), help="URL repo đặc tả (mặc định lấy từ spec.lock)")
    args = ap.parse_args()

    base = read_lock("sha")
    clone = Path(args.clone_dir).expanduser()
    ensure_clone(clone, args.repo)
    commits = list_commits(clone, base)
    handled = handled_shas()

    pending = []
    print(f"Commit chạm đường dẫn theo dõi, từ {base[:7]} tới origin/main:")
    for c in commits:
        done = any(c["sha"].startswith(h) for h in handled)
        if not done:
            pending.append(c)
        files = ", ".join(c["files"]) or "(không đổi file)"
        mark = "[đã xử lý] " if done else ""
        print(f"{c['sha'][:7]} {c['date'].date()} {mark}{c['title']} | {files} | {c['lines']} dòng")

    if not pending:
        print(f"Chưa chuyển: 0 commit (mục tiêu dưới {TARGET_COMMITS} commit, dưới {TARGET_DAYS} ngày): đạt.")
        return
    age = (datetime.now(timezone.utc) - pending[0]["date"]).days
    ok = len(pending) < TARGET_COMMITS and age < TARGET_DAYS
    print(
        f"Chưa chuyển: {len(pending)} commit, commit cũ nhất {age} ngày "
        f"(mục tiêu dưới {TARGET_COMMITS} commit, dưới {TARGET_DAYS} ngày): {'đạt' if ok else 'chưa đạt'}."
    )


if __name__ == "__main__":
    main()
