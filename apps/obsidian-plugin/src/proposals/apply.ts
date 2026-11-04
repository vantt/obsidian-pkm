// The review queue `_Đề xuất/`: list proposals, record the user's decision, apply what was accepted, close what was
// rejected, log every change to `_E11/proposals-log.jsonl`, and revert from that log. `pkm VAULT propose --create`
// writes the proposal notes; nothing changes in a user note until the user accepts and applies.
import { parseYaml } from 'obsidian';
import { editFrontmatter, type NoteIO } from '../note-io';
import { asList, cleanName, compactStamp, cpSlice, dirname, type Frontmatter, isRecord, localIso, mapBody, splitNote, title, truthy, wikilink } from '../text';
import { pyJson } from '../workshop/digest';

export const QUEUE = '_Đề xuất';
export const LOG = '_E11/proposals-log.jsonl';
export const IDEAS = 'Ý';
export const PEOPLE = 'Người';
const HIGHLIGHT = /==([^=\n]{8,}?)==/g;

export const KIND_TITLES: Record<string, string> = {
  duplicate: 'Trùng',
  'same-topic': 'Cùng chủ đề',
  'fix-link': 'Sửa liên kết',
  'type-batch': 'Gán loại',
  related: 'Liên quan',
  idea: 'Rút ý',
  person: 'Người',
};

export const PERSON_PAGE = `---
type: person
---
\`\`\`base
filters:
  and:
    - file.hasLink(this.file)
    - file.ext == "md"
views:
  - type: table
    name: Note nhắc tới
    order:
      - file.name
      - type
      - file.folder
      - created
\`\`\`
`;

/** Decisions the user can mark on a waiting proposal. */
export type Decision = 'nhận' | 'bỏ';
const DECIDED = ['nhận', 'bỏ'];

export interface QueuedProposal {
  path: string;
  kind: string;
  status: string;
  /** Notes the proposal is about, as wikilinks. */
  notes: string[];
  reason: string;
  rejectReason: string;
}

/** Vault path of a proposal's `[[path|title]]` entry: the target plus `.md`. */
export function notePath(link: string): string | null {
  const m = /^\[\[([^|\]]+)/.exec(link);
  return m ? `${m[1]}.md` : null;
}

async function queueFiles(io: NoteIO): Promise<string[]> {
  const entry = await io.list(QUEUE);
  return (entry?.files ?? []).filter((f) => f.endsWith('.md')).sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
}

const str = (v: unknown): string => (v === null || v === undefined ? '' : String(v));

/** Every proposal note in the queue, in file name order. */
export async function listQueue(io: NoteIO): Promise<QueuedProposal[]> {
  const out: QueuedProposal[] = [];
  for (const path of await queueFiles(io)) {
    const { fm } = splitNote(await io.read(path), parseYaml);
    if (fm.type !== 'proposal') continue;
    out.push({
      path,
      kind: str(fm.kind),
      status: str(fm.status).trim(),
      notes: asList(fm.notes).filter((n): n is string => typeof n === 'string'),
      reason: str(fm.reason),
      rejectReason: str(fm.reject_reason ?? fm['lý do bỏ']),
    });
  }
  return out;
}

/** Marks a proposal `nhận` or `bỏ` (with an optional reason), or back to `chờ`. Applying is a separate step. */
export async function decide(io: NoteIO, path: string, decision: Decision | 'chờ', reason = ''): Promise<void> {
  await editFrontmatter(io, path, (fm) => {
    fm.status = decision;
    if (decision === 'bỏ' && reason.trim()) fm.reject_reason = reason.trim();
  });
}

/** Old text of every file a proposal changed; `null` for a file it created. */
type Changes = Map<string, string | null>;

class Editor {
  readonly changed: Changes = new Map();

  constructor(private readonly io: NoteIO) {}

  /** The existing file a proposal entry points to. */
  target(link: string): string {
    const p = notePath(link);
    const found = p && this.io.findFile(p);
    if (!found) throw new Error(`không thấy note ${p ?? link}`);
    return found;
  }

  private async remember(path: string): Promise<void> {
    if (!this.changed.has(path)) this.changed.set(path, await this.io.read(path));
  }

  async frontmatter(path: string, fn: (fm: Frontmatter) => void): Promise<void> {
    await this.remember(path);
    await editFrontmatter(this.io, path, fn);
  }

  async body(path: string, fn: (body: string) => string): Promise<void> {
    await this.remember(path);
    await this.io.process(path, (text) => mapBody(text, fn));
  }

  async create(path: string, text: string): Promise<void> {
    await this.io.write(path, text);
    this.changed.set(path, null);
  }

  /** Puts back every file touched so far, newest first. */
  async rollback(): Promise<void> {
    for (const [path, old] of [...this.changed].reverse()) {
      try {
        if (old === null) await this.io.remove(path);
        else await this.io.write(path, old);
      } catch (e) {
        console.error(`pkm: could not restore ${path}`, e);
      }
    }
  }
}

/** A free `folder/name.md`, then `name 2.md`, `name 3.md`, ... */
async function freePath(io: NoteIO, folder: string, name: string): Promise<string> {
  const clean = cleanName(name);
  let path = `${folder}/${clean}.md`;
  for (let n = 2; await io.exists(path); n++) path = `${folder}/${clean} ${n}.md`;
  return path;
}

/** PyYAML's single-quoted scalar, which is how it writes a string starting with `[` (a wikilink). */
const yamlQuoted = (s: string) => `'${s.replace(/'/g, "''")}'`;

/** The draft idea note: highlights of the source as quotes, the idea itself left for the user to write. */
export function ideaDraft(topic: string | null, sourceLink: string, highlights: string[]): string {
  const fm = [
    'type: note',
    `topic: ${yamlQuoted(topic ? wikilink(topic) : '')}`,
    "gist: ''",
    'source:',
    `- ${yamlQuoted(sourceLink)}`,
    'wise: null',
    'use: null',
    'wf:',
    '- to-clarify',
  ];
  const quotes = highlights.map((h) => `> ${h.trim()}\n\n`).join('');
  return `---\n${fm.join('\n')}\n---\n## Ý chính (lời mình)\n\n\n## Từ nguồn\n\n${quotes}Từ ${sourceLink}\n\n## Mình thấy gì, đem thử ở đâu\n`;
}

function addTo(fm: Frontmatter, key: string, value: string): void {
  const list = asList(fm[key]);
  if (!list.includes(value)) list.push(value);
  fm[key] = list;
}

async function applyAccepted(io: NoteIO, ed: Editor, pfm: Frontmatter): Promise<void> {
  const notes = asList(pfm.notes).filter((n): n is string => typeof n === 'string');
  const data = isRecord(pfm.data) ? pfm.data : {};
  const need = (n: number) => {
    if (notes.length < n) throw new Error(`đề xuất cần ít nhất ${n} note trong notes`);
  };
  const kind = str(pfm.kind);
  switch (kind) {
    case 'duplicate':
      need(2);
      for (const n of notes.slice(1)) await ed.frontmatter(ed.target(n), (fm) => void (fm.duplicate_of = notes[0]));
      return;
    case 'same-topic':
    case 'related': {
      need(2);
      const [a, b] = [notes[0]!, notes[1]!];
      await ed.frontmatter(ed.target(a), (fm) => addTo(fm, 'related', b));
      await ed.frontmatter(ed.target(b), (fm) => addTo(fm, 'related', a));
      return;
    }
    case 'fix-link': {
      need(2);
      if (typeof data.from_text !== 'string' || typeof data.to !== 'string') throw new Error('đề xuất thiếu data.from_text hoặc data.to');
      const from = `[[${data.from_text}`, to = `[[${data.to.replace(/\.md$/, '')}`;
      for (const n of notes.slice(0, -1)) await ed.body(ed.target(n), (body) => body.split(from).join(to));
      return;
    }
    case 'type-batch':
      need(1);
      for (const n of notes) {
        await ed.frontmatter(ed.target(n), (fm) => {
          const v = 'proposed_type' in fm ? fm.proposed_type : data.value;
          delete fm.proposed_type;
          fm.type = v;
        });
      }
      return;
    case 'idea': {
      need(1);
      const src = ed.target(notes[0]!);
      const { body } = splitNote(await io.read(src), parseYaml);
      let topic: string | null = null;
      for (let d = dirname(src); d; d = dirname(d)) {
        const page = `${d}/${d.split('/').pop()} (chủ đề).md`;
        if (await io.exists(page)) {
          topic = page;
          break;
        }
      }
      const highlights = [...body.matchAll(HIGHLIGHT)].map((m) => m[1] ?? '');
      const path = await freePath(io, IDEAS, `${cpSlice(title(src), 60).trim()} - ý`);
      await ed.create(path, ideaDraft(topic, notes[0]!, highlights));
      return;
    }
    case 'person': {
      need(1);
      const name = typeof data.value === 'string' ? data.value.trim() : '';
      if (!name || cleanName(name) !== name) throw new Error('đề xuất người thiếu tên hợp lệ (data.value)');
      const targets = notes.map((n) => ed.target(n));
      const page = `${PEOPLE}/${name}.md`;
      if (!(await io.exists(page))) await ed.create(page, PERSON_PAGE);
      const who = `[[${PEOPLE}/${name}|${name}]]`;
      for (const t of targets) await ed.frontmatter(t, (fm) => addTo(fm, 'who', who));
      return;
    }
    default:
      throw new Error(`kiểu đề xuất lạ: ${kind || '(trống)'}`);
  }
}

export interface ApplyResult {
  /** `kind|decision` -> count */
  counts: Record<string, number>;
  /** Proposals left as they were because applying failed. */
  failed: { path: string; message: string }[];
}

/**
 * Applies every proposal marked `nhận`, closes those marked `bỏ`, one log line each. A proposal whose edits fail is
 * rolled back and left marked, so it can be fixed and applied again.
 */
export async function applyDecisions(io: NoteIO, now: Date): Promise<ApplyResult> {
  const stamp = localIso(now, 'seconds');
  const result: ApplyResult = { counts: {}, failed: [] };
  for (const path of await queueFiles(io)) {
    const raw = await io.read(path);
    const { fm: pfm } = splitNote(raw, parseYaml);
    const status = str(pfm.status).trim();
    if (pfm.type !== 'proposal' || !DECIDED.includes(status)) continue;
    const ed = new Editor(io);
    try {
      if (status === 'nhận') await applyAccepted(io, ed, pfm);
      await editFrontmatter(io, path, (fm) => {
        fm.status = status === 'nhận' ? 'đã áp dụng' : 'đã bỏ';
        fm.closed = stamp;
        if (status === 'nhận') fm.asserted_at = stamp.slice(0, 10);
      });
    } catch (e) {
      await ed.rollback();
      result.failed.push({ path, message: e instanceof Error ? e.message : String(e) });
      continue;
    }
    const entry = {
      at: stamp,
      proposal: path,
      proposal_old: raw,
      decision: status,
      reject_reason: truthy(pfm.reject_reason) ? pfm.reject_reason : (pfm['lý do bỏ'] ?? null),
      changes: Object.fromEntries(ed.changed),
    };
    await io.append(LOG, pyJson(entry, false) + '\n');
    const key = `${str(pfm.kind)}|${status}`;
    result.counts[key] = (result.counts[key] ?? 0) + 1;
  }
  return result;
}

interface LogEntry {
  proposal: string;
  proposal_old: string;
  changes: Record<string, string | null>;
}

function parseLog(text: string): LogEntry[] {
  return text
    .split('\n')
    .map((line, i) => [line.trim(), i + 1] as const)
    .filter(([line]) => line)
    .map(([line, n]) => {
      let v: unknown;
      try {
        v = JSON.parse(line);
      } catch {
        throw new Error(`${LOG} dòng ${n} không phải JSON`);
      }
      const ok =
        isRecord(v) &&
        typeof v.proposal === 'string' &&
        typeof v.proposal_old === 'string' &&
        isRecord(v.changes) &&
        Object.values(v.changes).every((c) => c === null || typeof c === 'string');
      if (!ok) throw new Error(`${LOG} dòng ${n} thiếu proposal, proposal_old hoặc changes`);
      return v as unknown as LogEntry;
    });
}

/** Undoes every logged decision, newest first, then sets the log aside as `.reverted-<stamp>`. */
export async function revertAll(io: NoteIO, now: Date): Promise<number> {
  if (!(await io.exists(LOG))) throw new Error(`chưa có ${LOG}: chưa áp dụng đề xuất nào`);
  const entries = parseLog(await io.read(LOG));
  for (const e of [...entries].reverse()) {
    for (const [path, old] of Object.entries(e.changes)) {
      if (old !== null) await io.write(path, old);
      else if (await io.exists(path)) await io.remove(path);
    }
    await io.write(e.proposal, e.proposal_old);
  }
  await io.rename(LOG, `${LOG}.reverted-${compactStamp(now)}`);
  return entries.length;
}
