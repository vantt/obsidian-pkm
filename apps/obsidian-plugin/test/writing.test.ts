import { describe, expect, it } from 'vitest';
import { parse } from 'yaml';
import { type PkmCall, PkmError } from '../src/bridge';
import { addTicked, runMaterial } from '../src/material/flow';
import { LOG, pagePath, parseMaterial, renderPage, tickedIn } from '../src/material/page';
import { FM_RE } from '../src/text';
import { gradesIn, loadZones, PAGE, parseZones, postContent, startPost, zoneSummary } from '../src/zones/zones';
import golden from './fixtures/python-writing.json';
import { MemoryNoteIO } from './memory-note-io';

const NOW = new Date(2026, 10, 4, 9, 5, 7);
const fm = (text: string) => parse(FM_RE.exec(text)?.[1] ?? '') as Record<string, unknown>;
const body = (text: string) => text.slice(FM_RE.exec(text)?.[0].length ?? 0);
const fromPython = (page: string) => page.replace('generated_by: scripts/e11/material.py', 'generated_by: pkm material');

describe('material page matches material.py', () => {
  it('same layout: roles, used, graph chains, topics, queries, kept ticks', () => {
    const r = parseMaterial(golden.material);
    expect(renderPage(r, new Set(golden.ticked), NOW)).toBe(fromPython(golden.page));
  });

  it('same page for a draft without text', () => {
    const empty = { draft: 'Bài viết/Bài mới.md', queries: [], methods: {}, items: [], missing: [], edges: {}, draft_seed: false, topics: [] };
    expect(renderPage(parseMaterial(empty), new Set(), NOW)).toBe(fromPython(golden.emptyPage));
  });

  it('reads ticked boxes like ticked_in', () => {
    const page = '- [x] [[Vườn/Cây cà chua|Cây cà chua]] · note\n- [ ] [[Lưu/Đất trồng|Đất trồng]]\n    - [X] [[Vườn/Hạt giống]]\n';
    expect(tickedIn(page)).toEqual(golden.ticked);
    expect(tickedIn('- [x] [[A#h]]\r\n- [x] no link\r\n')).toEqual(['A.md']);
  });

  it('rejects output of the wrong shape', () => {
    expect(() => parseMaterial({ draft: 'x.md', queries: [], items: [{ id: 3 }] })).toThrow(PkmError);
    expect(() => parseMaterial([])).toThrow(/draft/);
  });
});

describe('ticks into uses', () => {
  const draft = 'Bài viết/Nháp.md';
  const notes = { 'A/b.md': 'b\n', 'A/c.md': 'c\n', 'D.md': 'd\n' };

  it('adds to an empty list, a block list, an inline list, and a note without uses; body kept', async () => {
    const cases: [string, unknown][] = [
      ['---\ntype: post\nuses: []\nchannel: x\n---\nthân\n', ['[[A/b]]']],
      ['---\nuses:\n  - "[[D]]"\nstatus: nháp\n---\n', ['[[D]]', '[[A/b]]']],
      ['---\nuses: ["[[D]]"]\n---\n', ['[[D]]', '[[A/b]]']],
      ['---\ntype: post\n---\n', ['[[A/b]]']],
    ];
    for (const [text, uses] of cases) {
      const io = new MemoryNoteIO({ ...notes, [draft]: text });
      expect(await addTicked(io, draft, ['A/b.md'], NOW)).toEqual(['A/b.md']);
      const out = io.files.get(draft)!;
      expect(fm(out).uses, text).toEqual(uses);
      expect(body(out)).toBe(body(text));
    }
  });

  it('keeps CRLF, skips the draft, missing notes, duplicates and notes already used; logs each added note', async () => {
    const io = new MemoryNoteIO({ ...notes, [draft]: '---\r\nuses:\r\n  - "[[c]]"\r\nx: 1\r\n---\r\nthân\r\n' });
    const added = await addTicked(io, draft, ['A/c.md', draft, 'Không có.md', 'D.md', 'D.md', 'A/b.md'], NOW);
    expect(added).toEqual(['D.md', 'A/b.md']);
    const out = io.files.get(draft)!;
    expect(fm(out).uses).toEqual(['[[c]]', '[[D]]', '[[A/b]]']);
    expect(FM_RE.exec(out)![0].replace(/\r\n/g, '')).not.toContain('\n');
    expect(body(out)).toBe('thân\r\n');
    expect(io.files.get(LOG)).toBe(
      '{"at": "2026-11-04T09:05:07", "draft": "Bài viết/Nháp.md", "rel": "uses", "note": "D.md", "status": "accepted", "by": "human", "via": "pkm plugin tick"}\n' +
        '{"at": "2026-11-04T09:05:07", "draft": "Bài viết/Nháp.md", "rel": "uses", "note": "A/b.md", "status": "accepted", "by": "human", "via": "pkm plugin tick"}\n',
    );
    // ticking again does not add twice
    expect(await addTicked(io, draft, ['D.md'], NOW)).toEqual([]);
  });
});

describe('material command flow', () => {
  const draft = 'Bài viết/Vườn rau ban công.md';
  const material = { ...golden.material, items: golden.material.items };

  it('moves ticks into uses, calls pkm without --json, rewrites the page keeping ticks', async () => {
    const page = pagePath(draft);
    const io = new MemoryNoteIO({
      [draft]: '---\ntype: post\n---\nÝ\n',
      'Vườn/Cây cà chua.md': 'x\n',
      [page]: '- [x] [[Vườn/Cây cà chua|Cây cà chua]] · note\n',
    });
    const calls: unknown[] = [];
    const call: PkmCall = async (args, opts) => {
      calls.push([args, opts]);
      return material;
    };
    const r = await runMaterial(io, call, '/v', ['--db', '/g.db'], draft, () => NOW);
    expect(page).toBe('_Xưởng/Nguyên liệu/Vườn rau ban công.md');
    expect(r.added).toEqual(['Vườn/Cây cà chua.md']);
    expect(calls).toEqual([[['/v', 'material', draft, '--db', '/g.db'], { json: false }]]);
    expect(fm(io.files.get(draft)!).uses).toEqual(['[[Vườn/Cây cà chua]]']);
    expect(io.files.get(page)).toBe(renderPage(parseMaterial(material), new Set(['Vườn/Cây cà chua.md']), NOW));
  });

  it('keeps the ticks saved and the page untouched when pkm fails', async () => {
    const page = pagePath(draft);
    const io = new MemoryNoteIO({ [draft]: 'Ý\n', 'D.md': '', [page]: '- [x] [[D]]\n' });
    const err = new PkmError('ohs_not_ready', 'OHS chưa sẵn sàng.');
    const r = await runMaterial(io, async () => Promise.reject(err), '/v', [], draft, () => NOW);
    expect(r.error).toBe(err);
    expect(r.added).toEqual(['D.md']);
    expect(io.files.get(page)).toBe('- [x] [[D]]\n');
  });
});

describe('zones', () => {
  const zones = parseZones({ generated: '2026-11-04', zones: golden.zones, stats: { zones: 2 } });

  it('describes a zone like the page does', () => {
    const lines = golden.zonesPage.split('\n');
    for (const z of zones.zones) {
      const line = lines.find((l) => l.startsWith(`${z.notes} note, ${z.folders} thư mục`))!;
      expect(line.startsWith(zoneSummary(z)), line).toBe(true);
    }
  });

  it('reads the user grades from the page and ignores empty ones', () => {
    const page = golden.zonesPage
      .replace('[Viết bài từ vùng này](', 'x')
      .replace(/(## 1\.[\s\S]*?\*\*Chấm:\*\*) /, '$1 đáng viết');
    expect(gradesIn(page)).toEqual(new Map([['cà chua · ban công', 'đáng viết']]));
    expect(gradesIn(golden.zonesPage)).toEqual(new Map());
  });

  it('starts a post with the content of the page link and never overwrites', async () => {
    const z = zones.zones[0]!;
    const link = /obsidian:\/\/new\?([^)]*)\)/.exec(golden.zonesPage)![1]!;
    const q = new URLSearchParams(link.replace(/\+/g, '%2B'));
    expect(postContent(z)).toBe(q.get('content'));
    const io = new MemoryNoteIO({});
    expect(await startPost(io, z)).toEqual({ path: `${q.get('file')}.md`, created: true });
    io.files.set('Bài viết/cà chua ban công.md', 'mine');
    expect(await startPost(io, z)).toEqual({ path: 'Bài viết/cà chua ban công.md', created: false });
    expect(io.files.get('Bài viết/cà chua ban công.md')).toBe('mine');
  });

  it('asks pkm to write the page only when it is missing, then reads the grades', async () => {
    const calls: unknown[] = [];
    const io = new MemoryNoteIO({});
    const call: PkmCall = async (args, opts) => {
      calls.push([args, opts]);
      if (args.includes('--apply')) io.files.set(PAGE, golden.zonesPage.replace('**Chấm:** ', '**Chấm:** bất ngờ'));
      return { generated: '2026-11-04', zones: golden.zones, stats: {} };
    };
    const first = await loadZones(io, call, '/v', []);
    expect(first.pageCreated).toBe(true);
    expect(first.grades.get('cà chua · ban công')).toBe('bất ngờ');
    const second = await loadZones(io, call, '/v', ['--db', '/g.db']);
    expect(second.pageCreated).toBe(false);
    expect(calls).toEqual([
      [['/v', 'zones', '--apply'], { json: false }],
      [['/v', 'zones', '--write', '--db', '/g.db'], { json: false }],
    ]);
  });

  it('rejects output without zones', () => {
    expect(() => parseZones({ error: null })).toThrow(PkmError);
    expect(() => parseZones({ zones: [{ score: 1 }] })).toThrow(/label/);
  });
});
