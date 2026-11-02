"""Dump every note's frontmatter exactly as the pinned spec reads it, as typed JSON lines.

Usage: python3 -I pyyaml-dump.py --spec <spec repo> <vault>   (or a single .md file instead of a vault)

The note list, the frontmatter split, the YAML loader and the props come from the spec's own
`scripts/kg/vault.py` (imported, not copied), so this is the reference side of the comparison.

First line: {"meta": {"loader": "CSafeLoader" | "SafeLoader", "pyyaml": ver, "python": ver}}.
Then one JSON object per note, sorted by path:
  {"path": str, "fm": typed, "error": str|null, "props": [[key, str|null], ...], "crash": null}
  or, when `vault.parse` raises (the spec's indexer would stop):  {..., "crash": "<exception type>"}
typed values: ["null"] ["bool", b] ["int", "123"] ["float", repr] ["str", s] ["bytes", repr]
              ["date", iso] ["datetime", iso] ["list", [...]] ["tuple", [...]] ["set", [...]]
              ["map", [[k, v], ...]]
"""
import argparse
import datetime
import json
import os
import sys


def enc(v):
    if v is None:
        return ['null']
    if isinstance(v, bool):
        return ['bool', v]
    if isinstance(v, int):
        return ['int', str(v)]
    if isinstance(v, float):
        return ['float', repr(v)]
    if isinstance(v, str):
        return ['str', v]
    if isinstance(v, bytes):
        return ['bytes', repr(v)]
    if isinstance(v, datetime.datetime):        # before date: datetime is a subclass of date
        return ['datetime', v.isoformat()]
    if isinstance(v, datetime.date):
        return ['date', v.isoformat()]
    if isinstance(v, list):
        return ['list', [enc(x) for x in v]]
    if isinstance(v, tuple):
        return ['tuple', [enc(x) for x in v]]
    if isinstance(v, (set, frozenset)):
        return ['set', sorted((enc(x) for x in v), key=lambda e: json.dumps(e, ensure_ascii=False, separators=(',', ':')))]
    if isinstance(v, dict):
        return ['map', [[enc(k), enc(x)] for k, x in v.items()]]
    raise TypeError(f'unexpected YAML value type {type(v).__name__}')


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--spec', required=True, help='spec repo checkout at the pinned sha')
    ap.add_argument('vault', help='vault directory, or one .md file')
    a = ap.parse_args()
    kg = os.path.join(os.path.abspath(a.spec), 'scripts', 'kg')
    if not os.path.isfile(os.path.join(kg, 'vault.py')):
        sys.exit(f'not a spec repo: {kg}/vault.py missing')
    sys.path[:0] = [kg, os.path.dirname(kg)]      # vault.py imports scripts/platform_paths.py, as kg/__main__.py sets up
    import vault                                 # noqa: E402  (path set above)

    if os.path.isfile(a.vault):
        notes = [(os.path.basename(a.vault), a.vault)]
    elif os.path.isdir(a.vault):
        notes = sorted((rel, fs) for rel, (fs, _, _) in vault.list_notes(a.vault).items())
    else:
        sys.exit(f'no such vault: {a.vault}')
    out = sys.stdout
    # vault.LOADER subclasses CSafeLoader when PyYAML has libyaml, else the pure-Python SafeLoader,
    # whose error texts (and a few edge parses) differ. The Rust side copies the CSafeLoader path.
    import platform
    import yaml
    meta = {'loader': vault.LOADER.__mro__[1].__name__, 'pyyaml': yaml.__version__,
            'python': platform.python_version()}
    out.write(json.dumps({'meta': meta}) + '\n')
    for rel, fs in notes:
        try:
            p = vault.parse(vault.read(fs))
            rec = {'path': rel, 'fm': enc(p.fm), 'error': p.error, 'props': [list(x) for x in p.props], 'crash': None}
        except Exception as e:                    # the spec's indexer would stop here; record the type only
            rec = {'path': rel, 'fm': None, 'error': None, 'props': [], 'crash': type(e).__name__}
        out.write(json.dumps(rec, ensure_ascii=False) + '\n')


if __name__ == '__main__':
    main()
