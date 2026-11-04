// Hashes recorded in the workshop manifest, compatible with the ones the earlier Python script wrote, so a vault
// set up by it keeps its "edited by the user" detection.
import { createHash } from 'node:crypto';

export function sha(text: string): string {
  return createHash('sha256').update(text, 'utf8').digest('hex');
}

/** `json.dumps(value, sort_keys=True, ensure_ascii=False)` of Python, for values parsed from YAML or JSON; with
 * `sortKeys` false, keys keep their insertion order (`json.dumps(value, ensure_ascii=False)`). */
export function pyJson(v: unknown, sortKeys = true): string {
  const rec = (x: unknown) => pyJson(x, sortKeys);
  if (v === null || v === undefined) return 'null';
  if (typeof v === 'boolean') return v ? 'true' : 'false';
  if (typeof v === 'number') {
    if (Number.isNaN(v)) return 'NaN';
    if (!Number.isFinite(v)) return v > 0 ? 'Infinity' : '-Infinity';
    return String(v);
  }
  if (typeof v === 'string') return JSON.stringify(v);
  if (Array.isArray(v)) return `[${v.map(rec).join(', ')}]`;
  if (v instanceof Date) return JSON.stringify(v.toISOString());
  if (typeof v === 'object') {
    const keys = Object.keys(v);
    if (sortKeys) keys.sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
    return `{${keys.map((k) => `${JSON.stringify(k)}: ${rec((v as Record<string, unknown>)[k])}`).join(', ')}}`;
  }
  return JSON.stringify(String(v));
}

export type YamlParser = (text: string) => unknown;

/**
 * Obsidian rewrites a .base in its own layout (key order, quoting) once a view is touched; hash the parsed content
 * so that does not count as a user edit.
 */
export function digest(path: string, text: string, parseYaml: YamlParser): string {
  if (path.endsWith('.base')) {
    try {
      return sha(pyJson(parseYaml(text)));
    } catch {
      // not YAML any more: fall back to the raw text
    }
  }
  return sha(text);
}

/** True when `text` is what the workshop wrote (`stored` digest), in either hash form. */
export function unchanged(path: string, text: string, stored: string, parseYaml: YamlParser): boolean {
  return stored === digest(path, text, parseYaml) || stored === sha(text);
}
