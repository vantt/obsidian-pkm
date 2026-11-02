"""Write the synthetic edge-case notes into ./cases/ (one note per YAML shape that 1.1 and 1.2 readers may disagree on).

Usage: python3 -I make-cases.py
Generated, not hand-edited, because several cases need exact bytes (CRLF, BOM, NFD, control characters, bad UTF-8).
After regenerating, refresh the reference dump:
  python3 -E -P pyyaml-dump.py --spec <spec repo> cases > cases.pyyaml.jsonl
"""
import os
import shutil
import unicodedata

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, 'cases')

NFD_VI = unicodedata.normalize('NFD', 'Tiếng Việt')

FM = {
    'bool-yes-no-on-off': 'a: yes\nb: No\nc: on\nd: OFF\ne: y\nf: n\ng: True\nh: "yes"\ni: [yes, off]\n',
    'date-time-plain': ('d: 2026-01-05\ndt: 2026-01-05 10:00\ndt2: 2026-01-05T10:00:00Z\ndt3: 2026-01-05 10:00:00 +07:00\n'
                        'bad: 2026-13-01\nlist: [2026-01-05, 2026-1-5]\n'),
    'int-octal-underscore-base': ('oct: 010\noct2: 0o17\nunder: 1_000\nhex: 0x1F\nbin: 0b101\nplus: +12\nneg: -0\n'
                                  'big: 123456789012345678901234567890\nlead8: 08\nzero: 00\nunder0: 0_\n'),
    'int-phone-zip': 'phone: 0912345678\nzip: 0123\nid: 007\n',
    'int-sexagesimal-time': 'time: 10:30\nduration: 1:30:00\nearly: 09:30\nlate: 23:59\nsexa: 190:20:30\n',
    'float-forms': ('f1: 1.5\nf2: 1e3\nf3: 1.0e+3\nf4: .5\nf5: 1.\nf6: .inf\nf7: -.Inf\nf8: .nan\nf9: 1_000.5\n'
                    'f10: 190:20:30.15\nf11: 1e-7\nf12: 0.0001\nf13: 12345678901234567.0\nf14: 1.0e-5\nf15: 1.10\n'
                    'f16: 1.0e+400\nf17: -0.0\nf18: 3.0\nf19: 1.2.3\n'),
    'null-forms': 'a: ~\nb:\nc: null\nd: Null\ne: NULL\nf: nULL\ng: ""\nh: \'~\'\ni: [~, null, ]\n',
    'duplicate-key': 'a: 1\nb: 2\na: 3\n',
    'duplicate-key-type': 'type: client\nstatus: active\ntype: project\n',
    'duplicate-key-nested': 'meta:\n  x: 1\n  x: 2\n',
    'multiline-strings': ('lit: |\n  line1\n  line2\nfold: >\n  a\n  b\n\n  c\nplain: a\n  b\nkeep: |+\n  x\n\nstrip: |-\n  y\n'
                          'q: "a\n  b"\nsq: \'it\'\'s\n  here\'\n'),
    'block-scalar-last': 'a: 1\nnote: |\n  line one\n  line two\n',   # FM_RE drops the final newline
    'unicode-nfd': f'title: {NFD_VI}\n{NFD_VI}: nfd-key\nnfc: Tiếng Việt\nm:\n  k: {NFD_VI}\n',
    'merge-key': 'base: &b {x: 1, y: 2}\nder:\n  <<: *b\n  y: 3\nmulti:\n  <<: [{p: 1}, {p: 2, q: 3}]\n  r: 4\n',
    'anchor-alias': 'a: &x foo\nb: *x\nl: &l [1, 2]\nm: *l\n',
    'alias-undefined': 'a: 1\nb: *nope\n',
    'anchor-duplicate': 'a: &x 1\nb: &x 2\n',
    'explicit-tags': ('s: !!str 123\ni: !!int "42"\nf: !!float "1"\nb: !!bool "yes"\nt: !!timestamp 2026-01-05\n'
                      'tt: !!timestamp 2026-01-05 10:30:00.5+07:00\nbin: !!binary aGVsbG8=\nset: !!set {a}\n'
                      'omap: !!omap [a: 1, b: 2]\nn: ! 12\nns: !!null ""\n'),
    'explicit-tag-local': 'local: !foo bar\n',
    'explicit-timestamp-invalid': 't: !!timestamp 2026-02-30\n',
    'int-invalid-literal': 'hexbad: 0x_\n',
    'crash-bool-keyerror': 'b: !!bool maybe\n',
    'crash-timestamp-attributeerror': 't: !!timestamp soon\n',
    'crash-int-indexerror': 'i: !!int ""\n',
    'crash-recursive-alias': 'a: &r [1, *r]\n',
    'nested-repr': ('meta:\n  inner: {k: v, n: 1, t: true, x: ~, l: [1, \'a\', "it\'s"], f: 1.5, q: "say \\"hi\\""}\n'
                    '  s: x\n  d: !!timestamp 2026-01-05\n  dt: !!timestamp 2026-01-05 10:30:00Z\n'
                    '  zw: "a\\u200bb"\n  emoji: "😀"\n  ctl: "a\\x01b"\n  bs: "back\\\\slash"\n'),
    'flow-maps-in-list': 'items: [{a: 1}, {b: [x, y]}]\nblock:\n  - c: 1\n    d: 2\n',
    'non-string-keys': '1: a\ntrue: b\nyes: c\n~: d\n1.5: e\n2026-01-05: f\n',
    'key-equality': '1: a\n1.0: b\ntrue: c\nfalse: d\n0: e\n',
    'unhashable-key': '? [a, b]\n: c\n',
    'top-level-list': '- a\n- b\n',
    'top-level-string': 'just a string\n',
    'top-level-falsy-zero': '0\n',
    'top-level-falsy-empty-list': '[]\n',
    'empty-frontmatter': '\n',
    'comment-only': '# nothing here\n',
    'value-equals': 'a: =\n',
    'syntax-mapping-in-plain': 'a: b: c\n',
    'syntax-unclosed-flow': 'a: [1, 2\n',
    'syntax-tab-indent': 'a:\n\tb: 1\n',
    'syntax-backtick': 'a: `code`\n',
    'syntax-at-sign': 'a: @foo\n',
    'syntax-bad-indent': 'a: 1\n  b: 2\n',
    'multiple-documents': 'a: 1\n...\nb: 2\n',
    'control-char-raw': 'a: "x\x01y"\n',
    'quoted-escapes': 'a: "\\x41\\u00e9\\N\\_\\t"\nb: \'single \\n\'\n',
    'wiki-links': 'up: "[[A]]"\nrel: [[A]]\nrelated:\n  - [[B|alias]]\n  - "[[C#h]]"\naliases: [Tiếng Việt, "x, y"]\n',
    'comments-and-hash': 'a: b #comment\nc: d#not\ne: "f" # c\n',
    'url-and-colon': 'url: http://example.com/a:b\nratio: 1:2\nk: a: b\n',
    'long-key': ('k' * 1100) + ': v\n',
    'tags-and-lists': 'tags:\n- a\n- b\nempty: []\nemptymap: {}\n',
    'special-plain': 'pct: 50%\nprice: 1,000\nver: v1.0\nneg: -5\nplus: +.5\nhash: "#tag"\n',
}

RAW = {     # whole-file bytes for cases that are about the file, not the YAML text
    'crlf': ('---\r\na: 1\r\nlit: |\r\n  x\r\n  y\r\nq: "p\r\n  q"\r\n---\r\nbody\r\n').encode(),
    'bom': ('\ufeff---\na: 1\n---\n').encode(),
    'unclosed-frontmatter': b'---\na: 1\nbody\n',
    'invalid-utf8': b'---\na: caf\xe9\nb: ok\n---\n',
    'trailing-spaces-fence': b'---  \na: 1\n--- \nbody\n',
    'no-frontmatter': b'just body\n',
}


def main():
    if os.path.isdir(OUT):
        shutil.rmtree(OUT)
    os.makedirs(OUT)
    for name, fm in FM.items():
        with open(os.path.join(OUT, name + '.md'), 'w', encoding='utf-8', newline='') as f:
            f.write('---\n' + fm + '---\nbody\n')
    for name, data in RAW.items():
        with open(os.path.join(OUT, name + '.md'), 'wb') as f:
            f.write(data)
    print(f'{len(FM) + len(RAW)} cases in {OUT}')


if __name__ == '__main__':
    main()
