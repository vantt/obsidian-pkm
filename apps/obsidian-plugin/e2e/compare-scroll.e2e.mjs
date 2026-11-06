// Side-by-side reading (src/compare-scroll/), in a real Obsidian on the e2e vault's long and short notes.
import assert from 'node:assert/strict';

export const name = 'compare-scroll: lock button, follow, Alt, edges, auto-unlock';
export const timeoutMs = 240_000;

/** Runs inside Obsidian. Self-contained: no imports, no outer variables. */
export async function run() {
  const ws = app.workspace;
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const out = {};
  const file = (p) => app.vault.getFileByPath(p);
  const long = file('Đọc đối chiếu/Bản dài.md');
  const short = file('Đọc đối chiếu/Bản ngắn.md');
  const third = file('Đọc đối chiếu/Bản thứ ba.md');
  if (!long || !short || !third) return { error: 'e2e notes missing' };
  // start from one empty tab in the main area
  ws.iterateRootLeaves((l) => l.detach());
  await wait(300);
  const buttons = () => [...document.querySelectorAll('.pkm-compare-lock')].filter((e) => e.isConnected && e.offsetParent && !e.classList.contains('is-hidden'));
  const status = () => {
    const s = document.querySelector('.pkm-compare-status');
    return !s || s.classList.contains('is-hidden') ? null : s.textContent;
  };
  const scroller = (leaf) => leaf.view.contentEl.querySelector(leaf.view.getMode() === 'preview' ? '.markdown-preview-view' : '.cm-scroller');
  const lastNotice = () => [...document.querySelectorAll('.notice')].map((n) => n.textContent).at(-1) ?? null;
  out.commands = ['pkm:compare-open', 'pkm:compare-lock'].map((id) => Boolean(app.commands.commands[id]));

  for (const mode of ['preview', 'source']) {
    const r = {};
    const left = ws.getLeaf('tab');
    await left.openFile(long, { state: { mode } });
    await wait(800);
    r.buttonsOnePane = buttons().length;
    const right = ws.getLeaf('split', 'vertical');
    await right.openFile(short, { state: { mode } });
    await wait(1500);
    r.buttonsPair = buttons().length;
    const L = scroller(left);
    const R = scroller(right);

    // follow, both ways, at a 300 px offset
    L.scrollTop = 2000;
    R.scrollTop = 2300;
    await wait(1500);
    buttons()[0].click();
    await wait(300);
    r.statusLocked = status();
    r.iconLocked = Boolean(buttons()[0]?.querySelector('svg.lucide-lock'));
    const step = async (fn) => {
      const a = [L.scrollTop, R.scrollTop];
      fn();
      await wait(1200);
      return [Math.round(L.scrollTop - a[0]), Math.round(R.scrollTop - a[1])];
    };
    r.leftDown400 = await step(() => (L.scrollTop += 400));
    r.rightUp300 = await step(() => (R.scrollTop -= 300));

    // Alt: one pane alone, then the new offset holds
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Alt', altKey: true }));
    r.statusAlt = status();
    r.altLeft300 = await step(() => (L.scrollTop += 300));
    window.dispatchEvent(new KeyboardEvent('keyup', { key: 'Alt', altKey: false }));
    r.statusAfterAlt = status();
    r.left200 = await step(() => (L.scrollTop += 200));

    // edge: the short note hits its bottom, then back; the offset must survive
    const offset = R.scrollTop - L.scrollTop;
    const rMax = R.scrollHeight - R.clientHeight;
    for (let k = 0; k < 40 && L.scrollTop + offset < rMax + 3000; k++) {
      L.scrollTop += 2000;
      await wait(200);
    }
    await wait(1000);
    r.shortAtBottom = R.scrollHeight - R.clientHeight - R.scrollTop < 2;
    for (let k = 0; k < 60 && L.scrollTop > 2500; k++) {
      L.scrollTop -= 2000;
      await wait(200);
    }
    await wait(1200);
    r.offsetBefore = Math.round(offset);
    r.offsetAfter = Math.round(R.scrollTop - L.scrollTop);

    // another note in one pane unlocks
    await right.openFile(third);
    await wait(800);
    r.statusAfterFileChange = status();
    r.noticeAfterFileChange = lastNotice();

    // lock again, close one pane: unlocks, button hidden
    buttons()[0]?.click();
    await wait(300);
    r.relocked = status();
    right.detach();
    await wait(800);
    r.statusAfterClose = status();
    r.buttonsAfterClose = buttons().length;
    left.detach();
    await wait(300);
    out[mode] = r;
  }
  return out;
}

// Reading view renders long notes lazily and corrects scrollTop as sections get measured, so a step of +200 may land
// as -376 on both panes. What the lock promises is that both panes move by the same amount; check that, not pixels.
const together = (d, m) => assert.ok(d[0] !== 0 && d[0] === d[1], `${m}: both panes moved together, got ${JSON.stringify(d)}`);
const alone = (d, m) => assert.ok(d[0] !== 0 && d[1] === 0, `${m}: only the scrolled pane moved, got ${JSON.stringify(d)}`);

export function check(out) {
  assert.ok(!out.error, out.error);
  assert.deepEqual(out.commands, [true, true], 'both commands registered');
  for (const mode of ['preview', 'source']) {
    const r = out[mode];
    const m = `${mode}: `;
    assert.equal(r.buttonsOnePane, 0, `${m}no lock button with one pane`);
    assert.equal(r.buttonsPair, 2, `${m}lock button on both panes of a pair`);
    assert.equal(r.statusLocked, 'Cuộn chung: khoá', `${m}status after lock`);
    assert.ok(r.iconLocked, `${m}closed-lock icon`);
    together(r.leftDown400, `${m}left scroll moves right`);
    together(r.rightUp300, `${m}right scroll moves left`);
    assert.equal(r.statusAlt, 'Cuộn chung: tạm mở (Alt)', `${m}status while Alt`);
    alone(r.altLeft300, `${m}Alt scrolls one pane alone`);
    assert.equal(r.statusAfterAlt, 'Cuộn chung: khoá', `${m}status after Alt`);
    together(r.left200, `${m}locked again after Alt`);
    assert.equal(r.offsetAfter, r.offsetBefore, `${m}offset survives the short note's bottom`);
    assert.equal(r.statusAfterFileChange, null, `${m}file change unlocks`);
    assert.match(r.noticeAfterFileChange ?? '', /mở khoá/, `${m}notice on unlock`);
    assert.equal(r.relocked, 'Cuộn chung: khoá', `${m}relock`);
    assert.equal(r.statusAfterClose, null, `${m}closing a pane unlocks`);
    assert.equal(r.buttonsAfterClose, 0, `${m}no button after close`);
  }
  // reading view settles; Live Preview measures lines lazily, so only require the edge was reached there
  assert.ok(out.preview.shortAtBottom, 'preview: short note reached its bottom during the edge test');
}
