// Commands and the review modal for the proposal queue. The modal lists waiting proposals; Nhận and Bỏ only mark the
// note's `status` (the review log records it), "Áp dụng" then edits the notes. Opening a proposal opens its note,
// which embeds the sources, so the user decides after reading them.
import { type App, Modal, Notice, Setting } from 'obsidian';
import { messageFor } from '../bridge';
import type PkmPlugin from '../main';
import { ObsidianNoteIO } from '../note-io';
import { failNotice, openNote } from '../open-note';
import { title } from '../text';
import { ConfirmModal } from '../workshop/command';
import { applyDecisions, decide, KIND_TITLES, listQueue, LOG, type QueuedProposal, revertAll } from './apply';
import { refreshProposals } from './refresh';
import { reviewStats, STATS } from './stats';

const kindTitle = (k: string) => KIND_TITLES[k] ?? k;

function countText(counts: Record<string, number>): string {
  let accepted = 0, rejected = 0;
  for (const [k, n] of Object.entries(counts)) {
    if (k.endsWith('|nhận')) accepted += n;
    else rejected += n;
  }
  return `áp dụng ${accepted}, đóng ${rejected} đề xuất bỏ`;
}

export async function applyCommand(app: App): Promise<void> {
  try {
    const r = await applyDecisions(new ObsidianNoteIO(app), new Date());
    const done = Object.keys(r.counts).length ? `Đã ${countText(r.counts)}; sổ ở ${LOG}.` : 'Chưa có đề xuất nào đánh dấu nhận hay bỏ.';
    const failed = r.failed.length
      ? ` Không áp dụng được ${r.failed.length} đề xuất (giữ nguyên, sửa rồi chạy lại): ${r.failed.slice(0, 3).map((f) => `${title(f.path)}: ${f.message}`).join('; ')}`
      : '';
    new Notice(done + failed, failed ? 20000 : 8000);
  } catch (e) {
    failNotice('Áp dụng đề xuất dở dang', e);
  }
}

export function revertCommand(app: App): void {
  new ConfirmModal(
    app,
    'Hoàn tác đề xuất đã áp dụng',
    (el) => {
      el.createEl('p', {
        text: `Trả mọi note mà các đề xuất đã áp dụng về như trước (theo ${LOG}, mới nhất trước), xoá note chúng tạo ra, mở lại các đề xuất. Sổ được đổi tên thành .reverted-… để giữ lại.`,
      });
      el.createEl('p', { text: 'Note bạn đã sửa thêm sau khi áp dụng cũng bị trả về bản cũ.' });
    },
    'Hoàn tác',
    () => {
      revertAll(new ObsidianNoteIO(app), new Date())
        .then((n) => new Notice(`Đã hoàn tác ${n} quyết định.`, 8000))
        .catch((e: unknown) => failNotice('Không hoàn tác được', e));
    },
  ).open();
}

export async function statsCommand(app: App): Promise<void> {
  try {
    const r = await reviewStats(new ObsidianNoteIO(app), new Date());
    new Notice(r.rows ? `Đã ghi ${STATS}.` : `Chưa có đề xuất nào đã quyết; vẫn ghi ${STATS}.`);
    await openNote(app, STATS);
  } catch (e) {
    failNotice('Không đo được', e);
  }
}

export async function refreshCommand(plugin: PkmPlugin): Promise<boolean> {
  const vault = plugin.vaultPath();
  if (!vault) return false;
  const notice = new Notice('Đang tìm đề xuất mới (cặp gần nghĩa qua OHS, rồi các luật)…', 0);
  try {
    const { result, warning } = await refreshProposals(plugin.callPkm, vault, plugin.dbArgs());
    notice.hide();
    if (warning) new Notice(warning, 15000);
    const kinds = Object.entries(result.byKind).map(([k, n]) => `${kindTitle(k)} ${n}`).join(', ');
    new Notice(`Thêm ${result.written.length} đề xuất vào _Đề xuất/${kinds ? ` (${kinds})` : ''}.`, 8000);
    return true;
  } catch (e) {
    notice.hide();
    new Notice(messageFor(e), 15000);
    return false;
  }
}

export class ProposalsModal extends Modal {
  constructor(private readonly plugin: PkmPlugin) {
    super(plugin.app);
  }

  override onOpen(): void {
    void this.render();
  }

  override onClose(): void {
    this.contentEl.empty();
  }

  private io() {
    return new ObsidianNoteIO(this.app);
  }

  private async render(): Promise<void> {
    let queue: QueuedProposal[];
    try {
      queue = await listQueue(this.io());
    } catch (e) {
      this.close();
      return failNotice('Không đọc được _Đề xuất/', e);
    }
    const waiting = queue.filter((p) => p.status === 'chờ');
    const marked = queue.filter((p) => p.status === 'nhận' || p.status === 'bỏ');
    const el = this.contentEl;
    el.empty();
    el.addClass('pkm-proposals');
    this.titleEl.setText(`Đề xuất chờ duyệt (${waiting.length})`);

    new Setting(el)
      .setDesc('Mở đề xuất để đọc các note nguồn, rồi chọn Nhận hoặc Bỏ. Nhận và Bỏ chỉ ghi vào đề xuất; Áp dụng mới sửa note.')
      .addButton((b) =>
        b.setButtonText('Làm mới').onClick(async () => {
          b.setDisabled(true);
          if (await refreshCommand(this.plugin)) await this.render();
          else b.setDisabled(false);
        }),
      )
      .addButton((b) =>
        b
          .setButtonText(`Áp dụng (${marked.length})`)
          .setCta()
          .setDisabled(!marked.length)
          .onClick(async () => {
            b.setDisabled(true);
            await applyCommand(this.app);
            await this.render();
          }),
      );

    if (marked.length) {
      el.createEl('h4', { text: `Đã chọn, chưa áp dụng (${marked.length})` });
      for (const p of marked) this.row(el, p);
    }
    el.createEl('h4', { text: `Chờ (${waiting.length})` });
    if (!waiting.length) el.createEl('p', { text: 'Không còn đề xuất nào chờ. Bấm Làm mới để máy tìm thêm.' });
    for (const p of waiting) this.row(el, p);
  }

  private row(parent: HTMLElement, p: QueuedProposal): void {
    const s = new Setting(parent).setName(`${kindTitle(p.kind)}: ${title(p.path)}`).setDesc(p.status === 'chờ' ? p.reason : `Đã chọn: ${p.status}${p.rejectReason ? ` (${p.rejectReason})` : ''}`);
    s.nameEl.addClass('pkm-proposal-name');
    s.nameEl.onclick = () => {
      this.close();
      void openNote(this.app, p.path);
    };
    const act = (decision: 'nhận' | 'bỏ' | 'chờ', reason = '') =>
      decide(this.io(), p.path, decision, reason)
        .then(() => this.render())
        .catch((e: unknown) => failNotice('Không ghi được quyết định', e));
    if (p.status !== 'chờ') {
      s.addButton((b) => b.setButtonText('Chọn lại').onClick(() => void act('chờ')));
      return;
    }
    s.addButton((b) => b.setButtonText('Nhận').onClick(() => void act('nhận')));
    s.addButton((b) =>
      b.setButtonText('Bỏ…').onClick(() => {
        b.buttonEl.remove();
        let reason = '';
        s.addText((t) => t.setPlaceholder('Lý do bỏ (không bắt buộc)').onChange((v) => (reason = v)));
        s.addButton((ok) => ok.setButtonText('Bỏ').setWarning().onClick(() => void act('bỏ', reason)));
      }),
    );
  }
}
