// Refreshing the queue: `pkm VAULT pairs --write` first (semantic pairs for the `related` kind; needs OHS), then
// `pkm VAULT propose --create`. Without OHS the queue is still refreshed, only without `related`.
import path from 'node:path';
import { messageFor, type PkmCall, PkmError } from '../bridge';
import { isRecord } from '../text';

export const SIMILAR_PAIRS = path.join('_E11', 'similar-pairs.json');

export interface ProposeResult {
  total: number;
  new: number;
  byKind: Record<string, number>;
  written: string[];
}

export function parseProposeResult(v: unknown): ProposeResult {
  const ok =
    isRecord(v) &&
    typeof v.total === 'number' &&
    typeof v.new === 'number' &&
    isRecord(v.by_kind) &&
    Array.isArray(v.written) &&
    v.written.every((w) => typeof w === 'string');
  if (!ok) throw new PkmError('bad_output', 'pkm propose trả về dữ liệu không đúng dạng {total, new, by_kind, written}.');
  const byKind: Record<string, number> = {};
  for (const [k, n] of Object.entries(v.by_kind as Record<string, unknown>)) if (typeof n === 'number') byKind[k] = n;
  return { total: v.total as number, new: v.new as number, byKind, written: v.written as string[] };
}

/**
 * Runs pairs then propose. `dbArgs` is `['--db', PATH]` when a graph index is configured. `warning` says why the
 * `related` kind was left out, if it was.
 */
export async function refreshProposals(call: PkmCall, vault: string, dbArgs: string[]): Promise<{ result: ProposeResult; warning: string | null }> {
  let warning: string | null = null;
  const similar: string[] = [];
  try {
    await call([vault, 'pairs', '--write', ...dbArgs], { json: false });
    similar.push('--similar', path.join(vault, SIMILAR_PAIRS));
  } catch (e) {
    const why = e instanceof PkmError && e.kind === 'ohs_not_ready' ? e.message : messageFor(e);
    warning = `${why} Lần này không có đề xuất "liên quan".`;
  }
  const result = parseProposeResult(await call([vault, 'propose', '--create', ...similar]));
  return { result, warning };
}
