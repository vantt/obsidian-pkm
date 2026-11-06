import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
// @ts-expect-error plain ESM script without types
import { workspaceVersion } from '../scripts/check-version.mjs';
import { candidatePaths, detectPkm } from '../src/pkm-path';
import { cleanFolder, DEFAULT_SETTINGS, normalizeSettings, parseFolderList } from '../src/settings';

describe('pkm binary location', () => {
  it('uses the Windows installer folder under LOCALAPPDATA', () => {
    const c = candidatePaths({ platform: 'win32', env: { LOCALAPPDATA: 'C:\\Users\\u\\AppData\\Local' }, home: 'C:\\Users\\u' });
    expect(c).toEqual(['C:\\Users\\u\\AppData\\Local\\pkm\\bin\\pkm.exe']);
  });

  it('uses ~/.local/bin elsewhere, PKM_INSTALL_DIR first', () => {
    expect(candidatePaths({ platform: 'darwin', env: {}, home: '/Users/u' })).toEqual(['/Users/u/.local/bin/pkm']);
    expect(candidatePaths({ platform: 'linux', env: { PKM_INSTALL_DIR: '/opt/pkm' }, home: '/home/u' })).toEqual([
      '/opt/pkm/pkm',
      '/home/u/.local/bin/pkm',
    ]);
  });

  it('detects the first existing candidate', () => {
    const host = { platform: 'linux' as const, env: { PKM_INSTALL_DIR: '/opt/pkm' }, home: '/home/u' };
    expect(detectPkm(host, (f) => f === '/home/u/.local/bin/pkm')).toBe('/home/u/.local/bin/pkm');
    expect(detectPkm(host, () => false)).toBeNull();
  });
});

describe('settings', () => {
  it('cleans folder entries', () => {
    expect(cleanFolder(' /Notes\\Projects/ ')).toBe('Notes/Projects');
    expect(cleanFolder('../x')).toBeNull();
    expect(cleanFolder('a"b')).toBeNull();
    expect(cleanFolder("it's")).toBeNull();
    expect(parseFolderList('A\nB, A\n\n../C')).toEqual(['A', 'B']);
  });

  it('normalizes hand-edited data', () => {
    expect(normalizeSettings(null)).toEqual(DEFAULT_SETTINGS);
    expect(normalizeSettings({ graphDbPath: 3 }).graphDbPath).toBe('');
    expect(
      normalizeSettings({ pkmPath: ' /x/pkm ', timeoutSeconds: 1, graphDbPath: ' /v/.e11/graph.db ', topicRoots: ['A', 3, '..'], inboxFolders: 'x', navFollow: false, compareScroll: false }),
    ).toEqual({
      pkmPath: '/x/pkm',
      timeoutSeconds: 5,
      graphDbPath: '/v/.e11/graph.db',
      topicRoots: ['A'],
      inboxFolders: [],
      navFollow: false,
      compareScroll: false,
    });
    expect(normalizeSettings({ navFollow: 'no' }).navFollow).toBe(true);
    expect(normalizeSettings({ compareScroll: 1 }).compareScroll).toBe(true);
  });
});

describe('version check', () => {
  it('reads [workspace.package] version only', () => {
    const toml = '[package]\nversion = "9.9.9"\n\n[workspace.package]\nedition = "2024"\nversion = "1.2.3-rc.1"\n\n[x]\nversion = "0"\n';
    expect(workspaceVersion(toml)).toBe('1.2.3-rc.1');
    expect(() => workspaceVersion('[workspace]\n')).toThrow();
  });

  it('manifest.json and package.json carry the Cargo workspace version', () => {
    const root = path.resolve(__dirname, '..');
    const want = workspaceVersion(fs.readFileSync(path.resolve(root, '..', '..', 'Cargo.toml'), 'utf8'));
    for (const f of ['manifest.json', 'package.json']) {
      expect(JSON.parse(fs.readFileSync(path.join(root, f), 'utf8')).version).toBe(want);
    }
  });
});
