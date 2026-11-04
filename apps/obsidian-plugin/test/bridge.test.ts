import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { errorOf, execRunner, interpret, ohsMessage, PkmError, pkmVersion, type RawRun, runPkm, waitText } from '../src/bridge';

const ok = (stdout: string, code = 0): RawRun => ({ code, stdout, stderr: '', timedOut: false });

function kindOf(fn: () => unknown): string {
  try {
    fn();
  } catch (e) {
    if (e instanceof PkmError) return e.kind;
    throw e;
  }
  return 'none';
}

describe('interpret', () => {
  it('returns parsed JSON on success', () => {
    expect(interpret(ok('{"hits": [1, 2]}\n'), 'pkm', 1000)).toEqual({ hits: [1, 2] });
    expect(interpret(ok('{"schema":1,"ok":true,"error":null,"data":{}}'), 'pkm', 1000)).toMatchObject({ ok: true });
  });

  it('maps ohs_not_ready to a Vietnamese notice with progress, wait and action', () => {
    const body = { error: 'ohs_not_ready', state: 'indexing', progress: 0.64, eta_s: 180, action: 'Thử lại sau.' };
    try {
      interpret(ok(JSON.stringify(body), 1), 'pkm', 1000);
      expect.unreachable();
    } catch (e) {
      expect(e).toBeInstanceOf(PkmError);
      const err = e as PkmError;
      expect(err.kind).toBe('ohs_not_ready');
      expect(err.message).toBe('Tìm theo nghĩa (OHS) đang lập chỉ mục vault: 64%, còn khoảng 3 phút. Thử lại sau.');
      expect(err.payload).toEqual(body);
    }
  });

  it('handles null progress and every state', () => {
    const base = { error: 'ohs_not_ready' as const, progress: null, eta_s: null, action: '' };
    expect(ohsMessage({ ...base, state: 'indexing' })).toBe('Tìm theo nghĩa (OHS) đang lập chỉ mục vault.');
    expect(ohsMessage({ ...base, state: 'indexing', progress: 0.5 })).toBe('Tìm theo nghĩa (OHS) đang lập chỉ mục vault: 50%.');
    expect(ohsMessage({ ...base, state: 'starting' })).toBe('Tìm theo nghĩa (OHS) đang khởi động.');
    expect(ohsMessage({ ...base, state: 'broken', action: 'Cài gói OHS.' })).toBe('Tìm theo nghĩa (OHS) không chạy được. Cài gói OHS.');
    expect(ohsMessage({ ...base, state: 'model_missing' })).toContain('chưa có model');
    expect(ohsMessage({ ...base, state: 'later_state' })).toContain('(later_state)');
  });

  it('formats waits', () => {
    expect(waitText(0)).toBe('khoảng 1 giây');
    expect(waitText(42)).toBe('khoảng 42 giây');
    expect(waitText(61)).toBe('khoảng 2 phút');
  });

  it('maps string and envelope errors', () => {
    expect(errorOf({ error: 'index unavailable: x' })?.message).toBe('pkm báo lỗi: index unavailable: x');
    const env = { schema: 1, ok: false, data: {}, error: { code: 'checks_failed', why: '1 mục lỗi: db', fix: 'pkm install' }, next: [] };
    expect(errorOf(env)?.message).toBe('pkm báo lỗi: 1 mục lỗi: db. Cách sửa: pkm install');
    expect(errorOf({ error: null })).toBeNull();
    expect(errorOf([1])).toBeNull();
  });

  it('classifies process failures', () => {
    expect(kindOf(() => interpret({ code: null, stdout: '', stderr: '', timedOut: true }, 'pkm', 5000))).toBe('timeout');
    expect(kindOf(() => interpret({ code: null, stdout: '', stderr: '', timedOut: false, spawnError: 'ENOENT' }, 'pkm', 5000))).toBe('not_found');
    expect(kindOf(() => interpret({ code: null, stdout: '', stderr: '', timedOut: false, spawnError: 'EACCES' }, 'pkm', 5000))).toBe('not_executable');
    expect(kindOf(() => interpret({ code: 2, stdout: '', stderr: 'error: unexpected argument', timedOut: false }, 'pkm', 5000))).toBe('failed');
    expect(kindOf(() => interpret(ok('not json'), 'pkm', 5000))).toBe('bad_output');
    expect(kindOf(() => interpret(ok('{"a":1}', 3), 'pkm', 5000))).toBe('failed');
  });

  it('says how long it waited on timeout', () => {
    expect(() => interpret({ code: null, stdout: '', stderr: '', timedOut: true }, 'pkm', 90000)).toThrow(/90 giây/);
  });
});

describe('runPkm', () => {
  it('appends --json and passes the timeout', async () => {
    const calls: unknown[] = [];
    const out = await runPkm('/x/pkm', ['vault', 'stats'], {
      timeoutMs: 1234,
      runner: async (file, args, opts) => {
        calls.push([file, args, opts.timeoutMs]);
        return ok('{"notes": 3}');
      },
    });
    expect(out).toEqual({ notes: 3 });
    expect(calls).toEqual([['/x/pkm', ['vault', 'stats', '--json'], 1234]]);
  });

  it('refuses an empty binary path', async () => {
    await expect(runPkm(' ', ['doctor'], { timeoutMs: 1 })).rejects.toMatchObject({ kind: 'not_configured' });
  });
});

describe('execRunner against real processes', () => {
  const node = process.execPath;

  it('captures stdout and exit codes', async () => {
    const r = await execRunner(node, ['-e', 'console.log(JSON.stringify({a:1})); process.exit(3)'], { timeoutMs: 10000 });
    expect(r).toMatchObject({ code: 3, timedOut: false });
    expect(JSON.parse(r.stdout)).toEqual({ a: 1 });
  });

  it('kills a process that runs past the timeout', async () => {
    const r = await execRunner(node, ['-e', 'setTimeout(() => {}, 20000)'], { timeoutMs: 300 });
    expect(r.timedOut).toBe(true);
    expect(r.code).toBeNull();
  });

  it('reports a missing binary as ENOENT', async () => {
    const r = await execRunner(path.join(__dirname, 'no-such-pkm'), [], { timeoutMs: 1000 });
    expect(r.spawnError).toBe('ENOENT');
  });

  it('pkmVersion rejects a binary that is not pkm', async () => {
    await expect(pkmVersion(node, { timeoutMs: 10000 })).rejects.toMatchObject({ kind: 'bad_output' });
  });
});
