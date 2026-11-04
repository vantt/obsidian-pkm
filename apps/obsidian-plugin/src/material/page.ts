// The material page for a draft: what `pkm VAULT material DRAFT` found, grouped by role, written to
// `_Xưởng/Nguyên liệu/<draft>.md`. The page only points; the user opens, reads and ticks `[x]` on what to use.
// Ticked notes go into the draft's `uses` (using is reviewing) and into `_E11/material-log.jsonl`.
import { PkmError } from '../bridge';
import { clockDay, cpLength, cpSlice, isRecord, localIso, splitLines, wikilink } from '../text';

export const OUT_DIR = '_Xưởng/Nguyên liệu';
export const LOG = '_E11/material-log.jsonl';

/** Section headings by role and the note types they hold; untyped notes are saved articles. */
export const ROLES: [string, (string | null)[]][] = [
  ['Ý của anh', ['note', 'idea', 'condense', 'post', 'concept', 'question', 'checklist', 'example']],
  ['Trích', ['quote']],
  ['Bài lưu', [null, 'clip', 'reference', 'entity', 'relationship', 'utility', 'diagram', 'drawing', 'project']],
  ['Bản đồ của anh (MOC)', ['moc']],
  ['Mục lục', ['toc']],
];

export interface Step {
  from: string;
  rel: string;
  to: string;
  status: string;
  proposed_by?: string | null;
  score?: number | null;
}

export interface Item {
  id: string;
  type: string | null;
  found_by: string;
  used: boolean;
  sections?: string[];
  via?: Step[];
  highlights?: number;
  first_highlights?: string[];
  wise?: unknown;
  use?: unknown;
  gist?: unknown;
  why?: unknown;
}

export interface Material {
  draft: string;
  queries: [string, string][];
  methods: Record<string, number>;
  items: Item[];
  edges: Record<string, number>;
  draft_seed: boolean;
  topics: { id: string; hits: number }[];
}

const bad = (what: string) => new PkmError('bad_output', `pkm material trả về dữ liệu không đúng dạng (${what}).`);

/** Checks the shape of `pkm VAULT material` output before anything is rendered from it. */
export function parseMaterial(v: unknown): Material {
  if (!isRecord(v) || typeof v.draft !== 'string') throw bad('draft');
  const queries = Array.isArray(v.queries) ? v.queries : null;
  if (!queries?.every((q) => Array.isArray(q) && q.length === 2 && q.every((s) => typeof s === 'string'))) throw bad('queries');
  if (!Array.isArray(v.items)) throw bad('items');
  for (const it of v.items) {
    if (!isRecord(it) || typeof it.id !== 'string' || typeof it.found_by !== 'string') throw bad('items[].id');
    if (it.via !== undefined && !(Array.isArray(it.via) && it.via.every((s) => isRecord(s) && typeof s.from === 'string' && typeof s.to === 'string'))) {
      throw bad('items[].via');
    }
  }
  const topics = Array.isArray(v.topics) ? v.topics.filter((t): t is { id: string; hits: number } => isRecord(t) && typeof t.id === 'string') : [];
  return {
    draft: v.draft,
    queries: queries as [string, string][],
    methods: isRecord(v.methods) ? (v.methods as Record<string, number>) : {},
    items: (v.items as Record<string, unknown>[]).map((it) => ({ ...it, type: typeof it.type === 'string' ? it.type : null, used: it.used === true }) as Item),
    edges: isRecord(v.edges) ? (v.edges as Record<string, number>) : {},
    draft_seed: v.draft_seed === true,
    topics,
  };
}

/** `{:.2f}` of Python for the scores shown on the page. */
const fixed2 = (n: number) => n.toFixed(2);

function stepText(s: Step): string {
  const arrow = `—${s.rel}→`;
  if (s.status === 'accepted') return `${arrow} ${wikilink(s.to)}`;
  const who = s.proposed_by || 'máy';
  const sc = typeof s.score === 'number' ? ` ${fixed2(s.score)}` : '';
  return `${arrow} ${wikilink(s.to)} *(máy gợi ý: ${who}${sc})*`;
}

const filled = (v: unknown) => v !== undefined && v !== null && v !== '' && v !== 0 && v !== false;

export function itemLines(it: Item, ticked: ReadonlySet<string>): string[] {
  const box = ticked.has(it.id) ? 'x' : ' ';
  const bits = [it.type || 'bài lưu'];
  if (it.sections?.length) bits.push(`khớp: ${it.sections.join(', ')}`);
  if (filled(it.highlights)) bits.push(`${it.highlights} đoạn tô`);
  for (const key of ['wise', 'use'] as const) if (filled(it[key])) bits.push(`${key} ${String(it[key])}`);
  const lines = [`- [${box}] ${wikilink(it.id)} · ${bits.join(' · ')}`];
  if (it.via?.length) lines.push(`    - đường: ${wikilink(it.via[0]!.from)} ${it.via.map(stepText).join(' ')}`);
  for (const key of ['gist', 'why'] as const) if (filled(it[key])) lines.push(`    - ${key}: ${String(it[key])}`);
  for (const h of it.first_highlights ?? []) lines.push(`    - ==${h}==`);
  return lines;
}

/** The page text. `ticked` keeps the user's `[x]` on items still listed. */
export function renderPage(r: Material, ticked: ReadonlySet<string>, now: Date): string {
  const d = r.draft;
  const method = Object.entries(r.methods).map(([k, v]) => `${k} ×${v}`).join(', ') || 'không có';
  const out = [
    '---',
    `draft: "[[${d.replace(/\.md$/, '')}]]"`,
    `generated: ${localIso(now, 'minutes')}`,
    'generated_by: pkm material',
    '---',
    '',
    `# Nguyên liệu: ${wikilink(d)}`,
    '',
    `> Máy gom lúc ${clockDay(now)}: tìm chữ cho ${r.queries.length} đoạn của bài (${method}), rồi đi graph ` +
      `(${r.edges.accepted ?? 0} cạnh đã duyệt, ${r.edges.proposed ?? 0} cạnh máy gợi ý).` +
      (r.draft_seed ? ' Bài đã có cạnh nên bản thân bài cũng là điểm xuất phát.' : ''),
    '> Máy chỉ chỉ chỗ. Mở note, đọc, tự quyết. Đánh dấu `[x]` note muốn dùng rồi chạy lại lệnh: note vào `uses` của bài (dùng là duyệt).',
    '',
  ];
  const used = r.items.filter((i) => i.used);
  if (used.length) {
    out.push(`## Đã dùng trong bài (${used.length})`, '');
    for (const it of used) out.push(`- ${wikilink(it.id)} · ${it.type || 'bài lưu'}`);
    out.push('');
  }
  const fresh = r.items.filter((i) => !i.used);
  const textHits = fresh.filter((i) => i.found_by === 'ohs');
  out.push(`## Tìm theo chữ (${textHits.length})`, '');
  for (const [head, types] of ROLES) {
    const group = textHits.filter((i) => types.includes(i.type));
    if (!group.length) continue;
    out.push(`### ${head} (${group.length})`, '');
    for (const it of group) out.push(...itemLines(it, ticked));
    out.push('');
  }
  const other = textHits.filter((i) => !ROLES.some(([, types]) => types.includes(i.type)));
  if (other.length) out.push(`### Khác (${other.length})`, '', ...other.flatMap((it) => itemLines(it, ticked)), '');
  const graphHits = fresh.filter((i) => i.found_by === 'graph');
  out.push(
    `## Graph đưa tới, tìm chữ không ra (${graphHits.length})`,
    '',
    'Mỗi note kèm đường đi từ một note tìm được. Bước in nghiêng là cạnh máy gợi ý, chưa ai duyệt.',
    '',
  );
  for (const it of graphHits) out.push(...itemLines(it, ticked));
  out.push('');
  if (r.topics.length) {
    out.push('## Cửa vào chủ đề', '', 'Thư mục chứa các note tìm được; mở để xem cả nhóm.', '');
    out.push(...r.topics.map((t) => `- ${wikilink(t.id)} · ${t.hits} note tìm được nằm ở đây`), '');
  }
  out.push('## Đoạn của bài máy đã tìm', '');
  const qs = r.queries.map(([label, q]) => `- ${label}: ${cpSlice(q, 120)}${cpLength(q) > 120 ? '…' : ''}`);
  out.push(...(qs.length ? qs : ['- (bài chưa có chữ: viết ý một câu hoặc dàn ý trước)']));
  return out.join('\n') + '\n';
}

const TICK = /^\s*- \[[xX]\] .*?\[\[([^\]|#]+)/;

/** Vault paths (with `.md`, NFC) the user ticked on a material page, in page order. */
export function tickedIn(page: string): string[] {
  const out: string[] = [];
  for (const line of splitLines(page)) {
    const m = TICK.exec(line);
    if (m) out.push(`${(m[1] ?? '').trim().normalize('NFC')}.md`);
  }
  return out;
}

/** Material page of a draft. */
export const pagePath = (draft: string): string => `${OUT_DIR}/${draft.split('/').pop()}`;
