// Stand-in for the `obsidian` module in unit tests: only what the pure modules import at load time. UI classes are
// empty shells; tests never drive them.
import { parse } from 'yaml';

export const parseYaml = (text: string): unknown => parse(text);
export const normalizePath = (p: string): string => p.replace(/\\/g, '/').replace(/\/+/g, '/').replace(/^\/|\/$/g, '');

export class TAbstractFile {
  constructor(public path: string) {}
}
export class TFile extends TAbstractFile {}
export class TFolder extends TAbstractFile {}
export class Plugin {}
export class PluginSettingTab {}
export class Modal {}
export class Setting {}
export class MarkdownView {}
export class Notice {
  static shown: string[] = [];
  constructor(message: string) {
    Notice.shown.push(message);
  }
  hide(): void {}
}
export const setIcon = (): void => {};
