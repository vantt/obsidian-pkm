import fs from 'node:fs';
import os from 'node:os';
import { FileSystemAdapter, Notice, Plugin } from 'obsidian';
import { messageFor, type PkmCall, runPkm } from './bridge';
import { activeDraft, materialCommand } from './material/command';
import { detectPkm } from './pkm-path';
import { startReviewLog } from './review-log';
import { normalizeSettings, type PkmSettings } from './settings';
import { applyCommand, ProposalsModal, refreshCommand, revertCommand, statsCommand } from './proposals/commands';
import { PkmSettingTab } from './settings-tab';
import { TableZoom } from './table-zoom';
import { startThemeGuard } from './theme-guard';
import { buildWorkshop, removeWorkshopCommand } from './workshop/command';
import { zonesCommand } from './zones/commands';

interface DoctorCheck {
  id: string;
  status: string;
  detail?: string | null;
  action?: string | null;
}

export default class PkmPlugin extends Plugin {
  pkmSettings!: PkmSettings;
  private tableZoom: TableZoom | null = null;

  override async onload(): Promise<void> {
    this.pkmSettings = normalizeSettings(await this.loadData());
    this.addSettingTab(new PkmSettingTab(this.app, this));

    this.addCommand({ id: 'workshop-build', name: 'Dựng xưởng', callback: () => void buildWorkshop(this.app, this.pkmSettings) });
    this.addCommand({ id: 'workshop-remove', name: 'Gỡ xưởng', callback: () => removeWorkshopCommand(this.app) });
    this.addCommand({ id: 'doctor', name: 'Kiểm tra pkm (doctor)', callback: () => void this.doctor() });

    this.addCommand({ id: 'proposals-review', name: 'Đề xuất chờ duyệt', callback: () => new ProposalsModal(this).open() });
    this.addCommand({ id: 'proposals-refresh', name: 'Làm mới đề xuất', callback: () => void refreshCommand(this) });
    this.addCommand({ id: 'proposals-apply', name: 'Áp dụng đề xuất đã chọn', callback: () => void applyCommand(this.app) });
    this.addCommand({ id: 'proposals-revert', name: 'Hoàn tác đề xuất đã áp dụng', callback: () => revertCommand(this.app) });
    this.addCommand({ id: 'proposals-stats', name: 'Đo duyệt đề xuất', callback: () => void statsCommand(this.app) });
    this.addCommand({
      id: 'material',
      name: 'Nguyên liệu cho bản nháp này',
      checkCallback: (checking) => {
        const draft = activeDraft(this);
        if (!draft) return false;
        if (!checking) void materialCommand(this, draft);
        return true;
      },
    });
    this.addCommand({ id: 'zones', name: 'Gợi ý bài viết (vùng tri thức)', callback: () => void zonesCommand(this) });

    startReviewLog(this);
    startThemeGuard(this);
    this.tableZoom = new TableZoom(this);
    this.tableZoom.load();
  }

  override onunload(): void {
    this.tableZoom?.unload();
  }

  async saveSettings(): Promise<void> {
    await this.saveData(this.pkmSettings);
  }

  /** The installer's pkm if present, else null. */
  detectedPkm(): string | null {
    return detectPkm({ platform: process.platform, env: process.env, home: os.homedir() }, (f) => fs.existsSync(f));
  }

  /** Path to run: the setting, else the detected install; empty when neither (runPkm reports it). */
  pkmBinary(): string {
    return this.pkmSettings.pkmPath || this.detectedPkm() || '';
  }

  /** Runs `pkm <args> --json` (`json: false`: without the flag) with the configured timeout. */
  runPkm(args: string[], json = true): Promise<unknown> {
    return runPkm(this.pkmBinary(), args, { timeoutMs: this.pkmSettings.timeoutSeconds * 1000, json });
  }

  readonly callPkm: PkmCall = (args, opts) => this.runPkm(args, opts?.json ?? true);

  /** The vault folder on disk, which pkm takes as VAULT; null (with a notice) when the vault is not a folder. */
  vaultPath(): string | null {
    const adapter = this.app.vault.adapter;
    if (adapter instanceof FileSystemAdapter) return adapter.getBasePath();
    new Notice('Lệnh này cần vault là một thư mục trên máy (Obsidian trên máy tính).');
    return null;
  }

  /** `['--db', PATH]` when a graph index is set, for material, zones and pairs. */
  dbArgs(): string[] {
    return this.pkmSettings.graphDbPath ? ['--db', this.pkmSettings.graphDbPath] : [];
  }

  private async doctor(): Promise<void> {
    const notice = new Notice('Đang kiểm tra pkm…', 0);
    try {
      const r = (await this.runPkm(['doctor'])) as { ok?: boolean; data?: { checks?: DoctorCheck[] } };
      const checks = r.data?.checks ?? [];
      const warn = checks.filter((c) => c.status === 'warn');
      notice.hide();
      const lines = warn.map((c) => `${c.id}: ${c.action ?? c.detail ?? ''}`);
      new Notice(`pkm ổn (${checks.length} mục kiểm)${lines.length ? `; cần xem: ${lines.join('; ')}` : '.'}`, 10000);
    } catch (e) {
      notice.hide();
      new Notice(messageFor(e), 15000);
    }
  }
}
