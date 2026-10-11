import { userEvent } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { TemplateNode } from '../../../../src/modules/sync/application/component-model.js';
import { htmlElement } from '../editor-fixtures.js';
import {
  BADGE_TEMPLATE,
  actionButton,
  badgeChange,
  badgeComponent,
  badgeModel,
  clickLayer,
  itemIds,
  layerMeta,
  layerName,
  mountWithLayers,
  recordLayerEvents,
  reveal,
  selectedLayerIds,
  tree,
  treeItem,
  treeItems,
} from './layer-fixtures.js';

const mounted: Element[] = [];

const TEMPLATE_LAYER_IDS = ['neutral:0', 'neutral:0.0', 'neutral:0.1', 'accent:0', 'accent:0.0', 'accent:0.1'];

function layerCount(): string {
  return htmlElement('#layer-count').textContent.trim();
}

beforeEach(() => {
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
  for (const element of mounted.splice(0)) element.remove();
});

describe('<ui-layer-tree> content', () => {
  // covers: FCT-0035
  it('shows the layers of the opened component as a tree labelled after its tag', async () => {
    await mountWithLayers(mounted);

    expect(tree().getAttribute('aria-label')).toBe('Calques de hz-plan-badge');
    expect(tree().classList.contains('ui-tree')).toBe(true);
    expect(treeItems()).toHaveLength(9);
  });

  // covers: FCT-0035
  it('puts the component tag at the root, then one branch per variant instance with its value as meta', async () => {
    await mountWithLayers(mounted);
    const [root] = treeItems();
    const rootId = root?.dataset['id'] ?? '';
    const branches = treeItems().filter((item) => item.parentElement?.closest('[role="treeitem"]') === root);
    const branchIds = branches.map((item) => item.dataset['id'] ?? '');

    expect(layerName(rootId)).toBe('hz-plan-badge');
    expect(root?.getAttribute('aria-label')).toBe('hz-plan-badge');
    expect(branchIds.map((id) => layerName(id))).toEqual(['neutral', 'accent']);
    expect(branchIds.map((id) => layerMeta(id))).toEqual(['neutral', 'accent']);
  });

  // covers: FCT-0035
  it('reproduces the template elements in their order inside each variant branch, with variant-prefixed ids', async () => {
    await mountWithLayers(mounted);

    expect(itemIds().filter((id) => id.includes(':'))).toEqual(TEMPLATE_LAYER_IDS);
    expect(treeItem('accent:0.0').parentElement?.closest('[role="treeitem"]')).toBe(treeItem('accent:0'));
    expect(treeItem('accent:0.1').parentElement?.closest('[role="treeitem"]')).toBe(treeItem('accent:0'));
  });

  // covers: FCT-0035
  it('gives a toggle and an expanded state to the layers that have children, and a spacer to the others', async () => {
    await mountWithLayers(mounted);

    expect(treeItem('accent:0').hasAttribute('aria-expanded')).toBe(true);
    expect(treeItem('accent:0').querySelector(':scope > .ui-tree__row .ui-tree__toggle')).not.toBeNull();
    expect(treeItem('accent:0.0').hasAttribute('aria-expanded')).toBe(false);
    expect(treeItem('accent:0.0').querySelector(':scope > .ui-tree__row .ui-tree__spacer')).not.toBeNull();
  });

  // covers: FCT-0035
  it('gives each layer a hide and a lock button named after it, with the shortcut in their tooltip', async () => {
    await mountWithLayers(mounted);
    const name = layerName('accent:0.0');

    expect(name).not.toBe('');
    expect(actionButton('accent:0.0', 'hide').getAttribute('aria-label')).toBe(`Masquer le calque ${name}`);
    expect(actionButton('accent:0.0', 'hide').dataset['tip']).toBe('Masquer (Ctrl Maj H)');
    expect(actionButton('accent:0.0', 'lock').getAttribute('aria-label')).toBe(`Verrouiller le calque ${name}`);
    expect(actionButton('accent:0.0', 'lock').dataset['tip']).toBe('Verrouiller (Ctrl Maj L)');
    expect(treeItem('accent:0.0').getAttribute('aria-label')).toBe(name);
  });

  // covers: FCT-0035
  it('gives the number of layers in the header, with a non-breaking space', async () => {
    await mountWithLayers(mounted);

    expect(layerCount()).toBe('9\u00a0calques');
  });

  // covers: FCT-0035
  it('says 1 calque in the singular for a component reduced to its root layer', async () => {
    const lone = { ...badgeModel('v1', []), properties: [], variantProperty: null, variants: [] };
    await mountWithLayers(mounted, badgeComponent(lone));

    expect(treeItems()).toHaveLength(1);
    expect(layerCount()).toBe('1\u00a0calque');
  });

  // covers: FCT-0035
  it('rebuilds the layers and the count when a new version of the component arrives', async () => {
    const fake = await mountWithLayers(mounted);
    const longer: readonly TemplateNode[] = [
      ...BADGE_TEMPLATE,
      { kind: 'element', id: '1', tag: 'footer', range: { start: 420, end: 440 }, attributes: [], classes: [], children: [] },
    ];

    fake.sendChange(badgeChange(badgeModel('v2', longer)));

    await expect.poll(() => itemIds().filter((id) => id.includes(':'))).toEqual([
      'neutral:0',
      'neutral:0.0',
      'neutral:0.1',
      'neutral:1',
      'accent:0',
      'accent:0.0',
      'accent:0.1',
      'accent:1',
    ]);
    expect(layerCount()).toBe('11\u00a0calques');
  });
});

describe('<ui-layer-tree> selection', () => {
  // covers: FCT-0036
  it('selects a layer when its row is clicked, and tells which one by layer-select', async () => {
    await mountWithLayers(mounted);
    const events = recordLayerEvents();

    await clickLayer('accent:0.0');

    await expect.poll(() => selectedLayerIds()).toEqual(['accent:0.0']);
    expect(events.filter((event) => event.type === 'layer-select')).toEqual([{ type: 'layer-select', id: 'accent:0.0' }]);
  });

  // covers: FCT-0036
  it('moves the selection from one layer to another, leaving a single selected row', async () => {
    await mountWithLayers(mounted);

    await clickLayer('accent:0.0');
    await clickLayer('neutral:0.1');

    await expect.poll(() => selectedLayerIds()).toEqual(['neutral:0.1']);
  });

  // covers: FCT-0036
  it('tells which layer the pointer is over by layer-hover', async () => {
    await mountWithLayers(mounted);
    const events = recordLayerEvents();
    const item = await reveal('neutral:0.1');
    const name = item.querySelector(':scope > .ui-tree__row .ui-tree__name');
    if (!(name instanceof HTMLElement)) throw new Error('The layer has no name');

    await userEvent.hover(name);

    await expect.poll(() => events.filter((event) => event.type === 'layer-hover' && event.id === 'neutral:0.1')).not.toEqual([]);
    expect(events.filter((event) => event.type === 'layer-select')).toEqual([]);
  });

  // covers: FCT-0036
  it('does not select a layer when its hide or lock button is clicked', async () => {
    await mountWithLayers(mounted);
    const events = recordLayerEvents();
    await reveal('accent:0.0');

    actionButton('accent:0.0', 'hide').click();
    actionButton('accent:0.0', 'lock').click();

    await expect.poll(() => treeItem('accent:0.0').getAttribute('aria-label')).toMatch(/, masqué, verrouillé$/);
    expect(events.filter((event) => event.type === 'layer-select')).toEqual([]);
    expect(selectedLayerIds()).not.toContain('accent:0.0');
  });

  // covers: FCT-0036
  it('keeps the selection on the same element when a new version still has its node', async () => {
    const fake = await mountWithLayers(mounted);
    await clickLayer('accent:0.1');
    await expect.poll(() => selectedLayerIds()).toEqual(['accent:0.1']);
    const longer: readonly TemplateNode[] = [
      ...BADGE_TEMPLATE,
      { kind: 'element', id: '1', tag: 'footer', range: { start: 420, end: 440 }, attributes: [], classes: [], children: [] },
    ];

    fake.sendChange(badgeChange(badgeModel('v2', longer)));

    await expect.poll(() => itemIds()).toContain('accent:1');
    expect(selectedLayerIds()).toEqual(['accent:0.1']);
  });

  // covers: FCT-0036
  it('drops the selection when a new version no longer has the selected node', async () => {
    const fake = await mountWithLayers(mounted);
    await clickLayer('accent:0.1');
    await expect.poll(() => selectedLayerIds()).toEqual(['accent:0.1']);
    const shorter: readonly TemplateNode[] = [
      { kind: 'element', id: '0', tag: 'article', range: { start: 340, end: 400 }, attributes: [], classes: [], children: [] },
    ];

    fake.sendChange(badgeChange(badgeModel('v2', shorter)));

    await expect.poll(() => itemIds()).not.toContain('accent:0.1');
    expect(selectedLayerIds()).not.toContain('accent:0.1');
    expect(selectedLayerIds().length).toBeLessThanOrEqual(1);
  });
});
