import fs from 'node:fs';
import os from 'node:os';
import { Notice, Plugin } from 'obsidian';
import { messageFor, runPkm } from './bridge';
import { detectPkm } from './pkm-path';
import { startReviewLog } from './review-log';
import { normalizeSettings, type PkmSettings } from './settings';
import { PkmSettingTab } from './settings-tab';
import { TableZoom } from './table-zoom';
import { buildWorkshop, removeWorkshopCommand } from './workshop/command';

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

    startReviewLog(this);
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

  /** Runs `pkm <args> --json` with the configured timeout. */
  runPkm(args: string[]): Promise<unknown> {
    return runPkm(this.pkmBinary(), args, { timeoutMs: this.pkmSettings.timeoutSeconds * 1000 });
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
