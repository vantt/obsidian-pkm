// VaultIO over Obsidian. Notes and bases go through the Vault API (`vault.process` on an existing file, so a note
// open in an editor is updated in place rather than overwritten underneath it; `fileManager.trashFile` to delete,
// honouring the user's trash setting). Hidden paths and the manifest go through the adapter, which Obsidian does
// not index anyway.
import { type App, normalizePath, TFile, TFolder } from 'obsidian';
import type { VaultIO } from './apply';

const viaVault = (path: string) => /\.(md|base)$/.test(path) && !path.split('/').some((s) => s.startsWith('.'));

export class ObsidianVaultIO implements VaultIO {
  constructor(private readonly app: App) {}

  async exists(path: string): Promise<boolean> {
    return this.app.vault.adapter.exists(normalizePath(path));
  }

  async read(path: string): Promise<string> {
    const f = this.app.vault.getAbstractFileByPath(normalizePath(path));
    return f instanceof TFile ? this.app.vault.read(f) : this.app.vault.adapter.read(normalizePath(path));
  }

  async write(path: string, text: string): Promise<void> {
    const p = normalizePath(path);
    const f = this.app.vault.getAbstractFileByPath(p);
    if (f instanceof TFile) {
      await this.app.vault.process(f, () => text);
      return;
    }
    await this.ensureFolder(p.split('/').slice(0, -1).join('/'));
    if (viaVault(p)) await this.app.vault.create(p, text);
    else await this.app.vault.adapter.write(p, text);
  }

  async remove(path: string): Promise<void> {
    const p = normalizePath(path);
    const f = this.app.vault.getAbstractFileByPath(p);
    if (f instanceof TFile) await this.app.fileManager.trashFile(f);
    else if (await this.app.vault.adapter.exists(p)) await this.app.vault.adapter.remove(p);
  }

  async list(path: string): Promise<{ files: string[]; folders: string[] } | null> {
    const p = normalizePath(path);
    if (!(await this.app.vault.adapter.exists(p))) return null;
    const stat = await this.app.vault.adapter.stat(p);
    if (stat?.type !== 'folder') return null;
    return this.app.vault.adapter.list(p);
  }

  async rmdir(path: string): Promise<void> {
    const p = normalizePath(path);
    const f = this.app.vault.getAbstractFileByPath(p);
    if (f instanceof TFolder) await this.app.vault.delete(f, false);
    else await this.app.vault.adapter.rmdir(p, false);
  }

  private async ensureFolder(dir: string): Promise<void> {
    if (!dir) return;
    const parts = dir.split('/');
    for (let i = 1; i <= parts.length; i++) {
      const sub = parts.slice(0, i).join('/');
      if (await this.app.vault.adapter.exists(sub)) continue;
      if (parts.slice(0, i).some((s) => s.startsWith('.'))) await this.app.vault.adapter.mkdir(sub);
      else await this.app.vault.createFolder(sub);
    }
  }
}
