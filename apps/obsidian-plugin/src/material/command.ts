import { MarkdownView, Notice } from 'obsidian';
import { messageFor } from '../bridge';
import type PkmPlugin from '../main';
import { ObsidianNoteIO } from '../note-io';
import { failNotice, openNote } from '../open-note';
import { linkTarget } from '../text';
import { runMaterial } from './flow';
import { OUT_DIR } from './page';

/**
 * The draft for the active note: the note itself, or, on a material page, the draft named in its `draft` property
 * (so the user can tick and run the command again from the page). Null when there is none.
 */
export function activeDraft(plugin: PkmPlugin): string | null {
  const { app } = plugin;
  const file = app.workspace.getActiveViewOfType(MarkdownView)?.file;
  if (!file || file.extension !== 'md') return null;
  if (!file.path.startsWith(`${OUT_DIR}/`)) return file.path;
  const target = linkTarget(app.metadataCache.getFileCache(file)?.frontmatter?.draft);
  return target ? (app.metadataCache.getFirstLinkpathDest(target, file.path)?.path ?? null) : null;
}

/** "Nguyên liệu cho bản nháp này". */
export async function materialCommand(plugin: PkmPlugin, draft: string): Promise<void> {
  const vault = plugin.vaultPath();
  if (!vault) return;
  const notice = new Notice('Đang gom nguyên liệu cho bài…', 0);
  try {
    const r = await runMaterial(new ObsidianNoteIO(plugin.app), plugin.callPkm, vault, plugin.dbArgs(), draft, () => new Date());
    notice.hide();
    const added = r.added.length ? `${r.added.length} note đã đánh dấu vào uses của bài` : '';
    if (!r.material) {
      new Notice(`${messageFor(r.error)}${added ? ` (${added}.)` : ''}`, 15000);
      return;
    }
    const fresh = r.material.items.filter((i) => !i.used);
    const text = fresh.filter((i) => i.found_by === 'ohs').length, graph = fresh.filter((i) => i.found_by === 'graph').length;
    new Notice(`${r.page}: ${text} tìm theo chữ, ${graph} graph đưa tới${added ? `, ${added}` : ''}.`, 8000);
    await openNote(plugin.app, r.page, true);
  } catch (e) {
    notice.hide();
    failNotice('Không gom được nguyên liệu', e);
  }
}
