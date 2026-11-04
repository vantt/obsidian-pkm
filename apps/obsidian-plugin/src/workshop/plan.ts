// What "Dựng xưởng" writes: static templates bundled with the plugin, with a few variables filled in. Output is
// byte-identical to the earlier Python workshop script for the same inputs, so files it wrote count as ours.
import home from './templates/home.md';
import topicPage from './templates/topic-page.md';
import topicBase from './templates/topic.base';
import workshopBase from './templates/workshop.base';
import clip from './templates/note-templates/clip.md';
import concept from './templates/note-templates/concept.md';
import idea from './templates/note-templates/idea.md';
import moc from './templates/note-templates/moc.md';
import post from './templates/note-templates/post.md';
import project from './templates/note-templates/project.md';
import quote from './templates/note-templates/quote.md';
import toc from './templates/note-templates/toc.md';
import homepageConfig from './templates/homepage.json';

/** A folder needs this many notes (recursively) to get a topic page. */
export const MIN_NOTES = 3;
/** Always listed under "Mới lưu"; the workshop creates it. */
export const DEFAULT_INBOX = 'Inbox';
export const HOME = 'Trang chủ.md';
export const TEMPLATES_FOLDER = '_Templates';

/** Note templates, by the name the user sees in `_Templates/`. */
export const NOTE_TEMPLATES: Record<string, string> = {
  'Ý': idea,
  'Bài viết': post,
  'Bài cắt': clip,
  'Trích': quote,
  'Khái niệm': concept,
  'Dự án': project,
  'Mục lục': toc,
  'Bản đồ (MOC)': moc,
};

/** Quick-create links on the home page: label, new note path (no extension), template. */
const QUICK: [string, string, string][] = [
  ['Ý mới', 'Ý/Ý mới', 'Ý'],
  ['Bài viết mới', 'Bài viết/Bài mới', 'Bài viết'],
  ['Bài cắt mới', 'Inbox/Bài cắt mới', 'Bài cắt'],
];

const EMPTY_FOLDERS = ['Bài viết', 'Ý', 'Inbox'];

export interface PlanInput {
  vaultName: string;
  /** Vault-relative topic folders (see findTopicFolders). */
  topicFolders: string[];
  /** Extra capture folders for "Mới lưu", besides Inbox. */
  inboxFolders: string[];
}

/** Fills `{{key}}` placeholders; every placeholder in the template must have a value. */
export function fill(template: string, vars: Record<string, string>): string {
  return template.replace(/\{\{(\w+)\}\}/g, (_, key: string) => {
    const v = vars[key];
    if (v === undefined) throw new Error(`template variable without value: ${key}`);
    return v;
  });
}

/** Percent-encoding of Python's urllib.parse.quote(s, safe=''): only A-Z a-z 0-9 _ . - ~ stay as they are. */
export function pyQuote(s: string): string {
  return encodeURIComponent(s).replace(/[!'()*]/g, (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`);
}

export function quickLinks(vaultName: string): string {
  return QUICK.map(([label, file, tpl]) => {
    const content = NOTE_TEMPLATES[tpl];
    if (content === undefined) throw new Error(`unknown template ${tpl}`);
    const q = `vault=${pyQuote(vaultName)}&file=${pyQuote(file)}&content=${pyQuote(content)}`;
    return `[${label}](obsidian://new?${q})`;
  }).join(' · ');
}

/** The "Mới lưu" filter line of the workshop base: one `file.inFolder` per capture folder. */
export function inboxFilter(extra: string[]): string {
  const folders = [...extra.filter((f) => f !== DEFAULT_INBOX), DEFAULT_INBOX];
  return `'${folders.map((f) => `file.inFolder("${f}")`).join(' || ')}'`;
}

export function topicPagePath(folder: string): string {
  const name = folder.split('/').pop() ?? folder;
  return `${folder}/${name} (chủ đề).md`;
}

/** Every file to write, vault-relative path to content, in a stable order. */
export function buildPlan(input: PlanInput): Map<string, string> {
  const files = new Map<string, string>();
  files.set(HOME, fill(home, { quick: quickLinks(input.vaultName) }));
  files.set('_Xưởng/Xưởng.base', fill(workshopBase, { inboxFilter: inboxFilter(input.inboxFolders) }));
  files.set('_Xưởng/Chủ đề.base', topicBase);
  for (const [name, text] of Object.entries(NOTE_TEMPLATES)) files.set(`${TEMPLATES_FOLDER}/${name}.md`, text);
  for (const folder of input.topicFolders) {
    const name = folder.split('/').pop() ?? folder;
    files.set(topicPagePath(folder), fill(topicPage, { name }));
  }
  for (const folder of EMPTY_FOLDERS) files.set(`${folder}/.keep`, '');
  return files;
}

/** Plugin configs the workshop sets; homepage opens the home page on startup. */
export function homepageSettings(): unknown {
  return structuredClone(homepageConfig);
}

export function templaterSettings(old: unknown): Record<string, unknown> {
  const cfg = typeof old === 'object' && old !== null && !Array.isArray(old) ? { ...(old as Record<string, unknown>) } : {};
  cfg.templates_folder = TEMPLATES_FOLDER;
  cfg.folder_templates = [
    { folder: 'Bài viết', template: `${TEMPLATES_FOLDER}/Bài viết.md` },
    { folder: 'Ý', template: `${TEMPLATES_FOLDER}/Ý.md` },
  ];
  return cfg;
}

export interface FolderLister {
  /** Direct children of a vault-relative folder, as vault-relative paths; null when it does not exist. */
  list(path: string): Promise<{ files: string[]; folders: string[] } | null>;
}

const base = (p: string) => p.split('/').pop() ?? p;
const byName = (a: string, b: string) => (base(a) < base(b) ? -1 : base(a) > base(b) ? 1 : 0);

async function countNotes(fs: FolderLister, folder: string): Promise<number> {
  const entry = await fs.list(folder);
  if (!entry) return 0;
  let n = entry.files.filter((f) => f.endsWith('.md')).length;
  for (const sub of entry.folders) n += await countNotes(fs, sub);
  return n;
}

/**
 * Topic folders: for each root, every `root/<group>/<topic>` whose name does not start with `_` and that holds at
 * least MIN_NOTES notes, sorted by name within each level.
 */
export async function findTopicFolders(fs: FolderLister, roots: string[]): Promise<string[]> {
  const out: string[] = [];
  for (const root of roots) {
    const top = await fs.list(root);
    if (!top) continue;
    for (const group of [...top.folders].sort(byName)) {
      const inner = await fs.list(group);
      if (!inner) continue;
      for (const topic of [...inner.folders].sort(byName)) {
        if (base(topic).startsWith('_')) continue;
        if ((await countNotes(fs, topic)) >= MIN_NOTES) out.push(topic);
      }
    }
  }
  return out;
}
