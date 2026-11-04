// A button at the top right of every table in a note (Markdown, Bases, Dataview) that widens it to the full width
// of the pane, past the readable line width, and back (button again, or Esc). Reading view and Live Preview, light
// and dark. Nothing is written to notes; the report command writes a layout report under _E11/.
import { MarkdownView, Notice, type Plugin, setIcon } from 'obsidian';

type Kind = 'base' | 'dataview' | 'table';

/** What counts as a table. `holder` is where the button goes inside the box (default: the box itself). */
const KINDS: { kind: Kind; sel: string; has?: string; holder?: string }[] = [
  { kind: 'base', sel: '.bases-embed, .block-language-base' },
  { kind: 'dataview', sel: '.block-language-dataview', has: 'table' },
  { kind: 'table', sel: '.markdown-rendered .el-table, .cm-table-widget', holder: '.table-wrapper' },
];
const SCOPE = '.workspace-leaf-content[data-type="markdown"]';
const PANE = '.markdown-preview-view, .cm-scroller';
const REPORT = '_E11/table-zoom-report.json';
const ZOOMED = 'data-pkm-zoomed';
// Bases lays its cells out itself, each column at most --bases-table-column-max-width (Obsidian default 300px), so a
// wider box alone shows the same columns plus empty space. Zoomed, a column may take this share of the pane.
const BASES_COLUMN_SHARE = 0.5;

interface Box {
  box: HTMLElement;
  kind: Kind;
  holder: HTMLElement;
}

export class TableZoom {
  /** file path + table signature, so a zoom survives re-render and a mode switch */
  private readonly zoomed = new Set<string>();
  private readonly resize = new ResizeObserver(() => this.refit());
  private readonly observer = new MutationObserver(() => this.schedule());
  private pending: number | null = null;

  constructor(private readonly plugin: Plugin) {}

  load(): void {
    const { app } = this.plugin;
    app.workspace.onLayoutReady(() => {
      this.observer.observe(app.workspace.containerEl, { childList: true, subtree: true });
      this.schedule();
    });
    this.plugin.registerEvent(app.workspace.on('layout-change', () => this.schedule()));
    this.plugin.registerEvent(app.workspace.on('resize', () => this.refit()));
    for (const type of ['pointerdown', 'mousedown', 'pointerup', 'mouseup', 'click'] as const) {
      this.plugin.registerDomEvent(window, type, (e) => this.press(e), { capture: true });
    }
    // Esc folds the zoomed tables of the active note back; the key still reaches Obsidian.
    this.plugin.registerDomEvent(document, 'keydown', (e) => {
      if (e.key !== 'Escape' || document.querySelector('.modal-container')) return;
      const el = app.workspace.getMostRecentLeaf()?.view.containerEl;
      for (const box of Array.from(el?.querySelectorAll<HTMLElement>(`[${ZOOMED}]`) ?? [])) {
        this.zoomed.delete(this.key(box, kindOf(box)));
        this.unzoom(box);
      }
    });
    this.plugin.addCommand({ id: 'table-zoom-toggle-all', name: 'Phóng hoặc thu mọi bảng trong note đang mở', callback: () => this.toggleAll() });
    this.plugin.addCommand({ id: 'table-zoom-report', name: 'Ghi báo cáo nút phóng bảng', callback: () => void this.report() });
  }

  unload(): void {
    this.observer.disconnect();
    this.resize.disconnect();
    if (this.pending !== null) cancelAnimationFrame(this.pending);
    for (const btn of Array.from(document.querySelectorAll('.pkm-zoom'))) btn.remove();
    for (const box of Array.from(document.querySelectorAll<HTMLElement>('.pkm-zoom-box'))) {
      this.unzoom(box);
      box.removeClass('pkm-zoom-box');
    }
  }

  private schedule(): void {
    if (this.pending !== null) return;
    this.pending = requestAnimationFrame(() => {
      this.pending = null;
      this.scan();
    });
  }

  private boxes(root: HTMLElement = this.plugin.app.workspace.containerEl): Box[] {
    const out: Box[] = [];
    for (const k of KINDS) {
      for (const box of Array.from(root.querySelectorAll<HTMLElement>(k.sel))) {
        if (!box.closest(SCOPE) || box.closest('.hover-popover')) continue;
        if (k.has && !box.querySelector(k.has)) continue;
        if (k.kind !== 'base' && box.parentElement?.closest('.bases-embed, .block-language-base')) continue;
        const holder = (k.holder && box.querySelector<HTMLElement>(k.holder)) || box;
        out.push({ box, kind: k.kind, holder });
      }
    }
    return out;
  }

  private scan(): void {
    for (const { box, kind, holder } of this.boxes()) {
      box.addClass('pkm-zoom-box');
      box.dataset.pkmKind = kind;
      if (!box.querySelector(':scope .pkm-zoom')) this.button(box, kind, holder);
      const on = this.zoomed.has(this.key(box, kind));
      if (on !== box.hasAttribute(ZOOMED)) {
        if (on) this.zoom(box);
        else this.unzoom(box);
      }
    }
    this.refit();
  }

  private button(box: HTMLElement, kind: Kind, holder: HTMLElement): void {
    const bar = kind === 'base' ? box.querySelector('.bases-toolbar-items, .bases-toolbar') : null;
    const btn = createEl('button', { cls: 'pkm-zoom clickable-icon' });
    this.label(btn, box.hasAttribute(ZOOMED));
    (bar ?? holder).appendChild(btn);
  }

  // Clicks are caught on the window in the capture phase: Bases and the embed wrapper handle pointer and mouse
  // events on the way down and the button never sees the click. Swallow the press, act on the click.
  private press(e: Event): void {
    const btn = e.target instanceof Element ? e.target.closest('.pkm-zoom') : null;
    if (!btn) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    if (e.type !== 'click') return;
    const box = btn.closest<HTMLElement>('.pkm-zoom-box');
    if (!box) return;
    const key = this.key(box, kindOf(box));
    if (box.hasAttribute(ZOOMED)) {
      this.zoomed.delete(key);
      this.unzoom(box);
    } else {
      this.zoomed.add(key);
      this.zoom(box);
    }
  }

  private label(btn: HTMLElement, on: boolean): void {
    setIcon(btn, on ? 'minimize-2' : 'maximize-2');
    btn.setAttribute('aria-label', on ? 'Thu bảng về khổ đọc' : 'Phóng bảng hết chiều ngang');
  }

  private zoom(box: HTMLElement): void {
    box.setAttribute(ZOOMED, '');
    this.fit(box);
    const btn = box.querySelector<HTMLElement>('.pkm-zoom');
    if (btn) this.label(btn, true);
  }

  private unzoom(box: HTMLElement): void {
    box.removeAttribute(ZOOMED);
    box.style.removeProperty('--pkm-zoom-width');
    box.style.removeProperty('--pkm-zoom-x');
    box.style.removeProperty('--bases-table-column-max-width');
    delete box.dataset.pkmFit;
    this.relayout(box);
    const btn = box.querySelector<HTMLElement>('.pkm-zoom');
    if (btn) this.label(btn, false);
  }

  /** Width of the pane's text area (inside its margins) and how far left of the box it starts. */
  private fit(box: HTMLElement): void {
    const pane = box.closest<HTMLElement>(PANE), parent = box.parentElement;
    if (!pane || !parent) return;
    const ps = getComputedStyle(pane), pr = pane.getBoundingClientRect();
    const left = pr.left + pane.clientLeft + parseFloat(ps.paddingLeft);
    const width = pane.clientWidth - parseFloat(ps.paddingLeft) - parseFloat(ps.paddingRight);
    const qs = getComputedStyle(parent);
    const natural = parent.getBoundingClientRect().left + parent.clientLeft + parseFloat(qs.paddingLeft);
    box.style.setProperty('--pkm-zoom-width', `${Math.max(width, 0)}px`);
    box.style.setProperty('--pkm-zoom-x', `${left - natural}px`);
    const was = box.dataset.pkmFit;
    box.dataset.pkmFit = `${Math.round(width)} ${Math.round(left - natural)}`;
    // only on a real change: relayout fires resize, which calls refit again
    if (kindOf(box) === 'base' && box.dataset.pkmFit !== was) {
      box.style.setProperty('--bases-table-column-max-width', `${Math.max(300, Math.round(width * BASES_COLUMN_SHARE))}px`);
      this.relayout(box);
    }
    this.resize.observe(pane);
  }

  /** Bases recomputes column widths in its onResize/scroll path; nudge it after the box changed width. */
  private relayout(box: HTMLElement): void {
    if (kindOf(box) !== 'base') return;
    requestAnimationFrame(() => {
      for (const el of Array.from(box.querySelectorAll('.bases-view, .bases-table-container'))) el.dispatchEvent(new Event('scroll'));
      window.dispatchEvent(new Event('resize'));
    });
  }

  private refit(): void {
    for (const box of Array.from(document.querySelectorAll<HTMLElement>(`[${ZOOMED}]`))) this.fit(box);
  }

  /** Bases embeds by their source ("Xưởng.base#Hôm nay"), tables by their header row. */
  private key(box: HTMLElement, kind: Kind): string {
    const leaf = box.closest('.workspace-leaf-content');
    const view = this.plugin.app.workspace
      .getLeavesOfType('markdown')
      .map((l) => l.view)
      .find((v) => v.containerEl === leaf);
    const file = view instanceof MarkdownView ? view.file?.path : undefined;
    const head = box.querySelector('.bases-header, thead, tr');
    const sig =
      kind === 'base'
        ? box.getAttribute('src') ?? head?.textContent
        : Array.from(head?.querySelectorAll('th, td') ?? []).map((c) => (c.textContent ?? '').trim()).join('|');
    return `${file ?? ''}\u0000${kind}\u0000${(sig ?? '').slice(0, 300)}`;
  }

  private toggleAll(): void {
    const leaf = this.plugin.app.workspace.getMostRecentLeaf();
    const found = leaf ? this.boxes(leaf.view.containerEl) : [];
    const on = found.some(({ box }) => !box.hasAttribute(ZOOMED));
    for (const { box, kind } of found) {
      const key = this.key(box, kind);
      if (on) {
        this.zoomed.add(key);
        this.zoom(box);
      } else {
        this.zoomed.delete(key);
        this.unzoom(box);
      }
    }
  }

  /** What the plugin sees in every open note, one line per table with the widths it set, so the layout can be
   * checked without screenshots. */
  private async report(): Promise<void> {
    try {
      const { app } = this.plugin;
      const leaves = app.workspace.getLeavesOfType('markdown').map((leaf) => {
        const el = leaf.view.containerEl, pane = el.querySelector(PANE);
        const tables = this.boxes(el).map(({ box, kind }) => {
          const r = box.getBoundingClientRect(), btn = box.querySelector('.pkm-zoom');
          const p = btn?.getBoundingClientRect();
          const cells = Array.from(box.querySelectorAll('.bases-thead .bases-td')).map((c) => c.getBoundingClientRect()).filter((c) => c.width);
          const cols = cells.length
            ? { n: cells.length, right: Math.round(Math.max(...cells.map((c) => c.right)) - r.left), widths: cells.map((c) => Math.round(c.width)).join(' ') }
            : undefined;
          return {
            kind, cols, zoomed: box.hasAttribute(ZOOMED), fit: box.dataset.pkmFit, left: Math.round(r.left), width: Math.round(r.width),
            button: p ? { inToolbar: !!btn?.closest('.bases-toolbar'), right: Math.round(r.right - p.right), top: Math.round(p.top - r.top) } : null,
          };
        });
        const pr = pane?.getBoundingClientRect();
        const view = leaf.view instanceof MarkdownView ? leaf.view : null;
        return { path: view?.file?.path, mode: view?.getMode(), pane: pr && { left: Math.round(pr.left), width: Math.round(pr.width) }, tables };
      });
      const adapter = app.vault.adapter;
      if (!(await adapter.exists('_E11'))) await adapter.mkdir('_E11');
      await adapter.write(REPORT, JSON.stringify({ at: new Date().toISOString(), leaves }, null, 2));
      new Notice(`Đã ghi ${REPORT}`);
    } catch (e) {
      new Notice(`Không ghi được báo cáo phóng bảng: ${e instanceof Error ? e.message : String(e)}`);
      console.error(e);
    }
  }
}

const kindOf = (box: HTMLElement): Kind => (box.dataset.pkmKind as Kind | undefined) ?? 'table';
