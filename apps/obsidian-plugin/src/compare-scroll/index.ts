// Side-by-side reading ("Đọc đối chiếu"): two notes in a vertical split; the user scrolls each to the passage to
// compare, then locks; from then on both panes scroll together at that offset (session.ts). The lock button sits at
// the top right of both panes' headers and shows only while exactly two Markdown panes are side by side. Hold Alt to
// scroll one pane alone. The lock drops by itself when a note, the view mode or the layout changes.
import { FuzzySuggestModal, MarkdownView, Notice, type Plugin, setIcon, type TFile, type WorkspaceLeaf } from 'obsidian';
import { ScrollLockSession, type ScrollPane } from './session';

const ACTION_CLASS = 'pkm-compare-lock';
const T = {
  openCmd: 'Đọc đối chiếu với…',
  lockCmd: 'Khoá hoặc mở khoá cuộn hai bên',
  pick: 'Chọn note để đặt bên phải',
  opened: 'Cuộn mỗi bên tới chỗ cần so, rồi bấm ổ khoá ở góc trên bên phải.',
  noPair: 'Cần đúng hai note Markdown đặt cạnh nhau (trái, phải). Dùng lệnh "Đọc đối chiếu với…" để mở cặp.',
  noNote: 'Mở một note trước, rồi chạy lệnh này.',
  locked: 'Đã khoá: hai bên cuộn cùng nhau. Giữ Alt khi cuộn để chỉnh một bên.',
  unlocked: 'Đã mở khoá: cuộn từng bên.',
  changed: 'Đã mở khoá: note, chế độ xem hoặc bố cục vừa đổi.',
  statusLocked: 'Cuộn chung: khoá',
  statusAlt: 'Cuộn chung: tạm mở (Alt)',
  titleLocked: 'Đang khoá cuộn hai bên. Bấm để mở khoá. Giữ Alt khi cuộn để chỉnh một bên.',
  titleUnlocked: 'Cuộn mỗi bên tới chỗ cần so, rồi bấm để khoá cuộn chung.',
};

interface Bound {
  leaf: WorkspaceLeaf;
  view: MarkdownView;
  path: string;
  scroller: HTMLElement;
}

export class CompareScroll {
  private session: ScrollLockSession | null = null;
  private bound: Bound[] | null = null;
  private readonly actions = new Map<MarkdownView, HTMLElement>();
  private status: HTMLElement | null = null;

  constructor(
    private readonly plugin: Plugin,
    private readonly enabled: () => boolean,
  ) {}

  load(): void {
    const { plugin } = this;
    plugin.addCommand({
      id: 'compare-open',
      name: T.openCmd,
      checkCallback: (checking) => {
        if (!this.enabled()) return false;
        if (!checking) this.openPicker();
        return true;
      },
    });
    plugin.addCommand({
      id: 'compare-lock',
      name: T.lockCmd,
      checkCallback: (checking) => {
        if (!this.enabled()) return false;
        if (!checking) this.toggle();
        return true;
      },
    });
    this.status = plugin.addStatusBarItem();
    this.status.addClass('pkm-compare-status');
    plugin.registerDomEvent(this.status, 'click', () => this.toggle());
    const ws = plugin.app.workspace;
    for (const name of ['layout-change', 'file-open', 'active-leaf-change'] as const) {
      plugin.registerEvent(ws.on(name as 'layout-change', () => this.refresh()));
    }
    ws.onLayoutReady(() => this.refresh());
  }

  unload(): void {
    this.stop();
    for (const el of this.actions.values()) el.remove();
    this.actions.clear();
  }

  /** Called when the setting changes. */
  refresh(): void {
    if (this.bound) {
      const pair = this.findPair();
      const same =
        pair !== null &&
        this.bound.every(
          (b) => pair.includes(b.leaf) && b.leaf.view === b.view && b.view.file?.path === b.path && scrollerOf(b.view) === b.scroller,
        );
      if (!same || !this.enabled()) this.stop(this.enabled() ? T.changed : undefined);
    }
    this.syncActions();
    this.render();
  }

  private openPicker(): void {
    const ws = this.plugin.app.workspace;
    const current = ws.getActiveViewOfType(MarkdownView);
    if (!current?.file) {
      new Notice(T.noNote);
      return;
    }
    new NotePicker(this.plugin, current.file, async (file) => {
      const leaf = ws.getLeaf('split', 'vertical');
      await leaf.openFile(file);
      ws.setActiveLeaf(current.leaf, { focus: true });
      new Notice(T.opened, 5000);
    }).open();
  }

  private toggle(): void {
    if (this.session) {
      this.stop(T.unlocked);
      return;
    }
    const pair = this.findPair();
    if (!pair) {
      new Notice(T.noPair);
      return;
    }
    unlinkTabs(pair);
    this.bound = pair.map((leaf) => {
      const view = leaf.view as MarkdownView;
      return { leaf, view, path: view.file?.path ?? '', scroller: scrollerOf(view)! };
    });
    const panes = this.bound.map((b) => b.scroller as unknown as ScrollPane) as [ScrollPane, ScrollPane];
    this.session = new ScrollLockSession(
      panes,
      () => this.render(),
      () => this.stop(T.changed),
    );
    this.render();
    new Notice(T.locked, 3500);
  }

  private stop(message?: string): void {
    const was = this.session !== null;
    this.session?.dispose();
    this.session = null;
    this.bound = null;
    if (was && message) new Notice(message, 3000);
    this.render();
  }

  /** Exactly two visible Markdown panes in the main area, left and right, overlapping by at least half their height. */
  private findPair(): [WorkspaceLeaf, WorkspaceLeaf] | null {
    const ws = this.plugin.app.workspace;
    const leaves = ws.getLeavesOfType('markdown').filter((leaf) => {
      if (leaf.getRoot() !== ws.rootSplit) return false;
      const el = leaf.view.containerEl;
      if (!el.isConnected) return false;
      const r = el.getBoundingClientRect();
      return r.width > 0 && r.height > 0;
    });
    if (leaves.length !== 2) return null;
    const box = (l: WorkspaceLeaf) => l.view.containerEl.getBoundingClientRect();
    leaves.sort((a, b) => box(a).left - box(b).left);
    const [a, b] = leaves as [WorkspaceLeaf, WorkspaceLeaf];
    const ra = box(a);
    const rb = box(b);
    const overlap = Math.min(ra.bottom, rb.bottom) - Math.max(ra.top, rb.top);
    if (rb.left < ra.right - 8 || overlap < Math.min(ra.height, rb.height) / 2) return null;
    if (![a, b].every((l) => (l.view as MarkdownView).file && scrollerOf(l.view as MarkdownView))) return null;
    return [a, b];
  }

  /** A lock button in every Markdown pane header; render() shows it only when a pair exists. */
  private syncActions(): void {
    const views = new Set(
      this.plugin.app.workspace
        .getLeavesOfType('markdown')
        .map((l) => l.view)
        .filter((v): v is MarkdownView => v instanceof MarkdownView),
    );
    for (const [view, el] of this.actions) {
      if (!views.has(view) || !el.isConnected || !this.enabled()) {
        el.remove();
        this.actions.delete(view);
      }
    }
    if (!this.enabled()) return;
    for (const view of views) {
      if (this.actions.has(view)) continue;
      const el = view.addAction('unlock', T.titleUnlocked, () => this.toggle());
      el.addClass(ACTION_CLASS);
      this.actions.set(view, el);
    }
  }

  private render(): void {
    const locked = this.session !== null;
    const alt = locked && this.session!.alt;
    const pair = this.enabled() && (locked || this.findPair() !== null);
    const title = locked ? T.titleLocked : T.titleUnlocked;
    for (const el of this.actions.values()) {
      el.toggleClass('is-hidden', !pair);
      el.toggleClass('is-active', locked);
      setIcon(el, locked && !alt ? 'lock' : 'unlock');
      el.setAttribute('aria-label', title);
      el.setAttribute('aria-pressed', String(locked));
    }
    if (this.status) {
      this.status.toggleClass('is-hidden', !locked);
      this.status.setText(alt ? T.statusAlt : T.statusLocked);
      this.status.setAttribute('aria-label', title);
    }
  }
}

function scrollerOf(view: MarkdownView): HTMLElement | null {
  const preview = view.getMode() === 'preview';
  return view.contentEl.querySelector<HTMLElement>(preview ? '.markdown-preview-view' : '.cm-scroller');
}

/** Obsidian's "Link with tab" makes both panes follow links together; a locked pair should not. */
function unlinkTabs(pair: WorkspaceLeaf[]): void {
  const [a, b] = pair as [WorkspaceLeaf & { group?: string | null }, WorkspaceLeaf & { group?: string | null }];
  if (a.group && a.group === b.group) {
    for (const l of [a, b]) l.setGroup(null as unknown as string);
  }
}

class NotePicker extends FuzzySuggestModal<TFile> {
  constructor(
    plugin: Plugin,
    private readonly current: TFile,
    private readonly chosen: (file: TFile) => void,
  ) {
    super(plugin.app);
    this.setPlaceholder(T.pick);
  }
  getItems(): TFile[] {
    return this.app.vault.getMarkdownFiles().filter((f) => f.path !== this.current.path);
  }
  getItemText(file: TFile): string {
    return file.path.replace(/\.md$/, '');
  }
  onChooseItem(file: TFile): void {
    this.chosen(file);
  }
}
