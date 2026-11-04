// The "Dựng xưởng" and "Gỡ xưởng" commands: show what will happen, ask, then write or remove.
import { type App, Modal, Notice, parseYaml, Setting } from 'obsidian';
import type { PkmSettings } from '../settings';
import { type Analysis, analyse, applyPlan, removeWorkshop } from './apply';
import { buildPlan, findTopicFolders, NOTE_TEMPLATES, topicPagePath } from './plan';
import { ObsidianVaultIO } from './obsidian-io';

const SHOWN = 10;

function listEl(parent: HTMLElement, title: string, paths: string[]): void {
  if (!paths.length) return;
  parent.createEl('p', { text: `${title} (${paths.length}):` });
  const ul = parent.createEl('ul', { cls: 'pkm-workshop-list' });
  for (const p of paths.slice(0, SHOWN)) ul.createEl('li', { text: p });
  if (paths.length > SHOWN) ul.createEl('li', { text: `… và ${paths.length - SHOWN} file nữa` });
}

/** Title, body, Cancel and one action button. */
export class ConfirmModal extends Modal {
  constructor(
    app: App,
    private readonly title: string,
    private readonly body: (el: HTMLElement) => void,
    private readonly action: string,
    private readonly onConfirm: () => void,
  ) {
    super(app);
  }

  override onOpen(): void {
    this.titleEl.setText(this.title);
    this.body(this.contentEl);
    new Setting(this.contentEl)
      .addButton((b) => b.setButtonText('Huỷ').onClick(() => this.close()))
      .addButton((b) =>
        b
          .setButtonText(this.action)
          .setCta()
          .onClick(() => {
            this.close();
            this.onConfirm();
          }),
      );
  }

  override onClose(): void {
    this.contentEl.empty();
  }
}

const fail = (what: string, e: unknown) => {
  console.error(`pkm: ${what}`, e);
  new Notice(`${what}: ${e instanceof Error ? e.message : String(e)}`, 10000);
};

export async function buildWorkshop(app: App, settings: PkmSettings): Promise<void> {
  const io = new ObsidianVaultIO(app);
  let files: Map<string, string>, analysis: Analysis, topics: string[];
  try {
    topics = await findTopicFolders(io, settings.topicRoots);
    files = buildPlan({ vaultName: app.vault.getName(), topicFolders: topics, inboxFolders: settings.inboxFolders });
    analysis = await analyse(io, files, parseYaml);
  } catch (e) {
    return fail('Không chuẩn bị được xưởng', e);
  }
  if (analysis.clash.length) {
    new Notice(
      `Không dựng xưởng: ${analysis.clash.length} file đã có sẵn mà không do xưởng tạo (${analysis.clash.slice(0, 3).join(', ')}${analysis.clash.length > 3 ? ', …' : ''}). Đổi tên hoặc chuyển chúng đi rồi chạy lại.`,
      15000,
    );
    return;
  }
  const fresh = [...files.keys()].filter((p) => !analysis.edited.includes(p));
  new ConfirmModal(
    app,
    'Dựng xưởng',
    (el) => {
      el.createEl('p', {
        text: `Ghi ${fresh.length} file: trang chủ, 2 file Bases, ${Object.keys(NOTE_TEMPLATES).length} mẫu, ${topics.length} trang chủ đề. Note của bạn không bị sửa.`,
      });
      if (!settings.topicRoots.length) el.createEl('p', { text: 'Chưa chọn thư mục chủ đề trong Cài đặt > PKM nên không có trang chủ đề.' });
      listEl(el, 'Trang chủ đề', topics.map(topicPagePath));
      listEl(el, 'Giữ nguyên vì bạn đã sửa', analysis.edited);
      listEl(el, 'Không còn dùng, xoá nếu bạn chưa sửa', analysis.stale);
    },
    'Dựng',
    () => {
      applyPlan(io, files, analysis, parseYaml)
        .then((r) => {
          const kept = r.kept.length ? `, giữ nguyên ${r.kept.length} file bạn đã sửa` : '';
          const removed = r.removed.length ? `, xoá ${r.removed.length} file cũ` : '';
          new Notice(`Đã dựng xưởng: ghi ${r.written.length} file${kept}${removed}. Tải lại Obsidian (Ctrl+R) để Homepage và Templater nhận cấu hình mới.`, 10000);
        })
        .catch((e: unknown) => fail('Dựng xưởng dở dang', e));
    },
  ).open();
}

export function removeWorkshopCommand(app: App): void {
  const io = new ObsidianVaultIO(app);
  new ConfirmModal(
    app,
    'Gỡ xưởng',
    (el) => {
      el.createEl('p', { text: 'Xoá các file xưởng đã ghi (file bạn đã sửa được giữ lại) và trả cấu hình Homepage, Templater về như trước.' });
    },
    'Gỡ',
    () => {
      removeWorkshop(io, parseYaml)
        .then((r) => {
          const kept = r.kept.length ? `; giữ ${r.kept.length} file bạn đã sửa: ${r.kept.slice(0, 3).join(', ')}` : '';
          new Notice(`Đã gỡ xưởng: xoá ${r.removed.length} file${kept}.`, 10000);
        })
        .catch((e: unknown) => fail('Không gỡ được xưởng', e));
    },
  ).open();
}
