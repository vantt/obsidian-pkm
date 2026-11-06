// Scroll lock for two panes: once locked, scrolling one pane moves the other so the offset between them stays what
// it was at lock time. Holding Alt (or a wheel gesture with Alt) scrolls one pane alone; the new positions become the
// locked offset. No Obsidian import, so it runs in unit tests on plain objects.
//
// Derived from Relative Scroll Lock (https://github.com/v-49/obsidian-relative-scroll-lock), MIT License,
// Copyright (c) 2026 v-49, with the fix sent upstream in v-49/obsidian-relative-scroll-lock#1: the follower is placed
// from the locked offset, not moved by deltas, so the pair realigns after one pane hits its top or bottom.
// Permission notice: see THIRD_PARTY at the end of this file.

/** What the session needs from a scroll container (an HTMLElement in Obsidian). */
export interface ScrollPane extends EventTarget {
  scrollTop: number;
  readonly scrollHeight: number;
  readonly clientHeight: number;
  readonly isConnected: boolean;
  readonly ownerDocument: { defaultView: EventTarget | null };
}

/** A wheel gesture with or without Alt keeps its meaning this long (momentum scrolling sends more scroll events). */
export const GESTURE_MS = 220;

/** Where the follower goes when `source` (pane 0 or 1) is at `top`: offset applied, clamped to the follower's range. */
export function followerTop(sourceIndex: 0 | 1, top: number, offset: number, followerMax: number): number {
  const wanted = sourceIndex === 0 ? top + offset : top - offset;
  return Math.min(Math.max(0, followerMax), Math.max(0, wanted));
}

export class ScrollLockSession {
  /** pane 1 should sit this many pixels below pane 0 */
  offset: number;
  alt = false;
  private readonly positions: number[];
  private readonly gestures = [
    { alt: false, until: 0 },
    { alt: false, until: 0 },
  ];
  private readonly disposers: (() => void)[] = [];
  private propagating = false;
  private disposed = false;

  constructor(
    readonly panes: readonly [ScrollPane, ScrollPane],
    private readonly onAlt: (alt: boolean) => void = () => {},
    private readonly onInvalid: () => void = () => {},
    private readonly now: () => number = Date.now,
  ) {
    this.positions = panes.map((p) => p.scrollTop);
    this.offset = this.positions[1]! - this.positions[0]!;
    panes.forEach((pane, i) => {
      this.listen(pane, 'scroll', () => this.scroll(i as 0 | 1), { passive: true });
      this.listen(
        pane,
        'wheel',
        (e) => {
          const alt = (e as WheelEvent).altKey === true;
          this.gestures[i] = { alt, until: this.now() + GESTURE_MS };
          this.setAlt(alt);
        },
        { passive: true, capture: true },
      );
    });
    const windows = new Set(panes.map((p) => p.ownerDocument.defaultView).filter((w): w is EventTarget => w !== null));
    for (const win of windows) {
      const key = (e: Event) => this.setAlt((e as KeyboardEvent).altKey === true);
      this.listen(win, 'keydown', key, true);
      this.listen(win, 'keyup', key, true);
      this.listen(win, 'blur', () => {
        this.setAlt(false);
        for (const g of this.gestures) g.alt = false;
      });
    }
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    for (const d of this.disposers.splice(0)) d();
  }

  private listen(target: EventTarget, name: string, fn: (e: Event) => void, options?: AddEventListenerOptions | boolean): void {
    target.addEventListener(name, fn, options);
    this.disposers.push(() => target.removeEventListener(name, fn, options));
  }

  private setAlt(value: boolean): void {
    if (this.alt === value) return;
    this.alt = value;
    this.onAlt(value);
  }

  private scroll(index: 0 | 1): void {
    if (this.disposed) return;
    if (this.panes.some((p) => !p.isConnected)) {
      this.onInvalid();
      return;
    }
    const source = this.panes[index];
    const delta = source.scrollTop - this.positions[index]!;
    this.positions[index] = source.scrollTop;
    if (this.propagating || Math.abs(delta) < 0.01) return;
    const g = this.gestures[index]!;
    if (this.alt || (g.alt && this.now() < g.until)) {
      // adjusting one pane: what the user sees now is the new locked offset
      this.offset = this.positions[1]! - this.positions[0]!;
      return;
    }
    const other = (1 - index) as 0 | 1;
    const target = this.panes[other];
    this.propagating = true;
    try {
      target.scrollTop = followerTop(index, source.scrollTop, this.offset, target.scrollHeight - target.clientHeight);
      // recorded before the target's own (async) scroll event, whose echo then has zero delta
      this.positions[other] = target.scrollTop;
    } finally {
      this.propagating = false;
    }
  }
}

export const THIRD_PARTY = `Relative Scroll Lock — MIT License — Copyright (c) 2026 v-49

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the Software without restriction, including without limitation the
rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit
persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the
Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE
WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.`;
