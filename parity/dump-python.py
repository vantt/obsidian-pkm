"""Chạy bản Python của repo đặc tả trên một vault và ghi kết quả ra một thư mục dump trung lập ngôn ngữ.

Chỉ chạy trên máy (CI không có Python). Định dạng dump: parity/README.md, mục "Định dạng dump". Phần so là
`cargo run -p pkm-parity -- compare`; script này không so gì.

  python3 parity/dump-python.py --spec SPEC_CHECKOUT --vault VAULT --out DIR [--packs business]
         [--adapter e11] [--no-aliases] [--edge-keys links] [--body-edges] [--today 2026-10-15]
         [--replay DUMP] [--listing sorted|os|reverse]

--replay DUMP  chạy lại đúng bộ câu tool và lệnh CLI của một dump khác thay vì tự sinh (bên thứ hai của một lần so).
--listing      thứ tự liệt kê note trước khi build (mặc định sorted: theo đường dẫn, như NTFS và APFS trả). Python
               chèn dòng theo thứ tự liệt kê của hệ điều hành, nên số thứ tự cạnh (`edges.id`) và kết quả `related`
               (thứ tự đẩy PageRank, đường `via`) đổi theo máy; ghim thứ tự thì golden dựng lại được ở mọi máy và
               bản Rust có một thứ tự xác định để theo. `os` giữ thứ tự của hệ điều hành, `reverse` đảo thứ tự đã
               sắp: chỉ để xem phần nào của kết quả phụ thuộc thứ tự này.

Index dựng vào thư mục tạm ngoài vault; script không ghi gì vào vault.
"""
import argparse, copy, datetime, hashlib, json, os, re, shutil, statistics, subprocess, sys, tempfile, time

FORMAT = 1
DUMP_DIRS = ('tables', 'answers', 'tools', 'mcp', 'cli')
MCP_PROTOCOL = '2025-06-18'
PAST_DAY = '2025-06-01'
PICKS_PER_TYPE = 3


def parse_args(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--spec', required=True, help='thư mục checkout repo đặc tả (obsidian-pkm-automation) ở sha cần chạy')
    ap.add_argument('--vault', required=True)
    ap.add_argument('--out', required=True, help='thư mục dump (tạo mới; dump cũ ở đó bị thay)')
    ap.add_argument('--packs', default='business', help='gói schema, cách nhau bằng dấu phẩy (mặc định business, như test kg-solo)')
    ap.add_argument('--adapter', help='adapter của vault (e11 cho bản B)')
    ap.add_argument('--no-aliases', action='store_true')
    ap.add_argument('--edge-keys', choices=['schema', 'links'], default='schema')
    ap.add_argument('--body-edges', action='store_true')
    ap.add_argument('--today', help='ngày "hôm nay" (mặc định: `today` của .kg-answers.json, không có thì ngày máy)')
    ap.add_argument('--about', help='câu mô tả vault cho instructions MCP (mặc định như Python: mô tả gói)')
    ap.add_argument('--replay', help='dump có sẵn: chạy lại đúng bộ câu tool và lệnh CLI của nó')
    ap.add_argument('--listing', choices=['sorted', 'os', 'reverse'], default='sorted')
    return ap.parse_args(argv)


def pin_listing(index_mod, mode):
    """Đặt thứ tự liệt kê note của `build` (xem --listing)."""
    if mode == 'os':
        return
    listing = index_mod.list_notes
    def ordered(vault, skip=None):
        items = sorted(listing(vault, skip).items())
        return dict(reversed(items) if mode == 'reverse' else items)
    index_mod.list_notes = ordered


def kg_cli(argv):
    """`dump-python.py kg-cli SPEC LISTING VAULT ...`: chạy dòng lệnh kg của repo đặc tả (như `python3 scripts/kg`)
    với thứ tự liệt kê đã ghim."""
    spec, mode, rest = argv[0], argv[1], argv[2:]
    sys.path.insert(0, os.path.join(spec, 'scripts'))
    import kg.index as index_mod
    from kg.cli import main as cli_main
    pin_listing(index_mod, mode)
    for s in (sys.stdout, sys.stderr):           # như platform_paths.utf8_stdio (sha cũ chưa có module đó)
        s.reconfigure(encoding='utf-8')
    cli_main(rest)


# ------------------------------------------------------------------------------------------------ chung
class Scrub:
    """Thay đường dẫn tuyệt đối của vault và các DB trong mọi chuỗi đầu ra bằng `<VAULT>`, `<DB>` (golden không phụ
    thuộc máy dựng)."""

    def __init__(self, vault, dbs):
        pairs = []
        for path, mark in [(db, '<DB>') for db in dbs] + [(vault, '<VAULT>')]:
            for p in {os.path.abspath(path), os.path.realpath(path)}:
                pairs.append((p, mark))
        self.pairs = sorted(pairs, key=lambda x: -len(x[0]))       # đường dài trước: DB có thể nằm trong vault

    def text(self, s):
        for p, mark in self.pairs:
            s = s.replace(p, mark)
        return s

    def __call__(self, x):
        if isinstance(x, str):
            return self.text(x)
        if isinstance(x, dict):
            return {self.text(k) if isinstance(k, str) else k: self(v) for k, v in x.items()}
        if isinstance(x, (list, tuple)):
            return [self(v) for v in x]
        return x


def plain(x):
    """Kết quả Python thành JSON thuần, như MCP server ghi (`default=str`)."""
    return json.loads(json.dumps(x, ensure_ascii=False, default=str))


def line(obj, sort_keys=True):
    """Một dòng JSONL. Khoá sắp (thứ tự khoá object không phải hành vi, và `by_type` của Python có thứ tự theo hash
    ngẫu nhiên); dòng bảng giữ thứ tự cột (sort_keys=False)."""
    return json.dumps(obj, ensure_ascii=False, sort_keys=sort_keys) + '\n'


# Giá trị đổi theo lần chạy (thời điểm, thời gian dựng): ghi thành VOLATILE để golden dựng lại không đổi byte nào.
# Bộ so khớp vẫn bỏ qua các giá trị này ở cả hai bên (src/normalize.rs), nên dump của Rust không cần làm theo.
VOLATILE = '<volatile>'
VOLATILE_META = ('built_at', 'ms')
VOLATILE_CLI = (re.compile(r'(; ms )\{[^}]*\}'), re.compile(r'("(?:built_at|ms)": )"(?:[^"\\]|\\.)*"'))


def mask_cli(text):
    text = VOLATILE_CLI[0].sub(lambda m: m.group(1) + '{' + VOLATILE + '}', text)
    return VOLATILE_CLI[1].sub(lambda m: m.group(1) + '"' + VOLATILE + '"', text)


def write_json(path, obj):
    with open(path, 'w', encoding='utf-8', newline='\n') as f:
        json.dump(obj, f, ensure_ascii=False, indent=2, sort_keys=True)
        f.write('\n')


def write_text(path, s):
    with open(path, 'w', encoding='utf-8', newline='') as f:
        f.write(s)


def git_head(spec):
    try:
        r = subprocess.run(['git', '-C', spec, 'rev-parse', 'HEAD'], capture_output=True, text=True, check=True)
        return r.stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return None


def prepare_out(out):
    """Thư mục tạm `OUT.partial` để ghi dump; chỉ thay `OUT` khi dump xong (dump hỏng giữa chừng không bao giờ trông
    như dump đủ). `OUT` có sẵn chỉ được thay khi nó là dump (có manifest.json), để không xoá nhầm thư mục khác."""
    if os.path.exists(out):
        if not os.path.isdir(out):
            sys.exit(f'--out {out} không phải thư mục')
        if os.listdir(out) and not os.path.exists(os.path.join(out, 'manifest.json')):
            sys.exit(f'--out {out} có file nhưng không phải dump (thiếu manifest.json); không xoá')
    staging = out + '.partial'
    shutil.rmtree(staging, ignore_errors=True)
    for d in DUMP_DIRS:
        os.makedirs(os.path.join(staging, d))
    return staging


def publish(staging, out):
    if os.path.exists(out):
        shutil.rmtree(out)
    os.replace(staging, out)


# ------------------------------------------------------------------------------------------------ tầng bảng
def dump_tables(connect, db_path, vault, out, scrub):
    """Mọi bảng của index (trừ bảng nội bộ sqlite_*) và view v3_deviations: mỗi dòng một object JSON theo thứ tự cột,
    các dòng sắp theo mọi cột. `nodes.mtime` ghi thành độ lệch (ms) so với mtime đọc từ đĩa lúc dump."""
    db = connect(db_path, readonly=True)
    try:
        names = [r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")]
        views = [r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type = 'view' AND name = 'v3_deviations'")]
        info = {}
        for name in names + views:
            cols = db.execute(f'PRAGMA table_info("{name}")').fetchall()
            columns = [c[1] for c in cols]
            key = [c[1] for c in sorted((c for c in cols if c[5]), key=lambda c: c[5])]
            order = ', '.join(str(i + 1) for i in range(len(columns)))
            rows = db.execute(f'SELECT * FROM "{name}" ORDER BY {order}').fetchall()
            with open(os.path.join(out, 'tables', name + '.jsonl'), 'w', encoding='utf-8', newline='\n') as f:
                for r in rows:
                    d = dict(zip(columns, r))
                    if name == 'nodes' and isinstance(d.get('mtime'), int):
                        d['mtime'] = d['mtime'] - disk_mtime_ms(vault, d['id'])
                    if name == 'meta' and d.get('key') in VOLATILE_META:
                        d['value'] = VOLATILE
                    f.write(line(scrub(d), sort_keys=False))
            info[name] = {'kind': 'view' if name in views else 'table', 'columns': columns, 'key': key, 'rows': len(rows)}
        return info
    finally:
        db.close()


def disk_mtime_ms(vault, node):
    """mtime của file note như vault.list_notes đọc (ms, cắt phần lẻ)."""
    return int(os.stat(os.path.join(vault, node)).st_mtime * 1000)


# ------------------------------------------------------------------------------------------------ tầng câu đáp án
def dump_answers(fixture_check, Graph, vault, db_path, out, scrub):
    """Mọi câu graph và bảng của `.kg-answers.json`, chạy như fixture_check.py của repo đặc tả (cùng hàm `run`)."""
    path = os.path.join(vault, '.kg-answers.json')
    if not os.path.exists(path):
        return None
    with open(path, encoding='utf-8') as f:
        key = json.load(f)
    g = Graph(db_path)
    n = 0
    try:
        with open(os.path.join(out, 'answers', 'answers.jsonl'), 'w', encoding='utf-8', newline='\n') as f:
            for c in key['cases']:
                qs = c.get('queries') or [c['query']]
                for i, q in enumerate(qs):
                    if q['op'] not in fixture_check.GRAPH_OPS | fixture_check.TABLE_OPS:
                        continue
                    rec = {'id': f"{c['id']}#{i}", 'op': q['op'], 'query': q}
                    try:
                        rec['result'] = plain(fixture_check.run(g, q))
                    except Exception as e:  # noqa: BLE001  (lỗi cũng là hành vi cần khớp)
                        rec['raised'] = type(e).__name__
                    f.write(line(scrub(rec)))
                    n += 1
    finally:
        g.close()
    check = fixture_check.check(vault, db_path)
    return {'queries': n, 'fixture_check': {'graph': check['graph'], 'table': check['table'], 'fails': len(check['fails'])}}


# ------------------------------------------------------------------------------------------------ bộ câu tool
def pick_nodes(db):
    """Mỗi type vài note, chọn theo sha1 của id (rải đều, không phụ thuộc thứ tự build)."""
    by_type = {}
    for nid, typ in db.execute('SELECT id, type FROM nodes'):
        by_type.setdefault(typ or '', []).append(nid)
    picks = []
    for typ in sorted(by_type):
        ids = sorted(by_type[typ], key=lambda i: hashlib.sha1(i.encode('utf-8')).hexdigest())
        picks.extend((typ, i) for i in ids[:PICKS_PER_TYPE])
    return picks


def generate_calls(server, today):
    """Bộ câu sinh tự động: mỗi tool vài tham số, gồm as_of, statuses, accepted_only, system, limit. Trả danh sách
    (tên tool, tham số); `more` thêm lúc chạy, ngay sau câu có `handle`."""
    db = server.tools.g.db
    picks = pick_nodes(db)
    title = dict(db.execute('SELECT id, title FROM nodes'))
    calls = []
    add = lambda tool, **a: calls.append((tool, a))
    for typ, node in picks:
        add('note', node=node)
        add('note', node=node, body_chars=0)
        add('find', name=title[node])
        add('find', name=title[node].split(' ')[0], type=typ or None, limit=5)
        add('neighbors', node=node)
        add('neighbors', node=node, dir='in', accepted_only=True)
        add('neighbors', node=node, dir='out', statuses=['proposed'])
        add('neighbors', node=node, as_of=today, limit=3)
        add('neighbors', node=node, as_of=PAST_DAY, system=True)
        add('reach', node=node)
        add('reach', node=node, dir='in', max_depth=2)
        add('reach', node=node, dir='both', max_depth=3, as_of=today, accepted_only=True, limit=5)
        add('loops', node=node, max_len=4)
        add('provenance', claim=node)
    # paths: cặp (note, note tới được trong 3 bước)
    pairs = []
    for typ, node in picks:
        far = [i for i, d in server.tools.g.reach(node, max_depth=3) if d and d >= 2]
        if far:
            pairs.append((node, far[len(far) // 2]))
    for a, b in pairs:
        add('paths', **{'from': a, 'to': b})
        add('paths', **{'from': a, 'to': b, 'max_len': 3, 'accepted_only': True, 'k': 3})
        add('paths', **{'from': a, 'to': b, 'as_of': today, 'statuses': ['accepted', 'superseded']})
    types = [t for (t,) in db.execute('SELECT DISTINCT type FROM nodes WHERE type IS NOT NULL ORDER BY 1')]
    for t in types:
        add('select', type=t)
    for s in ('proposed', 'accepted', 'superseded', 'rejected'):
        add('select', status=s, limit=5)
    add('select', proposed_by='llm')
    add('select', type='claim', status='proposed', system=True)
    add('select', where={'confidence': 'established'})
    vtypes = [t for (t,) in db.execute('SELECT DISTINCT type FROM nodes WHERE valid_from IS NOT NULL AND type IS NOT NULL ORDER BY 1')]
    for t in vtypes:
        add('valid', type=t, as_of=today)
        add('valid', type=t, as_of=PAST_DAY, limit=2)
    # related: hạt giống là các note đã chọn, có và không có điểm
    seeds = [n for _, n in picks]
    for i in range(0, len(seeds), 4):
        group = seeds[i:i + 4]
        add('related', hits=[{'path': p, 'score': round(1.0 / (j + 1), 3)} for j, p in enumerate(group)])
    add('related', hits=seeds[:3], k=5)
    add('related', hits=seeds[3:6], accepted_only=True, types=types[:3])
    add('related', hits=[[seeds[0], 0.9], 'Không có note này.md'], as_of=today, statuses=['accepted'])
    # lỗi người gọi hay gặp
    add('neighbors', node='Không có note này.md')
    add('note', node=title[picks[0][1]])
    add('loops', node=picks[0][1])
    add('select')
    add('more', handle='find-0000000000')
    add('related')
    return calls


def run_calls(server, calls, out, scrub, replay):
    """Chạy từng câu qua `Server.call` (điểm vào chung của tool, trước khi MCP thu gọn JSON). Ghi tools/calls.jsonl."""
    ms = {}
    n = 0
    queue = list(calls)
    with open(os.path.join(out, 'tools', 'calls.jsonl'), 'w', encoding='utf-8', newline='\n') as f:
        while queue:
            tool, args = queue.pop(0)
            if not replay:
                args = {k: v for k, v in args.items() if v is not None}
            rec = {'id': f'{n:04d}-{tool}', 'tool': tool, 'args': args}
            t0 = time.perf_counter()
            try:
                res = server.call(tool, copy.deepcopy(args))
                rec['result'] = plain(res)
            except Exception as e:  # noqa: BLE001
                rec['raised'] = type(e).__name__
                res = None
            ms.setdefault(tool, []).append((time.perf_counter() - t0) * 1000)
            f.write(line(scrub(rec)))
            n += 1
            if not replay and isinstance(res, dict) and res.get('handle') and tool != 'more':
                shown = len(res.get('items') or [])
                queue.insert(0, ('more', {'handle': res['handle'], 'offset': shown, 'limit': 5}))
    return n, ms


# ------------------------------------------------------------------------------------------------ tầng MCP
def dump_mcp(server, out, scrub):
    init = server.handle({'jsonrpc': '2.0', 'id': 1, 'method': 'initialize',
                          'params': {'protocolVersion': MCP_PROTOCOL, 'capabilities': {}, 'clientInfo': {'name': 'pkm-parity', 'version': '1'}}})
    result = dict(init['result'])
    write_text(os.path.join(out, 'mcp', 'instructions.txt'), scrub(result.pop('instructions')) + '\n')
    write_json(os.path.join(out, 'mcp', 'initialize.json'), scrub(result))
    tl = server.handle({'jsonrpc': '2.0', 'id': 2, 'method': 'tools/list'})
    write_json(os.path.join(out, 'mcp', 'tools_list.json'), scrub(tl['result']))


# ------------------------------------------------------------------------------------------------ tầng CLI
def generate_cli(build_flags, calls, today, db):
    """Mỗi lệnh của mục 10 PRD, tham số lấy từ bộ câu tool đã sinh. Trả [{id, args}] (không có VAULT và --db)."""
    first = lambda tool, key: next((a[key] for t, a in calls if t == tool and key in a and isinstance(a[key], str)), None)
    node = first('neighbors', 'node')
    pair = next(((a['from'], a['to']) for t, a in calls if t == 'paths'), None)
    claim = next((i for (i,) in db.execute("SELECT id FROM nodes WHERE type = 'claim' ORDER BY id LIMIT 1")), None) or node
    rule = next((r for (r,) in db.execute('SELECT rule FROM v3_deviations GROUP BY rule ORDER BY count(*) DESC, rule LIMIT 1')), 'R1')
    vtype = next((t for (t,) in db.execute('SELECT type FROM nodes WHERE valid_from IS NOT NULL AND type IS NOT NULL ORDER BY type LIMIT 1')), 'contract')
    name = first('find', 'name') or 'x'
    cmds = [
        ('build-full', ['build', '--full', *build_flags]),
        ('build-noop', ['build']),
        ('build-json', ['build', '--json']),
        ('stats', ['stats']),
        ('validate', ['validate']),
        ('validate-json', ['validate', '--json']),
        ('validate-rule', ['validate', '--rule', rule, '--limit', '2', '--json']),
        ('find', ['find', name]),
        ('find-type', ['find', name.split(' ')[0], '--type', vtype]),
        ('note', ['note', node]),
        ('neighbors', ['neighbors', node, '--dir', 'in']),
        ('neighbors-filter', ['neighbors', node, '--accepted', '--as-of', today, '--limit', '3']),
        ('reach', ['reach', node, '--max-depth', '2']),
        ('loops', ['loops', node, '--max-len', '4']),
        ('loops-no-max-len', ['loops', node]),
        ('provenance', ['provenance', claim]),
        ('select', ['select', 'status=proposed', 'proposed_by=llm']),
        ('select-type', ['select', f'type={vtype}', '--system']),
        ('valid', ['valid', vtype, '--as-of', today]),
    ]
    if pair:
        cmds.append(('paths', ['paths', pair[0], pair[1], '--max-len', '4']))
        cmds.append(('paths-statuses', ['paths', pair[0], pair[1], '--statuses', 'accepted,proposed']))
    return [{'id': i, 'args': a} for i, a in cmds]


def run_cli(spec, listing, vault, cli_db, commands, out, scrub):
    env = dict(os.environ, PYTHONIOENCODING='utf-8')
    done = []
    for c in commands:
        argv = [sys.executable, os.path.abspath(__file__), 'kg-cli', spec, listing, vault, *c['args'], '--db', cli_db]
        r = subprocess.run(argv, capture_output=True, env=env)
        stdout = mask_cli(scrub(r.stdout.decode('utf-8', 'replace')))
        kind = 'text'
        try:
            if stdout.strip():
                json.loads(stdout)
                kind = 'json'
        except ValueError:
            pass
        write_text(os.path.join(out, 'cli', c['id'] + '.out'), stdout)
        rec = {'id': c['id'], 'args': c['args'], 'exit': r.returncode, 'stdout': c['id'] + '.out', 'kind': kind}
        if r.returncode != 0:
            write_text(os.path.join(out, 'cli', c['id'] + '.err'), scrub(r.stderr.decode('utf-8', 'replace')))
            rec['stderr'] = c['id'] + '.err'
        done.append(rec)
    write_json(os.path.join(out, 'cli', 'commands.json'), done)
    return len(done)


# ------------------------------------------------------------------------------------------------ chính
def main(argv=None):
    a = parse_args(argv)
    spec = os.path.abspath(a.spec)
    vault = os.path.abspath(a.vault)
    out = os.path.abspath(a.out)
    if not os.path.isdir(os.path.join(spec, 'scripts', 'kg')):
        sys.exit(f'--spec {spec}: không thấy scripts/kg (cần checkout repo đặc tả)')
    if not os.path.isdir(vault):
        sys.exit(f'--vault {vault}: không phải thư mục')
    if out == vault or out.startswith(os.path.join(vault, '')):
        sys.exit('--out không được nằm trong vault')
    replay = None
    if a.replay:
        with open(os.path.join(a.replay, 'tools', 'calls.jsonl'), encoding='utf-8') as f:
            replay_calls = [json.loads(x) for x in f if x.strip()]
        with open(os.path.join(a.replay, 'cli', 'commands.json'), encoding='utf-8') as f:
            replay_cli = json.load(f)
        replay = ([(r['tool'], r['args']) for r in replay_calls], [{'id': r['id'], 'args': r['args']} for r in replay_cli])

    sys.path.insert(0, os.path.join(spec, 'scripts'))
    sys.path.insert(0, os.path.join(spec, 'scripts', 'kg', 'tests'))
    import kg.index as index_mod  # noqa: E402
    from kg import Graph, Options  # noqa: E402
    from kg import mcp_server  # noqa: E402
    import fixture_check  # noqa: E402

    pin_listing(index_mod, a.listing)

    final, out = out, prepare_out(out)
    work = tempfile.mkdtemp(prefix='pkm-parity-')
    try:
        db_path = os.path.join(work, 'graph.db')
        cli_db = os.path.join(work, 'cli.db')
        scrub = Scrub(vault, [db_path, cli_db])
        answers_path = os.path.join(vault, '.kg-answers.json')
        today = a.today
        if not today and os.path.exists(answers_path):
            with open(answers_path, encoding='utf-8') as f:
                today = json.load(f).get('today')
        today = today or datetime.date.today().isoformat()
        packs = [p for p in a.packs.split(',') if p]
        opts = Options(aliases=not a.no_aliases, edge_keys=a.edge_keys, body_edges=a.body_edges)
        build_flags = ['--packs', ','.join(packs)] + (['--adapter', a.adapter] if a.adapter else []) + \
                      (['--no-aliases'] if a.no_aliases else []) + ['--edge-keys', a.edge_keys] + (['--body-edges'] if a.body_edges else [])
        perf = {}

        t0 = time.perf_counter()
        index_mod.build(vault, db_path, packs, opts, full=True, adapter=index_mod.load_adapter(a.adapter))
        perf['build_full_ms'] = round((time.perf_counter() - t0) * 1000, 1)
        t0 = time.perf_counter()
        info = index_mod.build(vault, db_path, packs, opts, adapter=index_mod.load_adapter(a.adapter))
        perf['build_noop_ms'] = round((time.perf_counter() - t0) * 1000, 1)
        if info['notes_read'] != '0':
            sys.exit(f'build lần hai đọc lại {info["notes_read"]} note: vault đang bị sửa trong lúc dump?')

        tables = dump_tables(index_mod.connect, db_path, vault, out, scrub)
        answers = dump_answers(fixture_check, Graph, vault, db_path, out, scrub)

        margs = [vault, 'mcp', '--db', db_path, '--refresh-every', '-1', '--today', today, '--packs', ','.join(packs),
                 '--edge-keys', a.edge_keys, '--adapter', a.adapter or 'none']
        margs += (['--no-aliases'] if a.no_aliases else []) + (['--body-edges'] if a.body_edges else []) + (['--about', a.about] if a.about else [])
        server = mcp_server.Server(mcp_server.parser().parse_args(margs))
        server.log = lambda msg: None
        server.ready.wait()
        if server.tools is None:
            sys.exit(f'MCP server không mở được index: {server.build_error}')
        dump_mcp(server, out, scrub)
        calls = replay[0] if replay else generate_calls(server, today)
        n_calls, tool_ms = run_calls(server, calls, out, scrub, replay is not None)
        perf['tool_ms'] = {k: {'calls': len(v), 'median': round(statistics.median(v), 2), 'total': round(sum(v), 1)} for k, v in sorted(tool_ms.items())}
        server.a.refresh_every = 0
        t0 = time.perf_counter()
        with server.lock:
            server.refresh()
        perf['mcp_refresh_ms'] = round((time.perf_counter() - t0) * 1000, 1)

        if replay:
            commands = replay[1]
        else:
            g = Graph(db_path)
            try:
                commands = generate_cli(build_flags, calls, today, g.db)
            finally:
                g.close()
        n_cli = run_cli(spec, a.listing, vault, cli_db, commands, out, scrub)
        server.tools.g.close()

        fx = None
        fm = os.path.join(vault, '.fixture-manifest.json')
        if os.path.exists(fm):
            with open(fm, encoding='utf-8') as f:
                m = json.load(f)
            fx = {'preset': m.get('preset'), 'version': m.get('version'), 'seed': (m.get('options') or {}).get('seed'),
                  'tree_hash': m.get('treeHash'), 'files': m.get('files')}
        write_json(os.path.join(out, 'perf.json'), perf)
        write_json(os.path.join(out, 'manifest.json'), {
            'format': FORMAT,
            'implementation': 'python',
            'spec_sha': git_head(spec),
            'vault': {'name': os.path.basename(vault.rstrip('/\\')), 'fixture': fx},
            'run': {'packs': ','.join(packs), 'adapter': a.adapter, 'no_aliases': a.no_aliases, 'edge_keys': a.edge_keys,
                    'body_edges': a.body_edges, 'today': today, 'about': a.about, 'listing': a.listing},
            'tables': tables,
            'answers': answers,
            'calls': n_calls,
            'cli': n_cli,
            'not_available': [],
        })
        publish(out, final)
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    if sys.argv[1:2] == ['kg-cli']:
        kg_cli(sys.argv[2:])
    else:
        main()
