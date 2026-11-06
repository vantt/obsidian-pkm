import { describe, expect, it } from 'vitest';
import { followerTop, GESTURE_MS, type ScrollPane, ScrollLockSession } from '../src/compare-scroll/session';

class Pane extends EventTarget implements ScrollPane {
  scrollTop: number;
  isConnected = true;
  constructor(
    top: number,
    readonly ownerDocument: { defaultView: EventTarget },
    readonly scrollHeight = 10000,
    readonly clientHeight = 700,
  ) {
    super();
    this.scrollTop = top;
  }
}

function setup(tops: [number, number] = [1200, 5400], heights: [number, number] = [10000, 10000]) {
  const win = new EventTarget();
  let t = 1_000_000;
  const panes = [new Pane(tops[0], { defaultView: win }, heights[0]), new Pane(tops[1], { defaultView: win }, heights[1])] as const;
  const alts: boolean[] = [];
  let invalid = 0;
  const s = new ScrollLockSession(panes, (a) => alts.push(a), () => invalid++, () => t);
  const scroll = (i: 0 | 1, top: number) => {
    panes[i].scrollTop = top;
    panes[i].dispatchEvent(new Event('scroll'));
  };
  const echo = (i: 0 | 1) => panes[i].dispatchEvent(new Event('scroll'));
  const key = (type: 'keydown' | 'keyup', altKey: boolean) => win.dispatchEvent(Object.assign(new Event(type), { altKey }));
  const wheel = (i: 0 | 1, altKey: boolean) => panes[i].dispatchEvent(Object.assign(new Event('wheel'), { altKey }));
  return { s, panes, scroll, echo, key, wheel, alts, invalid: () => invalid, tick: (ms: number) => (t += ms) };
}

describe('followerTop', () => {
  it('applies the offset in both directions and clamps to the follower range', () => {
    expect(followerTop(0, 1000, 300, 9300)).toBe(1300);
    expect(followerTop(1, 1300, 300, 9300)).toBe(1000);
    expect(followerTop(0, 9200, 300, 9300)).toBe(9300);
    expect(followerTop(1, 100, 300, 9300)).toBe(0);
    expect(followerTop(0, 10, 300, -5)).toBe(0);
  });
});

describe('scroll lock session', () => {
  it('locks at the current positions and follows both ways', () => {
    const f = setup();
    expect(f.panes.map((p) => p.scrollTop)).toEqual([1200, 5400]);
    f.scroll(0, 1370);
    expect(f.panes[1].scrollTop).toBe(5570);
    f.echo(1);
    expect(f.panes[0].scrollTop).toBe(1370);
    f.scroll(1, 5440);
    expect(f.panes[0].scrollTop).toBe(1240);
  });

  it('keeps the locked offset after one pane hits its bottom (the 300 -> 1800 px bug)', () => {
    const f = setup([8049, 8349], [9049, 11000]);
    f.scroll(0, 8349); // pane 0 at its bottom (9049 - 700)
    f.scroll(1, 10149); // pane 1 scrolls on; pane 0 stays clamped
    expect(f.panes[0].scrollTop).toBe(8349);
    f.echo(0);
    f.scroll(1, 8349);
    expect(f.panes[1].scrollTop - f.panes[0].scrollTop).toBe(300);
  });

  it('keeps the locked offset after one pane hits its top', () => {
    const f = setup();
    f.scroll(1, 1000);
    expect(f.panes[0].scrollTop).toBe(0);
    f.echo(0);
    f.scroll(1, 5400);
    expect(f.panes[0].scrollTop).toBe(1200);
  });

  it('holding Alt scrolls one pane alone and re-anchors on release', () => {
    const f = setup();
    f.key('keydown', true);
    expect(f.s.alt).toBe(true);
    f.scroll(0, 2300);
    expect(f.panes[1].scrollTop).toBe(5400);
    f.key('keyup', false);
    expect(f.alts).toEqual([true, false]);
    f.scroll(0, 2400);
    expect(f.panes[1].scrollTop).toBe(5500);
  });

  it('an Alt wheel gesture is independent only while it lasts', () => {
    const f = setup();
    f.wheel(0, true);
    f.key('keyup', false); // Alt released, momentum events still belong to the gesture
    f.scroll(0, 1500);
    expect(f.panes[1].scrollTop).toBe(5400);
    f.tick(GESTURE_MS + 1);
    f.scroll(0, 1600);
    expect(f.panes[1].scrollTop).toBe(5500);
  });

  it('Alt while one pane is clamped re-anchors at the visible positions', () => {
    const f = setup();
    f.scroll(0, 8000);
    expect(f.panes[1].scrollTop).toBe(9300);
    f.key('keydown', true);
    f.scroll(0, 7000);
    f.key('keyup', false);
    f.scroll(0, 6900);
    expect(f.panes[1].scrollTop).toBe(9200);
  });

  it('window blur clears Alt', () => {
    const f = setup();
    f.key('keydown', true);
    (f.panes[0].ownerDocument.defaultView as EventTarget).dispatchEvent(new Event('blur'));
    expect(f.s.alt).toBe(false);
  });

  it('a pane leaving the DOM reports invalid; dispose removes every listener', () => {
    const f = setup();
    f.panes[1].isConnected = false;
    f.scroll(0, 1300);
    expect(f.invalid()).toBe(1);
    f.panes[1].isConnected = true;
    f.s.dispose();
    f.scroll(0, 1500);
    expect(f.panes[1].scrollTop).toBe(5400);
    f.key('keydown', true);
    expect(f.s.alt).toBe(false);
  });
});
