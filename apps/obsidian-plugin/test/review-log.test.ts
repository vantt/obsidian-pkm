import { describe, expect, it } from 'vitest';
import { inQueue, logLine, StatusTracker } from '../src/review-log';

describe('review log', () => {
  it('writes one JSON line with an NFC path', () => {
    const nfd = '_Đề xuất/a.md'.normalize('NFD');
    const line = logLine({ event: 'open', path: nfd }, new Date('2026-01-02T03:04:05Z'));
    expect(line).toBe('{"at":"2026-01-02T03:04:05.000Z","event":"open","path":"_Đề xuất/a.md"}\n');
    expect(line.endsWith('\n')).toBe(true);
  });

  it('knows the proposal queue, in either normalization', () => {
    expect(inQueue('_Đề xuất/x.md')).toBe(true);
    expect(inQueue('_Đề xuất/x.md'.normalize('NFD'))).toBe(true);
    expect(inQueue('Notes/x.md')).toBe(false);
  });

  it('logs only real status changes of queued proposals', () => {
    const t = new StatusTracker();
    t.seed('_Đề xuất/a.md', 'chờ');
    t.seed('Notes/b.md', 'x');
    expect(t.change('_Đề xuất/a.md', 'chờ')).toBeNull();
    expect(t.change('_Đề xuất/a.md', 'nhận')).toEqual({ event: 'status', path: '_Đề xuất/a.md', status: 'nhận' });
    expect(t.change('_Đề xuất/a.md', 'nhận')).toBeNull();
    expect(t.change('Notes/b.md', 'y')).toBeNull();
    // a new proposal without status yet is not an event; its first status is
    expect(t.change('_Đề xuất/new.md', undefined)).toBeNull();
    expect(t.change('_Đề xuất/new.md', 'chờ')).toMatchObject({ status: 'chờ' });
  });
});
