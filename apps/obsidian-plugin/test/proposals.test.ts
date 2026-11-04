import { describe, expect, it } from 'vitest';
import { parse } from 'yaml';
import { PkmError, type PkmCall } from '../src/bridge';
import { applyDecisions, decide, ideaDraft, listQueue, LOG, revertAll } from '../src/proposals/apply';
import { refreshProposals } from '../src/proposals/refresh';
import { reviewStats, STATS } from '../src/proposals/stats';
import { REVIEW_LOG } from '../src/review-log';
import { FM_RE } from '../src/text';
import golden from './fixtures/python-proposals.json';
import { MemoryNoteIO } from './memory-note-io';

/** The moment the golden was made, as local time (the Python script stamps local time). */
const NOW = new Date(2026, 10, 4, 11, 30, 5);

function vault(files: Record<string, string>): MemoryNoteIO {
  const io = new MemoryNoteIO(files);
  io.dirs.add('.obsidian');
  return io;
}

function parts(text: string): { fm: unknown; body: string } {
  const m = FM_RE.exec(text);
  return m ? { fm: parse(m[1] ?? '') ?? {}, body: text.slice(m[0].length) } : { fm: {}, body: text };
}

describe('apply matches propose.py --apply on an invented vault', () => {
  it('same files, same properties, same bodies, same created notes, same log', async () => {
    const io = vault(golden.before);
    const r = await applyDecisions(io, NOW);
    expect(r.failed).toEqual([]);
    expect(r.counts).toEqual({
      'same-topic|nhận': 1,
      'type-batch|bỏ': 1,
      'type-batch|nhận': 2,
      'related|nhận': 1,
      'person|nhận': 2,
      'idea|nhận': 2,
      'fix-link|nhận': 2,
      'fix-link|bỏ': 1,
      'duplicate|nhận': 1,
    });
    const after = golden.after as Record<string, string>;
    expect([...io.files.keys()].sort()).toEqual(Object.keys(after).sort());
    // Edited notes: Obsidian writes the frontmatter in its own YAML layout; properties and body must match.
    const sameNote = (got: string, want: string, what: string) => {
      if (got === want) return;
      const g = parts(got), w = parts(want);
      expect(g.fm, what).toEqual(w.fm);
      expect(g.body, what).toBe(w.body);
      const fmBlock = (t: string) => FM_RE.exec(t)?.[0] ?? '';
      if (fmBlock(want).includes('\r\n')) expect(fmBlock(got).replace(/\r\n/g, ''), `${what} keeps CRLF`).not.toContain('\n');
    };
    for (const [path, want] of Object.entries(after)) {
      const got = io.files.get(path)!;
      // created notes are written by the plugin itself: byte for byte
      if (path === LOG) continue;
      if (!(path in golden.before)) expect(got, path).toBe(want);
      else sameNote(got, want, path);
    }
    // Same log entries in the same order. An old text is byte for byte unless an earlier proposal of the same run
    // had already edited that note's properties.
    const lines = (t: string) => t.trimEnd().split('\n').map((l) => JSON.parse(l) as Record<string, unknown>);
    const got = lines(io.files.get(LOG)!), want = lines(after[LOG]!);
    expect(got).toHaveLength(want.length);
    got.forEach((g, i) => {
      const w = want[i]!;
      expect(Object.keys(g)).toEqual(Object.keys(w));
      expect({ ...g, changes: null }).toEqual({ ...w, changes: null });
      const gc = g.changes as Record<string, string | null>, wc = w.changes as Record<string, string | null>;
      expect(Object.keys(gc), String(w.proposal)).toEqual(Object.keys(wc));
      for (const [p, old] of Object.entries(wc)) {
        if (old === null) expect(gc[p]).toBeNull();
        else sameNote(gc[p]!, old, `${String(w.proposal)} -> ${p}`);
      }
    });
    expect(io.files.get(LOG)!.split('\n')[0]).toBe(after[LOG]!.split('\n')[0]);
  });

  it('revert restores every file byte for byte and sets the log aside', async () => {
    const io = vault(golden.before);
    await applyDecisions(io, NOW);
    expect(await revertAll(io, NOW)).toBe(13);
    const reverted = golden.reverted as Record<string, string>;
    expect([...io.files.keys()].sort()).toEqual(Object.keys(reverted).sort());
    for (const [path, want] of Object.entries(reverted)) {
      if (!path.startsWith(`${LOG}.reverted-`)) expect(io.files.get(path), path).toBe(want);
    }
    expect(io.renamed).toEqual([[LOG, `${LOG}.reverted-20261104T113005`]]);
  });

  it('stats match --stats, apart from the plugin name in the explanation', async () => {
    const io = vault(golden.after);
    io.files.set(REVIEW_LOG, golden.reviewLog);
    const r = await reviewStats(io, NOW);
    expect(r.rows).toBe(7);
    expect(io.files.get(STATS)).toBe(golden.stats.replace('plugin e11-review-log', 'plugin PKM'));
    expect(r.text).toContain('| same-topic | 1 | 100% | 1 | 100% | 70 |');
  });
});

describe('queue and decisions', () => {
  it('lists proposals with status and reason, and records decisions with an optional reason', async () => {
    const io = vault(golden.before);
    const q = await listQueue(io);
    expect(q).toHaveLength(14);
    const waiting = q.filter((p) => p.status === 'chờ');
    expect(waiting.map((p) => p.kind)).toEqual(['related']);
    const rejected = q.find((p) => p.kind === 'type-batch' && p.status === 'bỏ')!;
    expect(rejected.rejectReason).toBe('không phải bài cắt');

    const path = waiting[0]!.path;
    await decide(io, path, 'bỏ', '  trùng ý  ');
    let fm = parts(io.files.get(path)!).fm as Record<string, unknown>;
    expect(fm.status).toBe('bỏ');
    expect(fm.reject_reason).toBe('trùng ý');
    await decide(io, path, 'chờ');
    fm = parts(io.files.get(path)!).fm as Record<string, unknown>;
    expect(fm.status).toBe('chờ');
  });

  it('leaves a proposal marked and its notes untouched when a note is missing', async () => {
    const io = vault({
      'A.md': '---\ntype: note\n---\nA\n',
      '_Đề xuất/Liên quan - A.md': "---\ntype: proposal\nkind: related\nstatus: nhận\nnotes:\n- '[[A|A]]'\n- '[[Gone|Gone]]'\n---\n",
    });
    const before = new Map(io.files);
    const r = await applyDecisions(io, NOW);
    expect(r.failed).toEqual([{ path: '_Đề xuất/Liên quan - A.md', message: 'không thấy note Gone.md' }]);
    expect(io.files).toEqual(before);
  });

  it('treats a single related value as a list and does not add a link twice', async () => {
    const io = vault({
      'A.md': "---\nrelated: '[[B|B]]'\n---\nA\n",
      'B.md': 'B\n',
      '_Đề xuất/x.md': "---\ntype: proposal\nkind: related\nstatus: nhận\nnotes:\n- '[[A|A]]'\n- '[[B|B]]'\n---\n",
    });
    await applyDecisions(io, NOW);
    expect(parts(io.files.get('A.md')!).fm).toEqual({ related: ['[[B|B]]'] });
    expect(parts(io.files.get('B.md')!)).toEqual({ fm: { related: ['[[A|A]]'] }, body: 'B\n' });
  });

  it('refuses a corrupt log before changing anything', async () => {
    const io = vault({ [LOG]: '{"proposal": "x"}\n', 'x': 'kept' });
    await expect(revertAll(io, NOW)).rejects.toThrow(/dòng 1/);
    expect(io.files.get('x')).toBe('kept');
    await expect(revertAll(vault({}), NOW)).rejects.toThrow(/chưa có/);
  });

  it('writes a wikilink with a quote as a single-quoted YAML scalar', () => {
    expect(ideaDraft("A/B's (chủ đề).md", "[[N|N's]]", [' x '])).toContain("topic: '[[A/B''s (chủ đề)|B''s (chủ đề)]]'\n");
  });
});

describe('refresh', () => {
  it('runs pairs --write, then propose --create with the pairs file', async () => {
    const calls: [string[], boolean | undefined][] = [];
    const call: PkmCall = async (args, opts) => {
      calls.push([args, opts?.json]);
      return args[1] === 'pairs' ? [] : { total: 3, new: 2, by_kind: { related: 1, idea: 1 }, proposals: [], written: ['_Đề xuất/a.md', '_Đề xuất/b.md'] };
    };
    const r = await refreshProposals(call, '/v', ['--db', '/v/.e11/graph.db']);
    expect(r.warning).toBeNull();
    expect(r.result).toEqual({ total: 3, new: 2, byKind: { related: 1, idea: 1 }, written: ['_Đề xuất/a.md', '_Đề xuất/b.md'] });
    expect(calls).toEqual([
      [['/v', 'pairs', '--write', '--db', '/v/.e11/graph.db'], false],
      [['/v', 'propose', '--create', '--similar', '/v/_E11/similar-pairs.json'], undefined],
    ]);
  });

  it('without OHS, says so and proposes without the related kind', async () => {
    const calls: string[][] = [];
    const call: PkmCall = async (args) => {
      calls.push(args);
      if (args[1] === 'pairs') throw new PkmError('ohs_not_ready', 'Tìm theo nghĩa (OHS) đang khởi động.');
      return { total: 0, new: 0, by_kind: {}, proposals: [], written: [] };
    };
    const r = await refreshProposals(call, '/v', []);
    expect(r.warning).toBe('Tìm theo nghĩa (OHS) đang khởi động. Lần này không có đề xuất "liên quan".');
    expect(calls[1]).toEqual(['/v', 'propose', '--create']);
  });

  it('rejects propose output of the wrong shape', async () => {
    const call: PkmCall = async (args) => (args[1] === 'pairs' ? [] : { total: 1 });
    await expect(refreshProposals(call, '/v', [])).rejects.toMatchObject({ kind: 'bad_output' });
  });
});
