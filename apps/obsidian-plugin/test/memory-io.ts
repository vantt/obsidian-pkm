import type { VaultIO } from '../src/workshop/apply';

/** In-memory vault: files by path; folders are implied by file paths plus explicit empty ones. */
export class MemoryIO implements VaultIO {
  readonly files = new Map<string, string>();
  readonly dirs = new Set<string>();

  constructor(initial: Record<string, string> = {}) {
    for (const [p, t] of Object.entries(initial)) this.files.set(p, t);
  }

  private allDirs(): Set<string> {
    const out = new Set(this.dirs);
    for (const p of this.files.keys()) {
      const parts = p.split('/');
      for (let i = 1; i < parts.length; i++) out.add(parts.slice(0, i).join('/'));
    }
    return out;
  }

  async exists(path: string): Promise<boolean> {
    return this.files.has(path) || this.allDirs().has(path);
  }

  async read(path: string): Promise<string> {
    const t = this.files.get(path);
    if (t === undefined) throw new Error(`ENOENT ${path}`);
    return t;
  }

  async write(path: string, text: string): Promise<void> {
    this.files.set(path, text);
  }

  async remove(path: string): Promise<void> {
    if (!this.files.delete(path)) throw new Error(`ENOENT ${path}`);
    const dir = path.split('/').slice(0, -1).join('/');
    if (dir) this.dirs.add(dir); // the folder stays until rmdir
  }

  async list(path: string): Promise<{ files: string[]; folders: string[] } | null> {
    const dirs = this.allDirs();
    if (!dirs.has(path)) return null;
    const prefix = `${path}/`;
    const child = (p: string) => p.startsWith(prefix) && !p.slice(prefix.length).includes('/');
    return { files: [...this.files.keys()].filter(child), folders: [...dirs].filter(child) };
  }

  async rmdir(path: string): Promise<void> {
    const entry = await this.list(path);
    if (!entry || entry.files.length || entry.folders.length) throw new Error(`not empty or missing: ${path}`);
    this.dirs.delete(path);
  }
}
