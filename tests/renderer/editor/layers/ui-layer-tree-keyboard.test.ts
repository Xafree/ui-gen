import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { press } from '../editor-fixtures.js';
import {
  expand,
  focusedLayerId,
  mountWithLayers,
  recordLayerEvents,
  reveal,
  selectedLayerIds,
  tabStops,
  treeItem,
  treeItems,
} from './layer-fixtures.js';

const mounted: Element[] = [];

interface Branches {
  readonly root: string;
  readonly neutral: string;
  readonly accent: string;
}

function branches(): Branches {
  const [root] = treeItems();
  const children = treeItems().filter((item) => item.parentElement?.closest('[role="treeitem"]') === root);
  return {
    root: root?.dataset['id'] ?? '',
    neutral: children[0]?.dataset['id'] ?? '',
    accent: children[1]?.dataset['id'] ?? '',
  };
}

async function collapse(id: string): Promise<void> {
  const item = treeItem(id);
  if (item.getAttribute('aria-expanded') !== 'true') return;
  const toggle = item.querySelector<HTMLElement>(':scope > .ui-tree__row .ui-tree__toggle');
  if (toggle === null) throw new Error(`The layer ${id} has no toggle`);
  toggle.click();
  await expect.poll(() => treeItem(id).getAttribute('aria-expanded')).toBe('false');
}

async function focusLayer(id: string): Promise<void> {
  (await reveal(id)).focus();
  await expect.poll(() => focusedLayerId()).toBe(id);
}

async function showOnlyAccentBranch(): Promise<Branches> {
  const ids = branches();
  await reveal('accent:0.1');
  await collapse(ids.neutral);
  return ids;
}

async function walk(key: string, times: number): Promise<(string | null)[]> {
  const seen: (string | null)[] = [];
  for (let step = 0; step < times; step += 1) {
    await press(key);
    seen.push(focusedLayerId());
  }
  return seen;
}

beforeEach(() => {
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
  for (const element of mounted.splice(0)) element.remove();
});

describe('<ui-layer-tree> keyboard', () => {
  // covers: FCT-0036
  it('keeps a single tab stop in the tree', async () => {
    await mountWithLayers(mounted);

    expect(tabStops()).toHaveLength(1);
  });

  // covers: FCT-0036
  it('moves to the next and previous visible layer with Down and Up, and stops at the ends', async () => {
    await mountWithLayers(mounted);
    const ids = await showOnlyAccentBranch();
    await focusLayer(ids.root);

    expect(await walk('{ArrowDown}', 6)).toEqual([ids.neutral, ids.accent, 'accent:0', 'accent:0.0', 'accent:0.1', 'accent:0.1']);
    expect(await walk('{ArrowUp}', 2)).toEqual(['accent:0.0', 'accent:0']);
  });

  // covers: FCT-0036
  it('moves the tab stop along with the focus', async () => {
    await mountWithLayers(mounted);
    const ids = await showOnlyAccentBranch();
    await focusLayer(ids.root);

    await walk('{ArrowDown}', 3);

    expect(focusedLayerId()).toBe('accent:0');
    expect(tabStops()).toEqual(['accent:0']);
  });

  // covers: FCT-0036
  it('goes to the first and the last visible layer with Home and End', async () => {
    await mountWithLayers(mounted);
    const ids = await showOnlyAccentBranch();
    await focusLayer('accent:0.0');

    await press('{End}');
    expect(focusedLayerId()).toBe('accent:0.1');
    await press('{Home}');
    expect(focusedLayerId()).toBe(ids.root);
  });

  // covers: FCT-0036
  it('opens a closed layer with Right, then goes to its first child with Right again', async () => {
    await mountWithLayers(mounted);
    await reveal('accent:0');
    await collapse('accent:0');
    await focusLayer('accent:0');

    await press('{ArrowRight}');
    expect(treeItem('accent:0').getAttribute('aria-expanded')).toBe('true');
    expect(focusedLayerId()).toBe('accent:0');

    await press('{ArrowRight}');
    expect(focusedLayerId()).toBe('accent:0.0');
  });

  // covers: FCT-0036
  it('goes to the parent with Left, closes an open layer with Left, then goes to its parent', async () => {
    await mountWithLayers(mounted);
    const ids = branches();
    await focusLayer('accent:0.0');

    await press('{ArrowLeft}');
    expect(focusedLayerId()).toBe('accent:0');
    expect(treeItem('accent:0').getAttribute('aria-expanded')).toBe('true');

    await press('{ArrowLeft}');
    expect(treeItem('accent:0').getAttribute('aria-expanded')).toBe('false');
    expect(focusedLayerId()).toBe('accent:0');

    await press('{ArrowLeft}');
    expect(focusedLayerId()).toBe(ids.accent);
  });

  // covers: FCT-0036
  it('skips the layers of a closed branch', async () => {
    await mountWithLayers(mounted);
    const ids = branches();
    await expand(ids.root);
    await collapse(ids.neutral);
    await focusLayer(ids.neutral);

    await press('{ArrowDown}');

    expect(focusedLayerId()).toBe(ids.accent);
  });

  // covers: FCT-0036
  it('selects the focused layer with Enter, and with Space', async () => {
    await mountWithLayers(mounted);
    const events = recordLayerEvents();
    await focusLayer('neutral:0.0');

    await press('{Enter}');
    await expect.poll(() => selectedLayerIds()).toEqual(['neutral:0.0']);

    await focusLayer('accent:0.1');
    await press(' ');
    await expect.poll(() => selectedLayerIds()).toEqual(['accent:0.1']);

    expect(events.filter((event) => event.type === 'layer-select')).toEqual([
      { type: 'layer-select', id: 'neutral:0.0' },
      { type: 'layer-select', id: 'accent:0.1' },
    ]);
  });

  // covers: FCT-0036
  it('does not select a layer only by moving the focus onto it', async () => {
    await mountWithLayers(mounted);
    const events = recordLayerEvents();
    const ids = await showOnlyAccentBranch();
    await focusLayer(ids.root);

    await walk('{ArrowDown}', 3);

    expect(events.filter((event) => event.type === 'layer-select')).toEqual([]);
    expect(selectedLayerIds()).not.toContain('accent:0');
  });
});
