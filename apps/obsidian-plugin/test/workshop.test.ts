import { beforeEach, describe, expect, it } from 'vitest';
import { parse } from 'yaml';
import { analyse, applyPlan, CONFIGS, MANIFEST, removeWorkshop } from '../src/workshop/apply';
import { digest, pyJson, sha } from '../src/workshop/digest';
import { buildPlan, fill, findTopicFolders, inboxFilter, pyQuote, templaterSettings } from '../src/workshop/plan';
import golden from './fixtures/python-workshop.json';
import { MemoryIO } from './memory-io';

const parseYaml = (t: string): unknown => parse(t);

/** The fixture vault the golden was produced from (empty notes plus .obsidian). */
function fixtureVault(): MemoryIO {
  const io = new MemoryIO(Object.fromEntries(golden.notes.map((p) => [p, ''])));
  io.dirs.add('.obsidian');
  return io;
}

async function plan(io: MemoryIO): Promise<Map<string, string>> {
  const topicFolders = await findTopicFolders(io, golden.topicRoots);
  return buildPlan({ vaultName: golden.vaultName, topicFolders, inboxFolders: golden.inboxFolders });
}

describe('workshop plan matches the Python workshop byte for byte', () => {
  it('same paths in the same order, same content', async () => {
    const files = await plan(fixtureVault());
    expect([...files.keys()]).toEqual(Object.keys(golden.files));
    for (const [p, text] of files) expect(text, p).toBe((golden.files as Record<string, string>)[p]);
  });

  it('same digests, including the parsed-YAML digest of .base files', async () => {
    for (const [p, text] of Object.entries(golden.files)) {
      expect(digest(p, text, parseYaml), p).toBe((golden.digests as Record<string, string>)[p]);
    }
  });

  it('same plugin configs', () => {
    expect(CONFIGS.homepage!.make(null)).toEqual(golden.homepage);
    expect(templaterSettings({ keep: 1 })).toEqual(golden.templater);
  });
});

describe('template helpers', () => {
  it('quotes like urllib.parse.quote(safe="")', () => {
    expect(pyQuote("a/b c!'()*~._-")).toBe('a%2Fb%20c%21%27%28%29%2A~._-');
    expect(pyQuote('Ý')).toBe('%C3%9D');
  });

  it('refuses a template variable without value', () => {
    expect(fill('x {{a}} {{b}}', { a: '1', b: '$&' })).toBe('x 1 $&');
    expect(() => fill('{{missing}}', {})).toThrow(/missing/);
  });

  it('always lists Inbox last, once', () => {
    expect(inboxFilter([])).toBe(`'file.inFolder("Inbox")'`);
    expect(inboxFilter(['Inbox', 'A'])).toBe(`'file.inFolder("A") || file.inFolder("Inbox")'`);
  });

  it('pyJson matches json.dumps(sort_keys=True, ensure_ascii=False)', () => {
    expect(pyJson({ b: [1, 0.55, 'Ý "x"'], a: null, c: true })).toBe('{"a": null, "b": [1, 0.55, "Ý \\"x\\""], "c": true}');
  });
});

describe('apply and remove', () => {
  let io: MemoryIO;
  let files: Map<string, string>;

  beforeEach(async () => {
    io = fixtureVault();
    io.files.set('.obsidian/community-plugins.json', JSON.stringify(['homepage', 'e11-review-log', 'pkm']));
    io.files.set('.obsidian/appearance.json', JSON.stringify({ cssTheme: 'X', enabledCssSnippets: ['e11-folder-grid', 'other'] }));
    files = await plan(io);
  });

  it('writes every file, records digests and old configs', async () => {
    const r = await applyPlan(io, files, await analyse(io, files, parseYaml), parseYaml);
    expect(r.written).toHaveLength(files.size);
    for (const [p, t] of files) expect(io.files.get(p)).toBe(t);
    const m = JSON.parse(io.files.get(MANIFEST)!);
    expect(m.files['Trang chủ.md']).toBe(sha(files.get('Trang chủ.md')!));
    expect(m.configs.homepage).toBeNull();
    expect(JSON.parse(io.files.get('.obsidian/community-plugins.json')!)).toEqual(['homepage', 'pkm']);
    expect(JSON.parse(io.files.get('.obsidian/appearance.json')!).enabledCssSnippets).toEqual(['other']);
    expect(JSON.parse(io.files.get('.obsidian/plugins/templater-obsidian/data.json')!).templates_folder).toBe('_Templates');
  });

  it('refuses to write anything when a planned file exists that it did not write', async () => {
    io.files.set('_Templates/Ý.md', 'my own template');
    const a = await analyse(io, files, parseYaml);
    expect(a.clash).toEqual(['_Templates/Ý.md']);
    await expect(applyPlan(io, files, a, parseYaml)).rejects.toThrow(/không do xưởng tạo/);
    expect(io.files.has('Trang chủ.md')).toBe(false);
    expect(io.files.get('_Templates/Ý.md')).toBe('my own template');
  });

  it('keeps files the user edited, rewrites the rest, and treats a reformatted .base as unchanged', async () => {
    await applyPlan(io, files, await analyse(io, files, parseYaml), parseYaml);
    io.files.set('Trang chủ.md', 'edited by the user');
    // Obsidian re-serializes a base once a view is touched: same data, different text
    const reformatted = JSON.stringify(parseYaml(files.get('_Xưởng/Chủ đề.base')!), null, 2);
    io.files.set('_Xưởng/Chủ đề.base', reformatted);
    io.files.delete('_Templates/Trích.md');
    const a = await analyse(io, files, parseYaml);
    expect(a.clash).toEqual([]);
    expect(a.edited).toEqual(['Trang chủ.md']);
    const r = await applyPlan(io, files, a, parseYaml);
    expect(r.kept).toEqual(['Trang chủ.md']);
    expect(io.files.get('Trang chủ.md')).toBe('edited by the user');
    expect(io.files.get('_Templates/Trích.md')).toBe(files.get('_Templates/Trích.md'));
    expect(io.files.get('_Xưởng/Chủ đề.base')).toBe(files.get('_Xưởng/Chủ đề.base'));
  });

  it('removes files it no longer plans unless edited, including the old plugin and snippet files', async () => {
    const oldPlugin = '.obsidian/plugins/e11-review-log/main.js';
    const oldSnippet = '.obsidian/snippets/e11-folder-grid.css';
    io.files.set(oldPlugin, 'old plugin');
    io.files.set(oldSnippet, 'edited css');
    io.files.set(
      MANIFEST,
      JSON.stringify({ files: { [oldPlugin]: sha('old plugin'), [oldSnippet]: sha('css'), 'Notes\\1. Projects\\Alpha\\Alpha (chủ đề).md': 'x' }, configs: { appearance: '{}' } }),
    );
    const a = await analyse(io, files, parseYaml);
    expect(a.stale).toEqual([oldPlugin, oldSnippet]);
    const r = await applyPlan(io, files, a, parseYaml);
    expect(r.removed).toEqual([oldPlugin]);
    expect(r.kept).toEqual([oldSnippet]);
    expect(io.files.has(oldPlugin)).toBe(false);
    const m = JSON.parse(io.files.get(MANIFEST)!);
    expect(m.files[oldSnippet]).toBeDefined();
    expect(m.configs.appearance).toBe('{}');
  });

  it('is idempotent', async () => {
    await applyPlan(io, files, await analyse(io, files, parseYaml), parseYaml);
    const before = new Map(io.files);
    const a = await analyse(io, files, parseYaml);
    expect(a).toMatchObject({ clash: [], edited: [], stale: [] });
    await applyPlan(io, files, a, parseYaml);
    expect(io.files).toEqual(before);
  });

  it('stops before writing notes when a config file is not JSON', async () => {
    io.files.set('.obsidian/appearance.json', '{broken');
    await expect(applyPlan(io, files, await analyse(io, files, parseYaml), parseYaml)).rejects.toThrow(/appearance.json/);
    expect(io.files.has('Trang chủ.md')).toBe(false);
  });

  it('remove deletes unedited files and empty folders, keeps edits, restores configs', async () => {
    const before = new Map(io.files);
    await applyPlan(io, files, await analyse(io, files, parseYaml), parseYaml);
    io.files.set('_Templates/Ý.md', 'mine now');
    const r = await removeWorkshop(io, parseYaml);
    expect(r.kept).toEqual(['_Templates/Ý.md']);
    expect(io.files.has('Trang chủ.md')).toBe(false);
    expect(await io.exists('_Xưởng')).toBe(false);
    expect(await io.exists('Bài viết')).toBe(false);
    expect(io.files.has(MANIFEST)).toBe(false);
    expect(io.files.has('.obsidian/plugins/homepage/data.json')).toBe(false);
    expect(io.files.get('.obsidian/community-plugins.json')).toBe(before.get('.obsidian/community-plugins.json'));
    expect(JSON.parse(io.files.get('.obsidian/appearance.json')!).enabledCssSnippets).toEqual(['other']);
    const leftover = [...io.files.keys()].filter((p) => !before.has(p) && p !== '_Templates/Ý.md');
    expect(leftover).toEqual([]);
  });

  it('remove refuses without a manifest', async () => {
    await expect(removeWorkshop(io, parseYaml)).rejects.toThrow(/chưa dựng xưởng/);
  });
});
