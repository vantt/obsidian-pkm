#!/usr/bin/env python3
"""Writes python-proposals.json: what `propose.py --apply`, `--stats` and `--revert` (research repo, commit 9061581)
do to an invented vault, so the plugin's port can be compared with it.

  python3 make-python-proposals.py RESEARCH_REPO

The vault is the invented propose fixture of pkm-core (crates/pkm-core/tests/propose/vault) plus one topic page.
Proposals come from `propose.py --create`; some are then marked nhận or bỏ, as the user would.
"""
import datetime, json, os, shutil, sys, tempfile, types

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, '..', '..', '..', '..'))
FIXTURE = os.path.join(REPO, 'crates', 'pkm-core', 'tests', 'propose')
NOW = datetime.datetime(2026, 11, 4, 11, 30, 5)


class FixedClock(datetime.datetime):
    @classmethod
    def now(cls, tz=None):
        return cls(NOW.year, NOW.month, NOW.day, NOW.hour, NOW.minute, NOW.second)


class FixedDay(datetime.date):
    @classmethod
    def today(cls):
        return cls(NOW.year, NOW.month, NOW.day)


def snapshot(vault):
    out = {}
    for dp, dn, fn in os.walk(vault):
        dn[:] = sorted(d for d in dn if d != '.obsidian')
        for f in sorted(fn):
            p = os.path.join(dp, f)
            with open(p, encoding='utf-8', newline='') as fh:
                out[os.path.relpath(p, vault).replace(os.sep, '/')] = fh.read()
    return out


def main(research):
    sys.path.insert(0, os.path.join(research, 'scripts'))
    sys.path.insert(0, os.path.join(research, 'scripts', 'e11'))
    import propose as P
    P.datetime = types.SimpleNamespace(datetime=FixedClock, date=FixedDay)

    vault = os.path.join(tempfile.mkdtemp(prefix='proposals-'), 'vault')
    shutil.copytree(os.path.join(FIXTURE, 'vault'), vault)
    os.makedirs(os.path.join(vault, '.obsidian'))
    with open(os.path.join(vault, 'Ghi chép', 'Ghi chép (chủ đề).md'), 'w', encoding='utf-8') as f:
        f.write('---\ntype: toc\n---\nMục lục ghi chép.\n')
    P.create(vault, os.path.join(FIXTURE, 'similar.json'), True)

    # The user's decisions: accept everything except one rejected fix-link (reason), one rejected type-batch
    # (reason under its Vietnamese key) and one related left waiting.
    q = os.path.join(vault, P.QUEUE)
    seen = {}
    for f in sorted(os.listdir(q)):
        p = os.path.join(q, f)
        with open(p, encoding='utf-8', newline='') as fh:
            text = fh.read()
        kind = P.split(text)[0]['kind']
        seen[kind] = seen.get(kind, 0) + 1
        if kind == 'fix-link' and seen[kind] == 2:
            text = text.replace('status: chờ\n', 'status: bỏ\nreject_reason: tên gần giống nhưng khác người\n', 1)
        elif kind == 'type-batch' and seen[kind] == 1:
            text = text.replace('status: chờ\n', 'status: bỏ\nlý do bỏ: không phải bài cắt\n', 1)
        elif kind == 'related' and seen[kind] == 2:
            continue
        else:
            text = text.replace('status: chờ\n', 'status: nhận\n', 1)
        with open(p, 'w', encoding='utf-8', newline='') as fh:
            fh.write(text)
    before = snapshot(vault)

    P.apply(vault)
    after = snapshot(vault)

    names = sorted(os.listdir(q))
    first, second, third = (f'{P.QUEUE}/{n}' for n in names[:3])
    src = 'Cũ/Hoi thao/Hoi thao (chủ đề).md'
    events = [
        {'at': '2026-11-04T04:00:00.000Z', 'event': 'open', 'path': first},
        {'at': '2026-11-04T04:00:20.000Z', 'event': 'open', 'path': src},
        {'at': '2026-11-04T04:01:10.500Z', 'event': 'status', 'path': first, 'status': 'nhận'},
        {'at': '2026-11-04T04:02:00.000Z', 'event': 'open', 'path': second},
        {'at': '2026-11-04T04:02:30.000Z', 'event': 'status', 'path': second, 'status': 'bỏ'},
        {'at': '2026-11-04T04:03:00.000Z', 'event': 'status', 'path': third, 'status': 'nhận'},
        {'at': '2026-11-04T04:04:00.000Z', 'event': 'open', 'path': third},
    ]
    review_log = ''.join(json.dumps(e, ensure_ascii=False) + '\n' for e in events)
    with open(os.path.join(vault, P.REVIEW_LOG), 'w', encoding='utf-8', newline='') as fh:
        fh.write(review_log)
    P.stats(vault)
    with open(os.path.join(vault, P.STATS), encoding='utf-8', newline='') as fh:
        stats = fh.read()
    os.remove(os.path.join(vault, P.STATS))
    os.remove(os.path.join(vault, P.REVIEW_LOG))

    P.revert(vault)
    reverted = snapshot(vault)

    out = {'source': 'scripts/e11/propose.py at 9061581', 'now': NOW.isoformat(), 'before': before, 'after': after,
           'reviewLog': review_log, 'stats': stats, 'reverted': reverted}
    with open(os.path.join(HERE, 'python-proposals.json'), 'w', encoding='utf-8') as fh:
        json.dump(out, fh, ensure_ascii=False, indent=1)
        fh.write('\n')
    shutil.rmtree(os.path.dirname(vault))


if __name__ == '__main__':
    main(sys.argv[1])
