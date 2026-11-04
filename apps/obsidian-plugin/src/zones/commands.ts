// "Gợi ý bài viết": runs `pkm VAULT zones` and shows the zones in a modal: summary, grade from the page, core notes
// to open, highlighted passages, and "Viết bài từ vùng này". Grading stays on the page, which pkm never rewrites.
import { type App, Modal, Notice, Setting } from 'obsidian';
import { messageFor } from '../bridge';
import type PkmPlugin from '../main';
import { ObsidianNoteIO } from '../note-io';
import { failNotice, openNote } from '../open-note';
import { cpSlice, title } from '../text';
import { loadZones, PAGE, startPost, type Zone, zoneSummary } from './zones';

export async function zonesCommand(plugin: PkmPlugin): Promise<void> {
  const vault = plugin.vaultPath();
  if (!vault) return;
  const notice = new Notice('Đang tìm vùng tri thức (vector OHS, graph)…', 0);
  try {
    const { result, grades, pageCreated } = await loadZones(new ObsidianNoteIO(plugin.app), plugin.callPkm, vault, plugin.dbArgs());
    notice.hide();
    if (pageCreated) new Notice(`Đã ghi ${PAGE}: chấm từng vùng ở đó.`, 8000);
    new ZonesModal(plugin.app, result.zones, grades, result.generated).open();
  } catch (e) {
    notice.hide();
    new Notice(messageFor(e), 15000);
  }
}

class ZonesModal extends Modal {
  constructor(
    app: App,
    private readonly zones: Zone[],
    private readonly grades: Map<string, string>,
    private readonly generated: string,
  ) {
    super(app);
  }

  override onOpen(): void {
    const el = this.contentEl;
    el.addClass('pkm-zones');
    this.titleEl.setText(`Gợi ý bài viết: ${this.zones.length} vùng`);
    new Setting(el)
      .setDesc(
        `Máy quét ${this.generated}. Vùng xếp theo điểm; tên vùng là các chữ hay gặp trong tiêu đề, chưa phải góc viết. ` +
          `Chấm (đáng viết, không, bất ngờ) ở dòng **Chấm** trên trang ${title(PAGE)}.`,
      )
      .addButton((b) =>
        b.setButtonText('Mở trang chấm').onClick(() => {
          this.close();
          void openNote(this.app, PAGE);
        }),
      );
    if (!this.zones.length) el.createEl('p', { text: 'Chưa có vùng nào: vault cần thêm note có chữ, hoặc OHS chưa lập xong chỉ mục.' });
    this.zones.forEach((z, i) => this.zone(el, z, i + 1));
  }

  override onClose(): void {
    this.contentEl.empty();
  }

  private zone(parent: HTMLElement, z: Zone, n: number): void {
    const box = parent.createDiv({ cls: 'pkm-zone' });
    const grade = this.grades.get(z.label);
    new Setting(box)
      .setName(`${n}. ${z.label}`)
      .setDesc(`${zoneSummary(z)}${grade ? ` Đã chấm: ${grade}.` : ''}`)
      .addButton((b) =>
        b.setButtonText('Viết bài từ vùng này').onClick(() => {
          startPost(new ObsidianNoteIO(this.app), z)
            .then(({ path, created }) => {
              if (!created) new Notice(`${path} đã có; mở bài đó, không ghi đè.`);
              this.close();
              return openNote(this.app, path);
            })
            .catch((e: unknown) => failNotice('Không tạo được bài', e));
        }),
      );
    const notes = box.createDiv({ cls: 'pkm-zone-notes' });
    notes.createSpan({ text: 'Note chính: ' });
    z.core.forEach((p, i) => {
      if (i) notes.createSpan({ text: ' · ' });
      const a = notes.createEl('a', { text: title(p), href: '#' });
      a.onclick = (ev) => {
        ev.preventDefault();
        this.close();
        void openNote(this.app, p);
      };
    });
    if (z.posts.length) box.createDiv({ text: `Đã có bài: ${z.posts.map(title).join(', ')}` });
    for (const p of z.picks) box.createEl('blockquote', { text: `${cpSlice(p.text, 220)} (${p.colour})` });
  }
}
