import { type App, Notice, PluginSettingTab, Setting } from 'obsidian';
import { messageFor, pkmVersion } from './bridge';
import type PkmPlugin from './main';
import { MAX_TIMEOUT, MIN_TIMEOUT, parseFolderList } from './settings';

export class PkmSettingTab extends PluginSettingTab {
  constructor(
    app: App,
    private readonly plugin: PkmPlugin,
  ) {
    super(app, plugin);
  }

  override display(): void {
    const { containerEl } = this;
    containerEl.empty();
    const s = this.plugin.pkmSettings;
    const detected = this.plugin.detectedPkm();

    new Setting(containerEl)
      .setName('Đường dẫn tới pkm')
      .setDesc(
        detected
          ? `Để trống thì dùng bản bộ cài đặt ở ${detected}.`
          : 'Chưa thấy pkm ở chỗ bộ cài đặt. Cài pkm, hoặc ghi đường dẫn đầy đủ tới file pkm (pkm.exe trên Windows).',
      )
      .addText((t) =>
        t
          .setPlaceholder(detected ?? 'pkm')
          .setValue(s.pkmPath)
          .onChange(async (v) => {
            s.pkmPath = v.trim();
            await this.plugin.saveSettings();
          }),
      )
      .addButton((b) =>
        b.setButtonText('Kiểm tra').onClick(async () => {
          try {
            new Notice(`Chạy được: ${await pkmVersion(this.plugin.pkmBinary(), { timeoutMs: 15000 })}`);
          } catch (e) {
            new Notice(messageFor(e), 10000);
          }
        }),
      );

    new Setting(containerEl)
      .setName('Thời gian chờ pkm (giây)')
      .setDesc(`Quá thời gian này thì dừng pkm và báo lỗi (${MIN_TIMEOUT} đến ${MAX_TIMEOUT}).`)
      .addText((t) =>
        t.setValue(String(s.timeoutSeconds)).onChange(async (v) => {
          const n = Number(v);
          if (!Number.isInteger(n) || n < MIN_TIMEOUT || n > MAX_TIMEOUT) return;
          s.timeoutSeconds = n;
          await this.plugin.saveSettings();
        }),
      );

    new Setting(containerEl)
      .setName('Index graph (--db)')
      .setDesc(
        'Đường dẫn đầy đủ tới file index graph mà Nguyên liệu, Gợi ý bài viết và cặp gần nghĩa đọc. Để trống thì pkm dùng VAULT/.kg/graph.db. pkm setup tự điền.',
      )
      .addText((t) =>
        t
          .setPlaceholder('VAULT/.kg/graph.db')
          .setValue(s.graphDbPath)
          .onChange(async (v) => {
            s.graphDbPath = v.trim();
            await this.plugin.saveSettings();
          }),
      );

    new Setting(containerEl)
      .setName('Cây thư mục theo note đang mở')
      .setDesc(
        'Mở note từ link, quick switcher hay tab thì cây thư mục mở thư mục chứa note và cuộn dòng của nó vào giữa. Bấm file ngay trong cây thì cây đứng yên.',
      )
      .addToggle((t) =>
        t.setValue(s.navFollow).onChange(async (v) => {
          s.navFollow = v;
          await this.plugin.saveSettings();
        }),
      );

    containerEl.createEl('h3', { text: 'Dựng xưởng' });

    new Setting(containerEl)
      .setName('Thư mục chủ đề')
      .setDesc(
        'Mỗi dòng một thư mục gốc. Mỗi thư mục cháu (gốc/nhóm/chủ đề) có từ 3 note trở lên được một trang chủ đề. Để trống thì không tạo trang chủ đề.',
      )
      .addTextArea((t) =>
        t.setValue(s.topicRoots.join('\n')).onChange(async (v) => {
          s.topicRoots = parseFolderList(v);
          await this.plugin.saveSettings();
        }),
      );

    new Setting(containerEl)
      .setName('Thư mục lưu nhanh khác')
      .setDesc('Mỗi dòng một thư mục, hiện ở mục "Mới lưu" trên trang chủ cùng với Inbox. Tên thư mục không được có dấu nháy.')
      .addTextArea((t) =>
        t.setValue(s.inboxFolders.join('\n')).onChange(async (v) => {
          s.inboxFolders = parseFolderList(v);
          await this.plugin.saveSettings();
        }),
      );
  }
}
