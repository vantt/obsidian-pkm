"""Chạy bộ so khớp trên máy (cần Python 3, PyYAML, git, node; CI không dùng script này). Cách dùng: parity/README.md.

  python3 parity/parity.py golden [--sha SHA]                    dựng golden kg-solo vào parity/golden/<sha đủ>/
  python3 parity/parity.py run [--against rust|python]           Python ở sha ghim trên kg-solo, so với Rust hoặc Python
  python3 parity/parity.py run --private VAULT [cờ đọc vault]    như trên với vault thật của user: chỉ in số lệch

Repo đặc tả lấy từ `--spec-dir` (checkout có sẵn, phải đúng sha) hoặc clone vào ~/.cache/pkm-parity/ (repo private:
máy cần đăng nhập `gh` và `gh auth setup-git`). Fixture kg-solo dựng bằng `npx github:vantt/obsidian-test-fixture#TAG`
(hoặc `--otf DIR`, checkout có sẵn của obsidian-test-fixture), tag, preset, seed lấy từ spec/spec.lock.
Dump tạm nằm ngoài repo và bị xoá sau khi so (giữ lại bằng `--keep DIR`).
"""
import argparse, json, os, shutil, subprocess, sys, tempfile

try:
    import tomllib
except ModuleNotFoundError:  # Python < 3.11
    sys.exit('cần Python 3.11 trở lên (tomllib)')

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DUMPER = os.path.join(ROOT, 'parity', 'dump-python.py')
CACHE = os.path.join(os.path.expanduser('~'), '.cache', 'pkm-parity')


def lock():
    with open(os.path.join(ROOT, 'spec', 'spec.lock'), 'rb') as f:
        return tomllib.load(f)


def sh(argv, **kw):
    """Chạy lệnh, dừng với thông báo rõ khi lỗi."""
    try:
        return subprocess.run(argv, check=True, **kw)
    except FileNotFoundError:
        sys.exit(f'không chạy được {argv[0]}: chưa cài?')
    except subprocess.CalledProcessError as e:
        sys.exit(f'lệnh lỗi (mã {e.returncode}): {" ".join(map(str, argv))}')


def git_head(path):
    r = subprocess.run(['git', '-C', path, 'rev-parse', 'HEAD'], capture_output=True, text=True)
    return r.stdout.strip() if r.returncode == 0 else None


def spec_checkout(sha, spec_dir, repo):
    """Thư mục repo đặc tả ở đúng `sha`; trả (thư mục, sha đủ)."""
    if spec_dir:
        head = git_head(spec_dir)
        if not head or not head.startswith(sha):
            sys.exit(f'--spec-dir {spec_dir} đang ở {head}, không phải {sha}; checkout đúng sha rồi chạy lại')
        return os.path.abspath(spec_dir), head
    os.makedirs(CACHE, exist_ok=True)
    for name in os.listdir(CACHE):
        path = os.path.join(CACHE, name)
        if name.startswith('spec-') and (git_head(path) or '').startswith(sha):
            return path, git_head(path)
    tmp = tempfile.mkdtemp(prefix='spec-clone-', dir=CACHE)
    try:
        sh(['git', 'clone', '--quiet', repo, tmp])
        sh(['git', '-C', tmp, 'checkout', '--quiet', '--detach', sha])
        full = git_head(tmp)
        dest = os.path.join(CACHE, 'spec-' + full)
        os.rename(tmp, dest)
        return dest, full
    except BaseException:
        shutil.rmtree(tmp, ignore_errors=True)
        raise


def fixture(fx, otf, parent):
    """Sinh vault fixture (preset, seed của spec.lock) vào parent/<preset>; tên thư mục là tên vault trong golden."""
    out = os.path.join(parent, fx['preset'])
    args = ['generate', '--preset', fx['preset'], '--seed', str(fx['seed']), '--out', out]
    if otf:
        sh(['node', os.path.join(otf, 'bin', 'otf.mjs'), *args], stdout=subprocess.DEVNULL)
    else:
        npx = shutil.which('npx') or sys.exit('cần node và npx (hoặc --otf DIR)')
        repo = fx['repo'].removeprefix('https://github.com/')
        sh([npx, '--yes', f'github:{repo}#{fx["tag"]}', *args], stdout=subprocess.DEVNULL)
    return out


def dump(spec, vault, out, read_flags, extra=(), private=False):
    """Chạy dump-python.py. Với vault riêng tư, lỗi chỉ in loại lỗi: traceback có thể chứa tên note."""
    argv = [sys.executable, DUMPER, '--spec', spec, '--vault', vault, '--out', out, *read_flags, *extra]
    if not private:
        sh(argv)
        return
    r = subprocess.run(argv, capture_output=True, text=True, encoding='utf-8', errors='replace')
    if r.returncode != 0:
        last = (r.stderr.strip().splitlines() or ['?'])[-1]
        kind = last.split(':', 1)[0] if ':' in last else 'lỗi'
        sys.exit(f'dump Python lỗi ({kind}); chi tiết ẩn vì --private. Xem riêng trên máy bằng cách chạy '
                 f'parity/dump-python.py trực tiếp, không dán đầu ra vào repo hay issue.')


def parity_bin(*args):
    """Chạy bộ so khớp Rust; trả mã thoát (0 sạch, 1 có lệch chưa có tên, 2 lỗi)."""
    return subprocess.run(['cargo', 'run', '--quiet', '--locked', '-p', 'pkm-parity', '--', *args], cwd=ROOT).returncode


def read_flags(a):
    f = ['--packs', a.packs, '--edge-keys', a.edge_keys]
    f += ['--adapter', a.adapter] if a.adapter else []
    f += ['--no-aliases'] if a.no_aliases else []
    f += ['--body-edges'] if a.body_edges else []
    f += ['--today', a.today] if a.today else []
    return f


def cmd_golden(a, cfg):
    spec, full = spec_checkout(a.sha or cfg['spec']['sha'], a.spec_dir, cfg['spec']['repo'])
    work = tempfile.mkdtemp(prefix='pkm-parity-')
    try:
        vault = fixture(cfg['fixture'], a.otf, work)
        first, second = os.path.join(work, 'dump-a'), os.path.join(work, 'dump-b')
        flags = read_flags(a)
        dump(spec, vault, first, flags)
        with open(os.path.join(first, 'manifest.json'), encoding='utf-8') as f:
            check = (json.load(f).get('answers') or {}).get('fixture_check') or {}
        if check.get('fails'):
            sys.exit(f'fixture_check của repo đặc tả ở {full} có {check["fails"]} câu sai: không dựng golden')
        dump(spec, vault, second, flags, ['--replay', first])
        code = parity_bin('compare', first, second, '--report', os.path.join(work, 'self-report.json'))
        if code != 0:
            sys.exit('Python ở sha này không cho cùng kết quả hai lần: không dựng golden (xem bảng ở trên)')
        dest = os.path.join(ROOT, 'parity', 'golden', full)
        if os.path.exists(dest):
            shutil.rmtree(dest)
        shutil.copytree(first, dest, ignore=shutil.ignore_patterns('perf.json'))
        print(f'golden: {os.path.relpath(dest, ROOT)} (fixture_check graph {check.get("graph")}, table {check.get("table")})')
    finally:
        shutil.rmtree(work, ignore_errors=True)


def cmd_run(a, cfg):
    if a.keep and os.path.abspath(a.keep).startswith(os.path.join(ROOT, '')):
        sys.exit('--keep phải ở ngoài repo (dump chứa nội dung vault)')
    spec, full = spec_checkout(a.sha or cfg['spec']['sha'], a.spec_dir, cfg['spec']['repo'])
    work = tempfile.mkdtemp(prefix='pkm-parity-')
    try:
        if a.private:
            vault = os.path.abspath(a.private)
            if vault == ROOT or vault.startswith(os.path.join(ROOT, '')) or ROOT.startswith(os.path.join(vault, '')):
                sys.exit('--private: vault và repo không được lồng nhau')
        else:
            vault = fixture(cfg['fixture'], a.otf, work)
        expected = os.path.join(work, 'python')
        flags = read_flags(a)
        dump(spec, vault, expected, flags, private=bool(a.private))
        private = ['--private'] if a.private else []
        # Mọi tầng của Rust đã xong: mục "chưa có" (như thiếu PKM_BIN) là lỗi, không phải qua.
        private += ['--strict']
        report = ['--report', a.report] if a.report else []
        if a.against == 'python':
            actual = os.path.join(work, 'python-2')
            dump(spec, vault, actual, flags, ['--replay', expected], private=bool(a.private))
            code = parity_bin('compare', expected, actual, *private, *report)
        else:
            code = parity_bin('check', '--vault', vault, '--golden', expected,
                              '--dump-out', os.path.join(work, 'rust'), *private, *report)
        print(f'spec {full}, vault {"(riêng tư)" if a.private else os.path.basename(vault)}, so với {a.against}: mã {code}')
        if a.keep:
            if os.path.exists(a.keep):
                shutil.rmtree(a.keep)
            shutil.copytree(work, a.keep, ignore=shutil.ignore_patterns(os.path.basename(vault)) if not a.private else None)
        return code
    finally:
        shutil.rmtree(work, ignore_errors=True)


def main(argv=None):
    # Trên Windows, stdout/stderr ra pipe dùng bảng mã cũ (cp1252): in tiếng Việt làm script sập. Trên Linux không đổi byte nào.
    for s in (sys.stdout, sys.stderr):
        s.reconfigure(encoding='utf-8')
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest='cmd', required=True)
    for name in ('golden', 'run'):
        p = sub.add_parser(name)
        p.add_argument('--sha', help='sha của repo đặc tả (mặc định: sha ghim trong spec/spec.lock)')
        p.add_argument('--spec-dir', help='checkout có sẵn của repo đặc tả, đã ở đúng sha')
        p.add_argument('--otf', help='checkout có sẵn của obsidian-test-fixture ở tag của spec.lock (thay cho npx)')
        p.add_argument('--packs', default='business')
        p.add_argument('--adapter')
        p.add_argument('--no-aliases', action='store_true')
        p.add_argument('--edge-keys', choices=['schema', 'links'], default='schema')
        p.add_argument('--body-edges', action='store_true')
        p.add_argument('--today')
    run = sub.choices['run']
    run.add_argument('--against', choices=['rust', 'python'], default='rust')
    run.add_argument('--private', metavar='VAULT', help='vault thật của user: chỉ in số lệch theo bảng và theo tool')
    run.add_argument('--report', help='file báo cáo (mặc định parity/report.json)')
    run.add_argument('--keep', metavar='DIR', help='giữ các dump vào DIR (ngoài repo) để xem lệch')
    a = ap.parse_args(argv)
    cfg = lock()
    if a.cmd == 'golden':
        return cmd_golden(a, cfg)
    return cmd_run(a, cfg)


if __name__ == '__main__':
    sys.exit(main() or 0)
