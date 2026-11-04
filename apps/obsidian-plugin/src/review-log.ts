// Local log of what the user opens while reviewing proposals, and of every proposal `status` change. Nothing leaves
// the vault. Lines in _E11/review-log.jsonl: {"at", "event": "open", "path"} and {"at", "event": "status", "path",
// "status"}; the proposal statistics turn them into acceptance and source-open rates.
import type { Plugin, TFile } from 'obsidian';

export const REVIEW_LOG = '_E11/review-log.jsonl';
export const PROPOSAL_QUEUE = '_Đề xuất/';

export type ReviewEvent = { event: 'open'; path: string } | { event: 'status'; path: string; status: unknown };

export const inQueue = (path: string) => path.normalize('NFC').startsWith(PROPOSAL_QUEUE);

export function logLine(entry: ReviewEvent, now: Date): string {
  return JSON.stringify({ at: now.toISOString(), ...entry, path: entry.path.normalize('NFC') }) + '\n';
}

/** Remembers each queued proposal's status and reports only real changes. */
export class StatusTracker {
  private readonly status = new Map<string, unknown>();

  seed(path: string, status: unknown): void {
    if (inQueue(path)) this.status.set(path, status);
  }

  /** The event to log for a metadata change, or null. */
  change(path: string, status: unknown): ReviewEvent | null {
    if (!inQueue(path)) return null;
    if (this.status.get(path) === status) return null;
    this.status.set(path, status);
    return { event: 'status', path, status };
  }
}

export function startReviewLog(plugin: Plugin): void {
  const { app } = plugin;
  const tracker = new StatusTracker();
  let queue = Promise.resolve();
  const log = (entry: ReviewEvent) => {
    const line = logLine(entry, new Date());
    // Appends run one after another so two quick events cannot both create the file.
    queue = queue
      .then(async () => {
        const adapter = app.vault.adapter;
        if (!(await adapter.exists('_E11'))) await adapter.mkdir('_E11');
        if (await adapter.exists(REVIEW_LOG)) await adapter.append(REVIEW_LOG, line);
        else await adapter.write(REVIEW_LOG, line);
      })
      .catch((e: unknown) => console.error('pkm: review log write failed', e));
  };
  app.workspace.onLayoutReady(() => {
    for (const f of app.vault.getMarkdownFiles()) tracker.seed(f.path, app.metadataCache.getFileCache(f)?.frontmatter?.status);
    plugin.registerEvent(app.workspace.on('file-open', (f: TFile | null) => f && log({ event: 'open', path: f.path })));
    plugin.registerEvent(
      app.metadataCache.on('changed', (f, _data, cache) => {
        const ev = tracker.change(f.path, cache?.frontmatter?.status);
        if (ev) log(ev);
      }),
    );
  });
}
