// Writes and removes the workshop without ever overwriting what the user changed. A manifest in the vault records
// the hash of every file written; a file whose content no longer matches its hash was edited and is left alone.
// Plugin configs touched are recorded with their content before the first write, so removal restores them.
import { digest, unchanged, type YamlParser } from './digest';
import { homepageSettings, templaterSettings } from './plan';

export const MANIFEST = '_E11/workshop-ui-manifest.json';

/** Vault file access; paths are vault-relative with `/`. */
export interface VaultIO {
  exists(path: string): Promise<boolean>;
  read(path: string): Promise<string>;
  /** Creates missing parent folders. */
  write(path: string, text: string): Promise<void>;
  remove(path: string): Promise<void>;
  /** Direct children, vault-relative; null when the folder does not exist. */
  list(path: string): Promise<{ files: string[]; folders: string[] } | null>;
  rmdir(path: string): Promise<void>;
}

export interface Manifest {
  /** path -> digest of what was written */
  files: Record<string, string>;
  /** config name -> its file content before the first write (null: there was no file) */
  configs: Record<string, string | null>;
}

/** Plugins the workshop replaces; their entries leave the enabled list (restored on removal). */
const SUPERSEDED_PLUGINS = ['e11-review-log', 'e11-table-zoom'];
/** CSS snippet now shipped in the plugin's styles.css. */
const SUPERSEDED_SNIPPET = 'e11-folder-grid';

interface ConfigRule {
  path: string;
  make: (old: unknown) => unknown;
  /** Applied on removal instead of restoring the old content (file shared with other tools). */
  undo?: (cur: unknown) => unknown;
}

const asList = (v: unknown): unknown[] => (Array.isArray(v) ? v : []);
const asObject = (v: unknown): Record<string, unknown> =>
  typeof v === 'object' && v !== null && !Array.isArray(v) ? { ...(v as Record<string, unknown>) } : {};
const dropSnippet = (cur: unknown) => {
  const cfg = asObject(cur);
  cfg.enabledCssSnippets = asList(cfg.enabledCssSnippets).filter((i) => i !== SUPERSEDED_SNIPPET);
  return cfg;
};

export const CONFIGS: Record<string, ConfigRule> = {
  homepage: { path: '.obsidian/plugins/homepage/data.json', make: () => homepageSettings() },
  'templater-obsidian': { path: '.obsidian/plugins/templater-obsidian/data.json', make: templaterSettings },
  'community-plugins': {
    path: '.obsidian/community-plugins.json',
    make: (old) => asList(old).filter((id) => typeof id !== 'string' || !SUPERSEDED_PLUGINS.includes(id)),
  },
  appearance: { path: '.obsidian/appearance.json', make: dropSnippet, undo: dropSnippet },
};

const norm = (p: string) => p.replace(/\\/g, '/');

export async function loadManifest(io: VaultIO): Promise<Manifest> {
  if (!(await io.exists(MANIFEST))) return { files: {}, configs: {} };
  let raw: unknown;
  try {
    raw = JSON.parse(await io.read(MANIFEST));
  } catch (e) {
    throw new Error(`${MANIFEST} hỏng (không đọc được JSON): ${e instanceof Error ? e.message : String(e)}`);
  }
  const r = asObject(raw);
  const m: Manifest = { files: {}, configs: {} };
  for (const [k, v] of Object.entries(asObject(r.files))) if (typeof v === 'string') m.files[norm(k)] = v;
  for (const [k, v] of Object.entries(asObject(r.configs))) if (typeof v === 'string' || v === null) m.configs[k] = v;
  return m;
}

export interface Analysis {
  manifest: Manifest;
  /** Planned files that exist but were not written by the workshop: nothing is written while any remain. */
  clash: string[];
  /** Written earlier, changed by the user since: kept as is. */
  edited: string[];
  /** Written earlier, no longer planned: removed unless edited. */
  stale: string[];
}

async function isUnchanged(io: VaultIO, path: string, stored: string, parseYaml: YamlParser): Promise<boolean> {
  return unchanged(path, await io.read(path), stored, parseYaml);
}

export async function analyse(io: VaultIO, files: Map<string, string>, parseYaml: YamlParser): Promise<Analysis> {
  const manifest = await loadManifest(io);
  const clash: string[] = [], edited: string[] = [];
  for (const p of files.keys()) {
    if (!(await io.exists(p))) continue;
    const stored = manifest.files[p];
    if (stored === undefined) clash.push(p);
    else if (!(await isUnchanged(io, p, stored, parseYaml))) edited.push(p);
  }
  const stale = Object.keys(manifest.files).filter((p) => !files.has(p));
  return { manifest, clash, edited, stale };
}

export interface ApplyResult {
  written: string[];
  kept: string[];
  removed: string[];
}

const parseJson = (text: string, path: string): unknown => {
  try {
    return JSON.parse(text);
  } catch {
    throw new Error(`${path} không phải JSON hợp lệ; sửa hoặc xoá file đó rồi chạy lại.`);
  }
};

/** Writes the plan. Refuses (throws) when `analysis.clash` is not empty. */
export async function applyPlan(
  io: VaultIO,
  files: Map<string, string>,
  analysis: Analysis,
  parseYaml: YamlParser,
): Promise<ApplyResult> {
  if (analysis.clash.length) throw new Error(`đã có file không do xưởng tạo: ${analysis.clash.join(', ')}`);
  const { manifest } = analysis;
  const res: ApplyResult = { written: [], kept: [...analysis.edited], removed: [] };
  // Configs first: a broken JSON stops before any note is written.
  const configs: [string, ConfigRule, string | null][] = [];
  for (const [name, rule] of Object.entries(CONFIGS)) {
    const old = (await io.exists(rule.path)) ? await io.read(rule.path) : null;
    if (old !== null) parseJson(old, rule.path);
    configs.push([name, rule, old]);
  }
  for (const [p, text] of files) {
    if (analysis.edited.includes(p)) continue;
    await io.write(p, text);
    manifest.files[p] = digest(p, text, parseYaml);
    res.written.push(p);
  }
  for (const p of analysis.stale) {
    const stored = manifest.files[p];
    if (stored !== undefined && (await io.exists(p))) {
      if (!(await isUnchanged(io, p, stored, parseYaml))) {
        res.kept.push(p);
        continue;
      }
      await io.remove(p);
      res.removed.push(p);
    }
    delete manifest.files[p];
  }
  for (const [name, rule, old] of configs) {
    if (!(name in manifest.configs)) manifest.configs[name] = old;
    const next = rule.make(old === null ? null : parseJson(old, rule.path));
    await io.write(rule.path, JSON.stringify(next, null, 2));
  }
  await io.write(MANIFEST, JSON.stringify(manifest, null, 1));
  return res;
}

export interface RemoveResult {
  removed: string[];
  kept: string[];
}

async function removeEmptyParents(io: VaultIO, path: string): Promise<void> {
  const parts = path.split('/');
  for (let i = parts.length - 1; i > 0; i--) {
    const dir = parts.slice(0, i).join('/');
    const entry = await io.list(dir);
    if (!entry || entry.files.length || entry.folders.length) return;
    await io.rmdir(dir);
  }
}

/** Deletes what the workshop wrote (edited files are kept) and restores the configs. */
export async function removeWorkshop(io: VaultIO, parseYaml: YamlParser): Promise<RemoveResult> {
  if (!(await io.exists(MANIFEST))) throw new Error('vault chưa dựng xưởng (không có manifest).');
  const manifest = await loadManifest(io);
  const res: RemoveResult = { removed: [], kept: [] };
  for (const [p, stored] of Object.entries(manifest.files)) {
    if (!(await io.exists(p))) continue;
    if (!(await isUnchanged(io, p, stored, parseYaml))) {
      res.kept.push(p);
      continue;
    }
    await io.remove(p);
    res.removed.push(p);
    await removeEmptyParents(io, p);
  }
  for (const [name, old] of Object.entries(manifest.configs)) {
    const rule = CONFIGS[name];
    if (!rule) continue;
    if (rule.undo) {
      const cur = (await io.exists(rule.path)) ? parseJson(await io.read(rule.path), rule.path) : null;
      await io.write(rule.path, JSON.stringify(rule.undo(cur), null, 2));
    } else if (old === null) {
      if (await io.exists(rule.path)) await io.remove(rule.path);
    } else {
      await io.write(rule.path, old);
    }
  }
  await io.remove(MANIFEST);
  return res;
}
