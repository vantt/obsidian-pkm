// "Nguyên liệu cho bản nháp này": ticked items of the previous page go into the draft's `uses` first, then pkm
// gathers again (its incremental index build sees the new `uses`), then the page is rewritten with the ticks kept.
import { parseYaml } from 'obsidian';
import type { PkmCall } from '../bridge';
import { editFrontmatter, type NoteIO } from '../note-io';
import { asList, linkTarget, localIso, splitNote } from '../text';
import { pyJson } from '../workshop/digest';
import { LOG, type Material, pagePath, parseMaterial, renderPage, tickedIn } from './page';

/** Notes the draft's `uses` already points to, as Obsidian resolves the links. */
function usedBy(io: NoteIO, draft: string, fmUses: unknown): Set<string> {
  const out = new Set<string>();
  for (const v of asList(fmUses)) {
    const target = linkTarget(v) ?? (typeof v === 'string' ? v.trim() : null);
    const p = target && io.resolveLink(target, draft);
    if (p) out.add(p.normalize('NFC'));
  }
  return out;
}

/**
 * Adds ticked notes to the draft's `uses` (`"[[path-without-.md]]"`), skipping the draft itself, notes not in the
 * vault and notes already there; logs one line per note. Returns the paths added.
 */
export async function addTicked(io: NoteIO, draft: string, ticked: string[], now: Date): Promise<string[]> {
  if (!ticked.length) return [];
  const { fm } = splitNote(await io.read(draft), parseYaml);
  const have = usedBy(io, draft, fm.uses);
  const add: string[] = [];
  for (const t of ticked) {
    const p = io.findFile(t);
    if (!p || p === draft || have.has(p.normalize('NFC')) || add.includes(p)) continue;
    add.push(p);
  }
  if (!add.length) return [];
  await editFrontmatter(io, draft, (f) => {
    const uses = asList(f.uses);
    for (const p of add) {
      const entry = `[[${p.replace(/\.md$/, '')}]]`;
      if (!uses.includes(entry)) uses.push(entry);
    }
    f.uses = uses;
  });
  const at = localIso(now, 'seconds');
  const lines = add.map((note) => pyJson({ at, draft, rel: 'uses', note, status: 'accepted', by: 'human', via: 'pkm plugin tick' }, false) + '\n');
  await io.append(LOG, lines.join(''));
  return add;
}

export interface MaterialRun {
  page: string;
  added: string[];
  material: Material | null;
  /** Set when pkm failed after the ticks were saved. */
  error: unknown;
}

/** One run of the command on `draft` (a vault path). `vault` is the vault folder on disk. */
export async function runMaterial(io: NoteIO, call: PkmCall, vault: string, dbArgs: string[], draft: string, now: () => Date): Promise<MaterialRun> {
  const page = pagePath(draft);
  const ticked = (await io.exists(page)) ? tickedIn(await io.read(page)) : [];
  const added = await addTicked(io, draft, ticked, now());
  let material: Material;
  try {
    material = parseMaterial(await call([vault, 'material', draft, ...dbArgs], { json: false }));
  } catch (error) {
    return { page, added, material: null, error };
  }
  await io.write(page, renderPage(material, new Set(ticked), now()));
  return { page, added, material, error: null };
}
