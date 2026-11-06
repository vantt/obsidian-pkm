// Plugin settings: where pkm is, how long to wait for it, the graph index the writing commands read, the vault
// folders the workshop reads, whether the file explorer follows the open note, and side-by-side reading.

export interface PkmSettings {
  /** Path to the pkm binary; empty = the installer's default location, detected at each run. */
  pkmPath: string;
  /** Kill pkm after this many seconds. */
  timeoutSeconds: number;
  /** Graph index passed as `--db` to material, zones and pairs; empty = pkm's default `VAULT/.kg/graph.db`. `pkm
   * setup` fills it in data.json. */
  graphDbPath: string;
  /**
   * Folders whose grandchildren get a topic page (root/<group>/<topic>, like a PARA layout). Empty = no topic pages.
   */
  topicRoots: string[];
  /** Extra capture folders listed under "Mới lưu" on the home page, besides `Inbox`. */
  inboxFolders: string[];
  /** Scroll the file explorer to the note when it opens from a link, the switcher or a tab (nav-follow.ts). */
  navFollow: boolean;
  /** Lock button and commands for scrolling two side-by-side notes together (compare-scroll/). */
  compareScroll: boolean;
}

export const DEFAULT_SETTINGS: PkmSettings = {
  pkmPath: '',
  timeoutSeconds: 90,
  graphDbPath: '',
  topicRoots: [],
  inboxFolders: [],
  navFollow: true,
  compareScroll: true,
};

export const MIN_TIMEOUT = 5;
export const MAX_TIMEOUT = 3600;

/** Vault-relative folder: forward slashes, no leading/trailing slash, no `..`, no quotes (they end up in .base filters). */
export function cleanFolder(raw: string): string | null {
  const f = raw.trim().replace(/\\/g, '/').replace(/^\/+|\/+$/g, '').replace(/\/{2,}/g, '/');
  if (!f || f.split('/').some((s) => s === '..' || s === '.') || /["'\n\r]/.test(f)) return null;
  return f.normalize('NFC');
}

/** One folder per line (or comma separated); invalid and duplicate entries dropped. */
export function parseFolderList(text: string): string[] {
  const out: string[] = [];
  for (const part of text.split(/[\n,]/)) {
    const f = cleanFolder(part);
    if (f && !out.includes(f)) out.push(f);
  }
  return out;
}

/** Settings loaded from data.json, which the user can edit by hand: keep only well-formed values. */
export function normalizeSettings(raw: unknown): PkmSettings {
  const s: PkmSettings = { ...DEFAULT_SETTINGS, topicRoots: [], inboxFolders: [] };
  if (typeof raw !== 'object' || raw === null) return s;
  const r = raw as Record<string, unknown>;
  if (typeof r.pkmPath === 'string') s.pkmPath = r.pkmPath.trim();
  if (typeof r.graphDbPath === 'string') s.graphDbPath = r.graphDbPath.trim();
  if (typeof r.navFollow === 'boolean') s.navFollow = r.navFollow;
  if (typeof r.compareScroll === 'boolean') s.compareScroll = r.compareScroll;
  if (typeof r.timeoutSeconds === 'number' && Number.isFinite(r.timeoutSeconds)) {
    s.timeoutSeconds = Math.min(MAX_TIMEOUT, Math.max(MIN_TIMEOUT, Math.round(r.timeoutSeconds)));
  }
  const list = (v: unknown) => (Array.isArray(v) ? parseFolderList(v.filter((x) => typeof x === 'string').join('\n')) : []);
  s.topicRoots = list(r.topicRoots);
  s.inboxFolders = list(r.inboxFolders);
  return s;
}
