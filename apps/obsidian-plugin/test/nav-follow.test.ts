import { describe, expect, it } from 'vitest';
import { expandParents, FROM_TREE_MS, openedFromTree } from '../src/nav-follow';

interface Node {
  path: string;
  parent: Node | null;
  isRoot: () => boolean;
}
const node = (path: string, parent: Node | null): Node => ({ path, parent, isRoot: () => parent === null });

describe('nav follow', () => {
  it('counts an open right after a click in the tree as from the tree', () => {
    const now = 100_000;
    expect(openedFromTree(now, now - FROM_TREE_MS + 1)).toBe(true);
    expect(openedFromTree(now, now - FROM_TREE_MS)).toBe(false);
    expect(openedFromTree(now, 0)).toBe(false);
  });

  it('opens collapsed folders above the file, not the root, not open ones', async () => {
    const root = node('/', null);
    const a = node('A', root);
    const b = node('A/B', a);
    const file = node('A/B/note.md', b);
    const opened: string[] = [];
    const item = (path: string, collapsed: boolean) => ({
      collapsed,
      setCollapsed: (c: boolean) => {
        if (!c) opened.push(path);
      },
    });
    const items = { '/': item('/', true), A: item('A', false), 'A/B': item('A/B', true) };
    // a TAbstractFile only needs path, parent and isRoot here
    await expandParents(file as never, items);
    expect(opened).toEqual(['A/B']);
  });
});
