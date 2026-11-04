// Runs `pkm <args> --json`, parses its JSON and turns every failure into a PkmError whose message is a Vietnamese
// sentence the user can act on. The plugin never computes over the whole vault itself; pkm does.
import { execFile } from 'node:child_process';

export interface RawRun {
  /** Exit code; null when the process did not exit normally (killed, never started). */
  code: number | null;
  stdout: string;
  stderr: string;
  /** Set when the process could not start (`ENOENT`, `EACCES`, ...) or its output overflowed. */
  spawnError?: string;
  timedOut: boolean;
}

export type Runner = (file: string, args: string[], opts: { timeoutMs: number; cwd?: string }) => Promise<RawRun>;

export type PkmErrorKind =
  | 'not_configured'
  | 'not_found'
  | 'not_executable'
  | 'timeout'
  | 'bad_output'
  | 'failed'
  | 'pkm_error'
  | 'ohs_not_ready';

/** `{"error":"ohs_not_ready","state","progress","eta_s","action"}`: semantic search is not ready yet. */
export interface OhsNotReady {
  error: 'ohs_not_ready';
  state: string;
  progress: number | null;
  eta_s: number | null;
  action: string;
}

export class PkmError extends Error {
  constructor(
    readonly kind: PkmErrorKind,
    message: string,
    readonly payload?: unknown,
  ) {
    super(message);
    this.name = 'PkmError';
  }
}

const MAX_OUTPUT = 64 * 1024 * 1024;

export const execRunner: Runner = (file, args, opts) =>
  new Promise((resolve) => {
    execFile(
      file,
      args,
      { timeout: opts.timeoutMs, cwd: opts.cwd, maxBuffer: MAX_OUTPUT, windowsHide: true, encoding: 'utf8' },
      (err, stdout, stderr) => {
        const out = String(stdout ?? ''), errText = String(stderr ?? '');
        if (!err) return resolve({ code: 0, stdout: out, stderr: errText, timedOut: false });
        // Exit code as a number; a start failure as a string (ENOENT); null when killed by a signal (the timeout).
        const e = err as Omit<NodeJS.ErrnoException, 'code'> & { code?: number | string | null; killed?: boolean };
        if (typeof e.code === 'number') return resolve({ code: e.code, stdout: out, stderr: errText, timedOut: false });
        const timedOut = e.killed === true && typeof e.code !== 'string';
        const spawnError = timedOut ? undefined : typeof e.code === 'string' ? e.code : e.message;
        resolve({ code: null, stdout: out, stderr: errText, timedOut, spawnError });
      },
    );
  });

export interface RunOptions {
  timeoutMs: number;
  cwd?: string;
  runner?: Runner;
  /** Append `--json` (default). `material`, `zones` and `pairs` print JSON without it and refuse the flag. */
  json?: boolean;
}

/** Runs `pkm <args> --json` (or `pkm <args>` with `json: false`) and returns the parsed JSON; throws PkmError otherwise. */
export async function runPkm(bin: string, args: string[], opts: RunOptions): Promise<unknown> {
  if (!bin.trim()) throw new PkmError('not_configured', NOT_CONFIGURED);
  const runner = opts.runner ?? execRunner;
  const full = opts.json === false ? args : [...args, '--json'];
  const raw = await runner(bin, full, { timeoutMs: opts.timeoutMs, cwd: opts.cwd });
  return interpret(raw, bin, opts.timeoutMs);
}

/** `pkm --version` as one line, for the settings check. */
export async function pkmVersion(bin: string, opts: RunOptions): Promise<string> {
  if (!bin.trim()) throw new PkmError('not_configured', NOT_CONFIGURED);
  const raw = await (opts.runner ?? execRunner)(bin, ['--version'], { timeoutMs: opts.timeoutMs });
  failIfNotRun(raw, bin, opts.timeoutMs);
  const line = raw.stdout.trim().split(/\r?\n/)[0] ?? '';
  if (raw.code !== 0 || !line.startsWith('pkm ')) {
    throw new PkmError('bad_output', `${bin} không phải pkm hoặc không chạy được: ${firstLines(raw.stderr || raw.stdout)}`);
  }
  return line;
}

const NOT_CONFIGURED = 'Chưa tìm thấy pkm. Cài pkm theo hướng dẫn trong README, hoặc ghi đường dẫn trong Cài đặt > PKM.';

function failIfNotRun(raw: RawRun, bin: string, timeoutMs: number): void {
  if (raw.timedOut) {
    throw new PkmError('timeout', `pkm không trả lời sau ${Math.round(timeoutMs / 1000)} giây nên đã bị dừng. Thử lại, hoặc tăng thời gian chờ trong Cài đặt > PKM.`);
  }
  if (raw.spawnError === 'ENOENT') {
    throw new PkmError('not_found', `Không tìm thấy pkm ở ${bin}. Cài pkm, hoặc sửa đường dẫn trong Cài đặt > PKM.`);
  }
  if (raw.spawnError === 'EACCES' || raw.spawnError === 'EPERM') {
    throw new PkmError('not_executable', `Không chạy được ${bin}: máy không cho chạy file này (quyền chạy hoặc phần mềm diệt virus chặn).`);
  }
  if (raw.spawnError === 'ERR_CHILD_PROCESS_STDIO_MAXBUFFER') {
    throw new PkmError('bad_output', 'pkm trả về quá nhiều dữ liệu; thu hẹp câu hỏi (ví dụ --limit nhỏ hơn).');
  }
  if (raw.spawnError) throw new PkmError('failed', `Không chạy được pkm: ${raw.spawnError}.`);
}

/** Turns one finished run into parsed JSON or a PkmError. Pure, for tests. */
export function interpret(raw: RawRun, bin: string, timeoutMs: number): unknown {
  failIfNotRun(raw, bin, timeoutMs);
  let parsed: unknown;
  const text = raw.stdout.trim();
  try {
    parsed = text ? JSON.parse(text) : undefined;
  } catch {
    parsed = undefined;
  }
  if (parsed === undefined) {
    if (raw.code === 0) throw new PkmError('bad_output', `pkm trả về dữ liệu không đọc được: ${firstLines(text) || '(trống)'}`);
    throw new PkmError('failed', `pkm báo lỗi (mã ${raw.code ?? '?'}): ${firstLines(raw.stderr || text) || 'không có thông báo'}`);
  }
  const err = errorOf(parsed);
  if (err) throw err;
  if (raw.code !== 0) {
    throw new PkmError('failed', `pkm báo lỗi (mã ${raw.code ?? '?'}): ${firstLines(raw.stderr) || 'không có thông báo'}`, parsed);
  }
  return parsed;
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

export function isOhsNotReady(v: unknown): v is OhsNotReady {
  return isRecord(v) && v.error === 'ohs_not_ready';
}

/** The error a pkm JSON reply carries, if any: `ohs_not_ready`, `{"error": "text"}`, or `{"error": {code, why, fix}}`. */
export function errorOf(v: unknown): PkmError | null {
  if (!isRecord(v) || v.error === undefined || v.error === null || v.error === false) return null;
  if (isOhsNotReady(v)) return new PkmError('ohs_not_ready', ohsMessage(v), v);
  if (typeof v.error === 'string') return new PkmError('pkm_error', `pkm báo lỗi: ${v.error}`, v);
  if (isRecord(v.error)) {
    const why = typeof v.error.why === 'string' ? v.error.why : typeof v.error.code === 'string' ? v.error.code : 'lỗi không rõ';
    const fix = typeof v.error.fix === 'string' && v.error.fix ? ` Cách sửa: ${v.error.fix}` : '';
    return new PkmError('pkm_error', `pkm báo lỗi: ${why}.${fix}`, v);
  }
  return new PkmError('pkm_error', 'pkm báo lỗi không rõ.', v);
}

const OHS_STATE: Record<string, string> = {
  starting: 'Tìm theo nghĩa (OHS) đang khởi động.',
  model_missing: 'Tìm theo nghĩa (OHS) chưa có model dùng được.',
  broken: 'Tìm theo nghĩa (OHS) không chạy được.',
};

export function ohsMessage(e: OhsNotReady): string {
  let head: string;
  if (e.state === 'indexing') {
    const pct = typeof e.progress === 'number' ? `: ${Math.round(e.progress * 100)}%` : '';
    const eta = typeof e.eta_s === 'number' ? `, còn ${waitText(e.eta_s)}` : '';
    head = `Tìm theo nghĩa (OHS) đang lập chỉ mục vault${pct}${eta}.`;
  } else {
    head = OHS_STATE[e.state] ?? `Tìm theo nghĩa (OHS) chưa sẵn sàng (${e.state}).`;
  }
  const action = typeof e.action === 'string' ? e.action.trim() : '';
  return action ? `${head} ${action}` : head;
}

export function waitText(seconds: number): string {
  if (seconds < 60) return `khoảng ${Math.max(1, Math.round(seconds))} giây`;
  return `khoảng ${Math.ceil(seconds / 60)} phút`;
}

function firstLines(text: string, max = 300): string {
  const t = text.trim().split(/\r?\n/).slice(0, 3).join(' ');
  return t.length > max ? `${t.slice(0, max)}…` : t;
}

/** How the commands call pkm: `args` after the binary, `json` as in RunOptions. */
export type PkmCall = (args: string[], opts?: { json?: boolean }) => Promise<unknown>;

/** Notice text for any error thrown while talking to pkm. */
export function messageFor(err: unknown): string {
  if (err instanceof PkmError) return err.message;
  return `Lỗi khi gọi pkm: ${err instanceof Error ? err.message : String(err)}`;
}
