"""Write N random frontmatter notes built from YAML tokens that 1.1 and 1.2 readers treat differently.

Usage: python3 -I make-fuzz.py <out dir> [N] [seed]
Then: cargo run --release -- compare --spec <spec repo> <out dir>
A differential check of the PyYAML layer against the real PyYAML; the output dir is scratch, not committed.
"""
import os
import random
import shutil
import sys
import unicodedata

SCALARS = [
    'yes', 'No', 'on', 'OFF', 'y', 'n', 'true', 'False', '~', 'null', 'Null', 'nULL', '', '=', '<<',
    '010', '0o17', '08', '0912345678', '1_000', '0x1F', '0x_', '0b101', '+12', '-0', '00', '0_',
    '10:30', '09:30', '1:30:00', '190:20:30.15', '1.5', '1e3', '1.0e+3', '.5', '1.', '.inf', '-.Inf', '.nan',
    '1_000.5', '-0.0', '3.0', '1.2.3', '12345678901234567.0', '1.0e+400', '0.0001', '1.0e-5',
    '2026-01-05', '2026-01-05 10:00', '2026-01-05T10:00:00Z', '2026-13-01', '2026-1-5',
    'Tiếng Việt', unicodedata.normalize('NFD', 'Tiếng Việt'), 'đ', '😀', 'a​b',
    '[[Note A]]', '[[B|alias]]', '"[[C#h]]"', "'it''s'", '"say \\"hi\\""', '"\\x41\\u00e9\\t"',
    'http://x.y/a', 'plain text', 'v1.0', '50%', '#tag', '"#tag"', 'a #c', 'd#e', '"x, y"',
    '!!str 12', '!!int "7"', '!!float "2"', '!!bool "on"', '!!null ""', '! 12', '!!timestamp 2026-01-05',
    '!!binary aGk=', '!local x', '*a1', '&a1 anchored', 'b: c', '`tick`', '@at', '%pct', '>-', '|',
]
KEYS = ['type', 'status', 'tags', 'aliases', 'title', 'date', 'up', 'yes', 'on', '1', '1.0', 'true', '~',
        '2026-01-05', 'Tiếng', '<<', '=', '"quoted key"', 'meta', 'dup', 'dup']


def scalar(r):
    s = r.choice(SCALARS)
    if s in ('>-', '|'):
        return s + '\n  line one\n  line two'
    return s


def value(r, depth):
    k = r.random()
    if depth < 2 and k < 0.15:
        return '[' + ', '.join(scalar(r) for _ in range(r.randint(0, 3))) + ']'
    if depth < 2 and k < 0.25:
        return '{' + ', '.join(f'{r.choice(KEYS)}: {scalar(r)}' for _ in range(r.randint(0, 3))) + '}'
    if depth < 2 and k < 0.35:
        return '\n' + '\n'.join(f'{"  " * (depth + 1)}- {scalar(r)}' for _ in range(r.randint(1, 3)))
    if depth < 2 and k < 0.42:
        return '\n' + '\n'.join(f'{"  " * (depth + 1)}{r.choice(KEYS)}: {value(r, depth + 1)}'
                                for _ in range(r.randint(1, 3)))
    return scalar(r)


def note(r):
    lines = [f'{r.choice(KEYS)}: {value(r, 0)}' for _ in range(r.randint(1, 6))]
    body = '\n'.join(lines) + '\n'
    if r.random() < 0.1:
        body = body.replace('\n', '\r\n')
    nl = '\r\n' if '\r\n' in body else '\n'
    return '---' + nl + body + '---' + nl + 'body' + nl


def main():
    out = sys.argv[1]
    n = int(sys.argv[2]) if len(sys.argv) > 2 else 3000
    r = random.Random(int(sys.argv[3]) if len(sys.argv) > 3 else 1)
    if os.path.isdir(out):
        shutil.rmtree(out)
    os.makedirs(out)
    for i in range(n):
        with open(os.path.join(out, f'f{i:05}.md'), 'w', encoding='utf-8', newline='') as f:
            f.write(note(r))
    print(f'{n} notes in {out}')


if __name__ == '__main__':
    main()
