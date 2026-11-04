import { parse, stringify } from 'yaml';
import type { NoteIO } from '../src/note-io';
import { FM_RE, type Frontmatter } from '../src/text';
import { MemoryIO } from './memory-io';

/**
 * In-memory NoteIO. `editFrontmatter` behaves like Obsidian's processFrontMatter: the YAML block is re-serialized
 * with LF line endings, the body is kept as is.
 */
export class MemoryNoteIO extends MemoryIO implements NoteIO {
  readonly renamed: [string, string][] = [];

  async editFrontmatter(path: string, fn: (fm: Frontmatter) => void): Promise<void> {
    const text = await this.read(path);
    const m = FM_RE.exec(text);
    const fm = ((m ? parse(m[1] ?? '') : null) ?? {}) as Frontmatter;
    fn(fm);
    const body = m ? text.slice(m[0].length) : text;
    this.files.set(path, `---\n${stringify(fm)}---\n${body}`);
  }

  async process(path: string, fn: (text: string) => string): Promise<void> {
    this.files.set(path, fn(await this.read(path)));
  }

  async append(path: string, text: string): Promise<void> {
    this.files.set(path, (this.files.get(path) ?? '') + text);
  }

  async rename(from: string, to: string): Promise<void> {
    this.files.set(to, await this.read(from));
    this.files.delete(from);
    this.renamed.push([from, to]);
  }

  findFile(path: string): string | null {
    if (this.files.has(path)) return path;
    const want = path.normalize('NFC');
    return [...this.files.keys()].find((p) => p.normalize('NFC') === want) ?? null;
  }

  resolveLink(linkpath: string, _from: string): string | null {
    const exact = this.findFile(linkpath.endsWith('.md') ? linkpath : `${linkpath}.md`);
    if (exact) return exact;
    const byName = [...this.files.keys()].filter((p) => p.split('/').pop() === `${linkpath}.md`);
    return byName.length === 1 ? byName[0]! : null;
  }
}
