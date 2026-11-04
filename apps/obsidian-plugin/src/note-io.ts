// Note access for the commands that edit the user's notes (proposals, material). Everything goes through Obsidian's
// API so a note open in an editor is updated in place: `fileManager.processFrontMatter` for properties,
// `vault.process` for text, `fileManager.trashFile` to delete. Logs under _E11/ go through the adapter, like the
// review log.
import { type App, normalizePath, TFile } from 'obsidian';
import type { VaultIO } from './workshop/apply';
import { ObsidianVaultIO } from './workshop/obsidian-io';
import { crlfFrontmatter, type Frontmatter } from './text';

export interface NoteIO extends VaultIO {
  /** Edits the properties of an existing note (Obsidian rewrites the frontmatter block, the body is kept). */
  editFrontmatter(path: string, fn: (fm: Frontmatter) => void): Promise<void>;
  /** Replaces the text of an existing note atomically. */
  process(path: string, fn: (text: string) => string): Promise<void>;
  /** Appends to a log file, creating it and its folder when missing. */
  append(path: string, text: string): Promise<void>;
  rename(from: string, to: string): Promise<void>;
  /** The vault path of a file, matching `path` exactly or after Unicode normalization; null when absent. */
  findFile(path: string): string | null;
  /** The note a `[[linkpath]]` in `from` points to, as Obsidian resolves it; null when unresolved. */
  resolveLink(linkpath: string, from: string): string | null;
}

/** Edits properties and keeps CRLF line endings in the frontmatter of a note that had them. */
export async function editFrontmatter(io: NoteIO, path: string, fn: (fm: Frontmatter) => void): Promise<void> {
  const crlf = (await io.read(path)).includes('\r\n');
  await io.editFrontmatter(path, fn);
  if (crlf) await io.process(path, crlfFrontmatter);
}

export class ObsidianNoteIO extends ObsidianVaultIO implements NoteIO {
  constructor(private readonly obsidian: App) {
    super(obsidian);
  }

  private file(path: string): TFile {
    const f = this.obsidian.vault.getAbstractFileByPath(normalizePath(path));
    if (!(f instanceof TFile)) throw new Error(`không thấy note ${path}`);
    return f;
  }

  async editFrontmatter(path: string, fn: (fm: Frontmatter) => void): Promise<void> {
    await this.obsidian.fileManager.processFrontMatter(this.file(path), (fm: Frontmatter) => fn(fm));
  }

  async process(path: string, fn: (text: string) => string): Promise<void> {
    await this.obsidian.vault.process(this.file(path), fn);
  }

  async append(path: string, text: string): Promise<void> {
    const adapter = this.obsidian.vault.adapter;
    const p = normalizePath(path);
    const dir = p.split('/').slice(0, -1).join('/');
    if (dir && !(await adapter.exists(dir))) await adapter.mkdir(dir);
    if (await adapter.exists(p)) await adapter.append(p, text);
    else await adapter.write(p, text);
  }

  async rename(from: string, to: string): Promise<void> {
    await this.obsidian.vault.adapter.rename(normalizePath(from), normalizePath(to));
  }

  findFile(path: string): string | null {
    const p = normalizePath(path);
    if (this.obsidian.vault.getAbstractFileByPath(p) instanceof TFile) return p;
    const want = p.normalize('NFC');
    return this.obsidian.vault.getFiles().find((f) => f.path.normalize('NFC') === want)?.path ?? null;
  }

  resolveLink(linkpath: string, from: string): string | null {
    return this.obsidian.metadataCache.getFirstLinkpathDest(linkpath, from)?.path ?? null;
  }
}
