import { type App, normalizePath, Notice, TFile } from 'obsidian';

/**
 * Opens a vault note: in the pane that already shows it, else in a new split (`split`) or the current pane. A file
 * pkm just wrote may not be indexed yet; then it says so instead of creating an empty note.
 */
export async function openNote(app: App, path: string, split = false): Promise<void> {
  const file = app.vault.getAbstractFileByPath(normalizePath(path));
  if (!(file instanceof TFile)) {
    new Notice(`Chưa mở được ${path}: Obsidian chưa thấy file này. Thử lại sau vài giây.`);
    return;
  }
  const open = app.workspace.getLeavesOfType('markdown').find((l) => (l.view as { file?: TFile | null }).file?.path === file.path);
  if (open) {
    app.workspace.setActiveLeaf(open, { focus: true });
    return;
  }
  await app.workspace.getLeaf(split ? 'split' : false).openFile(file);
}

/** Logs and shows an error from a command. */
export function failNotice(what: string, e: unknown): void {
  console.error(`pkm: ${what}`, e);
  new Notice(`${what}: ${e instanceof Error ? e.message : String(e)}`, 15000);
}
