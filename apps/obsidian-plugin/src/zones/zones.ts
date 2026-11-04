// Knowledge zones that could become posts: `pkm VAULT zones` computes them; the plugin shows them and starts a
// post from one. The page `_Xưởng/Gợi ý bài viết.md` is the persistent output: pkm writes it only when it is
// missing (`--apply`), so the user's **Chấm:** lines are never written over.
import { PkmError, type PkmCall } from '../bridge';
import type { NoteIO } from '../note-io';
import { cleanName, isRecord, splitLines } from '../text';

export const PAGE = '_Xưởng/Gợi ý bài viết.md';
export const POSTS = 'Bài viết';

export interface Zone {
  label: string;
  score: number;
  notes: number;
  core: string[];
  with_highlights: number;
  highlights: number;
  colours: Record<string, number>;
  own_ideas: number;
  folders: number;
  posts: string[];
  years: string[];
  picks: { text: string; colour: string }[];
}

export interface ZonesResult {
  generated: string;
  zones: Zone[];
  stats: Record<string, unknown>;
}

const strings = (v: unknown): string[] => (Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string') : []);
const num = (v: unknown): number => (typeof v === 'number' && Number.isFinite(v) ? v : 0);

export function parseZones(v: unknown): ZonesResult {
  if (!isRecord(v) || !Array.isArray(v.zones)) throw new PkmError('bad_output', 'pkm zones trả về dữ liệu không đúng dạng (zones).');
  const zones = v.zones.map((z): Zone => {
    if (!isRecord(z) || typeof z.label !== 'string') throw new PkmError('bad_output', 'pkm zones trả về một vùng không có label.');
    const colours: Record<string, number> = {};
    if (isRecord(z.colours)) for (const [k, n] of Object.entries(z.colours)) colours[k] = num(n);
    return {
      label: z.label,
      score: num(z.score),
      notes: num(z.notes),
      core: strings(z.core),
      with_highlights: num(z.with_highlights),
      highlights: num(z.highlights),
      colours,
      own_ideas: num(z.own_ideas),
      folders: num(z.folders),
      posts: strings(z.posts),
      years: strings(z.years),
      picks: (Array.isArray(z.picks) ? z.picks : [])
        .filter(isRecord)
        .map((p) => ({ text: String(p.text ?? ''), colour: String(p.colour ?? '') })),
    };
  });
  return { generated: typeof v.generated === 'string' ? v.generated : '', zones, stats: isRecord(v.stats) ? v.stats : {} };
}

/** The zone's one-line description, as on the page. */
export function zoneSummary(z: Zone): string {
  const colours = Object.entries(z.colours).map(([k, v]) => `${k} ${v}`).join(', ');
  const years = z.years.length >= 2 ? `, năm ${z.years[0]}–${z.years[1]}` : '';
  return `${z.notes} note, ${z.folders} thư mục, ${z.with_highlights} note có tô (${z.highlights} đoạn: ${colours}), ${z.own_ideas} ý của mình${years}. Điểm ${z.score}.`;
}

/** The user's grade of each zone on the page (`## N. label` ... `**Chấm:** text`), by label; empty grades left out. */
export function gradesIn(page: string): Map<string, string> {
  const out = new Map<string, string>();
  let label: string | null = null;
  for (const line of splitLines(page)) {
    const h = /^## \d+\. (.+)$/.exec(line);
    if (h) {
      label = (h[1] ?? '').trim();
      continue;
    }
    const g = /\*\*Chấm:\*\*(.*)$/.exec(line);
    if (g && label !== null) {
      const grade = (g[1] ?? '').trim();
      if (grade) out.set(label, grade);
      label = null;
    }
  }
  return out;
}

/** `Bài viết/<label without the · separators>.md` */
export const postPath = (z: Zone): string => `${POSTS}/${cleanName(z.label.split(' · ').join(' ')) || 'Bài mới'}.md`;

/** A new post seeded from the zone: `type: post`, `status: ý tưởng`, the zone's core notes as sources. */
export function postContent(z: Zone): string {
  return `---\ntype: post\nstatus: ý tưởng\n---\n\n## Nguồn (máy gom từ vùng)\n\n${z.core.map((n) => `- [[${n.replace(/\.md$/, '')}]]`).join('\n')}\n`;
}

/** Creates the post unless a note with that name exists; either way returns its path. */
export async function startPost(io: NoteIO, z: Zone): Promise<{ path: string; created: boolean }> {
  const path = postPath(z);
  if (await io.exists(path)) return { path, created: false };
  await io.write(path, postContent(z));
  return { path, created: true };
}

/**
 * Runs `pkm VAULT zones`: `--apply` when the page is missing (pkm writes it), `--write` otherwise (zones.json only).
 * Returns the zones and the grades already on the page.
 */
export async function loadZones(io: NoteIO, call: PkmCall, vault: string, dbArgs: string[]): Promise<{ result: ZonesResult; grades: Map<string, string>; pageCreated: boolean }> {
  const hadPage = await io.exists(PAGE);
  const result = parseZones(await call([vault, 'zones', hadPage ? '--write' : '--apply', ...dbArgs], { json: false }));
  const pageCreated = !hadPage && (await io.exists(PAGE));
  const grades = pageCreated || hadPage ? gradesIn(await io.read(PAGE)) : new Map<string, string>();
  return { result, grades, pageCreated };
}
