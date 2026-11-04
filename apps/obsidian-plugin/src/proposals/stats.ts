// Review statistics per proposal kind: decided, accepted, and how often a source note was opened between opening the
// proposal and deciding (a high accept rate with few source opens is nodding along with the machine). Input is the
// review log the plugin writes; output is `_E11/Đo duyệt.md`.
import { parseYaml } from 'obsidian';
import type { NoteIO } from '../note-io';
import { REVIEW_LOG } from '../review-log';
import { asList, isRecord, localIso, pyRound, splitNote } from '../text';
import { notePath, QUEUE } from './apply';

export const STATS = '_E11/Đo duyệt.md';

interface Event {
  at: string;
  event: string;
  path: string;
  status?: unknown;
}

function parseEvents(text: string): Event[] {
  const out: Event[] = [];
  for (const line of text.split('\n')) {
    if (!line.trim()) continue;
    try {
      const v: unknown = JSON.parse(line);
      if (isRecord(v) && typeof v.at === 'string' && typeof v.event === 'string' && typeof v.path === 'string') {
        out.push({ at: v.at, event: v.event, path: v.path, status: v.status });
      }
    } catch {
      // a line cut short by a crash: skip it
    }
  }
  return out;
}

interface Row {
  decided: number;
  accepted: number;
  logged: number;
  noLog: number;
  openedSource: number;
  seconds: number[];
}

/** Seconds between two log stamps, compared on their first 19 characters like the Python script. */
const secondsBetween = (start: string, end: string) => Math.trunc((Date.parse(`${end.slice(0, 19)}Z`) - Date.parse(`${start.slice(0, 19)}Z`)) / 1000);

const pct = (a: number, b: number) => (b ? `${pyRound((100 * a) / b)}%` : '-');

/** Builds the statistics page; `rows` is the number of kinds with a decided proposal. */
export async function reviewStats(io: NoteIO, now: Date): Promise<{ text: string; rows: number }> {
  const events = (await io.exists(REVIEW_LOG)) ? parseEvents(await io.read(REVIEW_LOG)) : [];
  const rows = new Map<string, Row>();
  const files = ((await io.list(QUEUE))?.files ?? []).filter((f) => f.endsWith('.md')).sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
  for (const file of files) {
    const { fm } = splitNote(await io.read(file), parseYaml);
    if (fm.type !== 'proposal' || (fm.status !== 'đã áp dụng' && fm.status !== 'đã bỏ')) continue;
    const kind = String(fm.kind ?? '');
    const me = file.normalize('NFC');
    const sources = new Set(
      asList(fm.notes)
        .map((n) => (typeof n === 'string' ? notePath(n) : null))
        .filter((p): p is string => p !== null)
        .map((p) => p.normalize('NFC')),
    );
    let r = rows.get(kind);
    if (!r) rows.set(kind, (r = { decided: 0, accepted: 0, logged: 0, noLog: 0, openedSource: 0, seconds: [] }));
    r.decided += 1;
    if (fm.status === 'đã áp dụng') r.accepted += 1;
    const decided = events.filter((e) => e.path === me && e.event === 'status' && (e.status === 'nhận' || e.status === 'bỏ'));
    const opened = events.filter((e) => e.path === me && e.event === 'open');
    const start = opened[0]?.at, end = decided[decided.length - 1]?.at;
    if (start === undefined || end === undefined || start > end) {
      r.noLog += 1;
      continue;
    }
    r.logged += 1;
    if (events.some((e) => e.event === 'open' && sources.has(e.path) && start <= e.at && e.at <= end)) r.openedSource += 1;
    r.seconds.push(secondsBetween(start, end));
  }
  const lines = [
    `# Đo duyệt đề xuất, ${localIso(now, 'minutes')}`,
    '',
    'Máy tự đo từ `review-log.jsonl` (plugin PKM, chỉ ghi trong vault). "Mở nguồn": trong lúc từ khi mở ' +
      'đề xuất tới khi đổi `status`, bạn có mở ít nhất một note nguồn. Tỷ lệ nhận cao mà ít mở nguồn là dấu hiệu ' +
      'gật đầu theo máy (HG13).',
    '',
    '| Kiểu | Đã quyết | Nhận | Có log | Mở nguồn | Giây mỗi đề xuất (trung vị) |',
    '|---|---|---|---|---|---|',
  ];
  for (const kind of [...rows.keys()].sort((a, b) => (a < b ? -1 : a > b ? 1 : 0))) {
    const r = rows.get(kind)!;
    const sec = [...r.seconds].sort((a, b) => a - b);
    const median = sec.length ? String(sec[Math.floor(sec.length / 2)]) : '-';
    lines.push(`| ${kind} | ${r.decided} | ${pct(r.accepted, r.decided)} | ${r.logged} | ${pct(r.openedSource, r.logged)} | ${median} |`);
  }
  const text = lines.join('\n') + '\n';
  await io.write(STATS, text);
  return { text, rows: rows.size };
}
