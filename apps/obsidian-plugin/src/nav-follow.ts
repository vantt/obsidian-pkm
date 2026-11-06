// The open note's row in the file explorer sits wherever Obsidian last scrolled it, so the user has to scan the tree
// to find it. When a note opens and the open did NOT start in the file explorer (a link in a note, the quick
// switcher, a tab, back/forward), this opens the note's folders and scrolls its row to the middle of the tree. A
// click or key in the explorer itself does nothing: the row is already where the user looks. Focus stays where it
// was; Obsidian's "Reveal current file" command moves focus to the explorer, so it is not used.
// Uses the explorer's internal fileItems / tree.infinityScroll (not public API, present in 1.15); when they are
// missing it does nothing. The look of the row is e11-theme index 10 (presets/b/theme). Same behaviour as
// scripts/e11/nav-follow-plugin in obsidian-pkm-automation (vault B before it runs this plugin).
import type { Plugin, TAbstractFile, TFile } from 'obsidian';

/** An open this soon after a pointer or key event in the explorer counts as an open from the tree. */
export const FROM_TREE_MS = 1500;
const EXPLORER = '.workspace-leaf-content[data-type="file-explorer"]';

interface TreeItem {
  collapsed?: boolean;
  selfEl?: HTMLElement;
  setCollapsed?: (collapsed: boolean, animate: boolean) => Promise<void> | void;
}
interface ExplorerView {
  containerEl?: HTMLElement & { isShown?: () => boolean };
  fileItems?: Record<string, TreeItem>;
  tree?: { infinityScroll?: { scrollEl?: HTMLElement; scrollIntoView?: (item: TreeItem, margin: number) => void } };
}

export function openedFromTree(now: number, treeAt: number): boolean {
  return now - treeAt < FROM_TREE_MS;
}

/** Opens every collapsed folder above `file` (not the vault root), top-most last as Obsidian walks it. */
export async function expandParents(file: TAbstractFile, items: Record<string, TreeItem>): Promise<void> {
  for (let p = file.parent; p && !p.isRoot(); p = p.parent) {
    const folder = items[p.path];
    if (folder?.collapsed) await folder.setCollapsed?.(false, false);
  }
}

export function startNavFollow(plugin: Plugin, enabled: () => boolean): void {
  let treeAt = 0;
  const mark = (e: Event) => {
    if (e.target instanceof Element && e.target.closest(EXPLORER)) treeAt = Date.now();
  };
  plugin.registerDomEvent(document, 'pointerdown', mark, true);
  plugin.registerDomEvent(document, 'keydown', mark, true);
  plugin.registerEvent(
    plugin.app.workspace.on('file-open', (file) => {
      if (file && enabled() && !openedFromTree(Date.now(), treeAt)) void follow(plugin, file);
    }),
  );
}

async function follow(plugin: Plugin, file: TFile): Promise<void> {
  const view = plugin.app.workspace.getLeavesOfType('file-explorer')[0]?.view as unknown as ExplorerView | undefined;
  // sidebar closed, or another tab in front of the explorer: leave the tree alone
  if (!view?.containerEl?.isShown?.() || !view.fileItems) return;
  const item = view.fileItems[file.path];
  const scroll = view.tree?.infinityScroll;
  if (!item || !scroll) return;
  await expandParents(file, view.fileItems);
  // one frame for the folders to open, one for the virtual list to render the row, then centre it
  requestAnimationFrame(() => {
    scroll.scrollIntoView?.(item, 4);
    requestAnimationFrame(() => {
      const el = item.selfEl;
      const box = scroll.scrollEl ?? view.containerEl?.querySelector<HTMLElement>('.nav-files-container');
      if (!el?.isConnected || !box) return;
      const r = el.getBoundingClientRect();
      const b = box.getBoundingClientRect();
      box.scrollTop += r.top + r.height / 2 - (b.top + b.height / 2);
    });
  });
}
