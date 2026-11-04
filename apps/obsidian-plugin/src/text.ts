// Small text helpers shared by the proposal, material and zone commands: frontmatter split, line endings, local
// timestamps in the format the earlier Python scripts wrote, and a few Python string semantics the ported layouts
// depend on (code-point slicing, splitlines, truthiness, round half to even).

/** Frontmatter block at the very start of a note, LF or CRLF. Group 1 is the YAML text. */
export const FM_RE = /^---[ \t]*\r?\n([\s\S]*?)\r?\n---[ \t]*\r?\n/;

export type Frontmatter = Record<string, unknown>;

/** `(frontmatter, body)`; a missing, empty, unparsable or non-mapping frontmatter is `{}`. */
export function splitNote(text: string, parseYaml: (t: string) => unknown): { fm: Frontmatter; body: string; hasFm: boolean } {
  const m = FM_RE.exec(text);
  if (!m) return { fm: {}, body: text, hasFm: false };
  let fm: unknown;
  try {
    fm = parseYaml(m[1] ?? '');
  } catch {
    fm = null;
  }
  return { fm: isRecord(fm) ? fm : {}, body: text.slice(m[0].length), hasFm: true };
}

/** Rewrites the line endings of the frontmatter block to CRLF; the body is left as is. */
export function crlfFrontmatter(text: string): string {
  const m = /^---[ \t]*\r?\n[\s\S]*?\r?\n---[ \t]*(?:\r?\n|$)/.exec(text);
  if (!m) return text;
  return m[0].replace(/\r?\n/g, '\r\n') + text.slice(m[0].length);
}

/** Applies `fn` to the body only (the text after the frontmatter block). */
export function mapBody(text: string, fn: (body: string) => string): string {
  const m = FM_RE.exec(text);
  const head = m ? m[0] : '';
  return head + fn(text.slice(head.length));
}

export function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

/** Python truthiness for values parsed from YAML or JSON. */
export function truthy(v: unknown): boolean {
  if (v === null || v === undefined || v === false || v === 0 || v === '') return false;
  if (Array.isArray(v)) return v.length > 0;
  if (isRecord(v)) return Object.keys(v).length > 0;
  return true;
}

/** A frontmatter value as a list: missing or empty is `[]`, a single value is a one-item list. */
export function asList(v: unknown): unknown[] {
  if (Array.isArray(v)) return [...v];
  return truthy(v) ? [v] : [];
}

/** `s[:n]` of Python: counts code points, not UTF-16 units. */
export function cpSlice(s: string, n: number): string {
  const cps = Array.from(s);
  return cps.length <= n ? s : cps.slice(0, n).join('');
}

export const cpLength = (s: string): number => Array.from(s).length;

/** `str.splitlines()` of Python. */
export function splitLines(text: string): string[] {
  const out = text.split(/\r\n|[\n\r\v\f\x1c\x1d\x1e\x85\u2028\u2029]/);
  if (out.length && out[out.length - 1] === '') out.pop();
  return out;
}

/** `round(x)` of Python: halves go to the even neighbour. */
export function pyRound(x: number): number {
  const r = Math.round(x);
  return Math.abs(x % 1) === 0.5 && r % 2 !== 0 ? r - 1 : r;
}

/** Last path segment without `.md`. */
export const title = (path: string): string => (path.split('/').pop() ?? path).replace(/\.md$/, '');

export const dirname = (path: string): string => path.split('/').slice(0, -1).join('/');

/** `[[path-without-.md|title]]`, as the Python scripts link notes. */
export const wikilink = (path: string): string => `[[${path.replace(/\.md$/, '')}|${title(path)}]]`;

/** Target of a `[[target|alias]]` or `[[target#heading]]` string, or null. */
export function linkTarget(v: unknown): string | null {
  if (typeof v !== 'string') return null;
  const m = /\[\[([^|\]#]+)/.exec(v);
  return m ? (m[1] ?? '').trim() : null;
}

/** Characters not allowed in a note name, replaced by spaces (as the Python scripts did). */
export const cleanName = (name: string): string => name.replace(/[\\/:*?"<>|#^[\]]/g, ' ').trim();

const pad = (n: number, w = 2) => String(n).padStart(w, '0');

/** Local time as Python's `datetime.now().isoformat(timespec=...)`: no offset. */
export function localIso(d: Date, timespec: 'seconds' | 'minutes'): string {
  const date = localDate(d);
  const time = `${pad(d.getHours())}:${pad(d.getMinutes())}`;
  return `${date}T${timespec === 'seconds' ? `${time}:${pad(d.getSeconds())}` : time}`;
}

export const localDate = (d: Date): string => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;

/** `strftime('%Y%m%dT%H%M%S')`. */
export const compactStamp = (d: Date): string =>
  `${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}T${pad(d.getHours())}${pad(d.getMinutes())}${pad(d.getSeconds())}`;

/** `strftime('%H:%M %d/%m')`. */
export const clockDay = (d: Date): string => `${pad(d.getHours())}:${pad(d.getMinutes())} ${pad(d.getDate())}/${pad(d.getMonth() + 1)}`;
