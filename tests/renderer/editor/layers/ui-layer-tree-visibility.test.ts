import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { press } from '../editor-fixtures.js';
import {
  actionButton,
  clickLayer,
  layerName,
  mountWithLayers,
  ownRow,
  recordNotices,
  reveal,
  selectedLayerIds,
  treeItem,
  treeItems,
  type SeenNotice,
} from './layer-fixtures.js';

const mounted: Element[] = [];
let notices: ReturnType<typeof recordNotices>;

function seen(): SeenNotice[] {
  return notices.seen();
}

function isPinned(id: string): boolean {
  return ownRow(id).querySelector('.ui-tree__actions')?.classList.contains('is-pinned') ?? false;
}

beforeEach(() => {
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
  notices = recordNotices();
});

afterEach(() => {
  notices.stop();
  vi.restoreAllMocks();
  for (const element of mounted.splice(0)) element.remove();
});

describe('<ui-layer-tree> hide', () => {
  // covers: FCT-0037
  it('hides a layer from its button, says so, and offers to show it again', async () => {
    await mountWithLayers(mounted);
    await reveal('accent:0.0');
    const name = layerName('accent:0.0');

    actionButton('accent:0.0', 'hide').click();

    await expect.poll(() => seen()).toContainEqual({
      title: `${name} masqué`,
      message: 'Masqué dans le canvas seulement : le code ne change pas.',
    });
    expect(treeItem('accent:0.0').getAttribute('aria-label')).toBe(`${name}, masqué`);
    expect(ownRow('accent:0.0').classList.contains('is-muted')).toBe(true);
    expect(isPinned('accent:0.0')).toBe(true);
    expect(actionButton('accent:0.0', 'hide').getAttribute('aria-label')).toBe(`Afficher le calque ${name}`);
    expect(actionButton('accent:0.0', 'hide').dataset['tip']).toBe('Afficher (Ctrl Maj H)');
  });

  // covers: FCT-0037
  it('shows a hidden layer again from the same button', async () => {
    await mountWithLayers(mounted);
    await reveal('accent:0.0');
    const name = layerName('accent:0.0');
    actionButton('accent:0.0', 'hide').click();
    await expect.poll(() => treeItem('accent:0.0').getAttribute('aria-label')).toBe(`${name}, masqué`);

    actionButton('accent:0.0', 'hide').click();

    await expect.poll(() => seen()).toContainEqual({ title: `${name} affiché`, message: 'Calque de nouveau visible.' });
    expect(treeItem('accent:0.0').getAttribute('aria-label')).toBe(name);
    expect(ownRow('accent:0.0').classList.contains('is-muted')).toBe(false);
    expect(isPinned('accent:0.0')).toBe(false);
    expect(actionButton('accent:0.0', 'hide').getAttribute('aria-label')).toBe(`Masquer le calque ${name}`);
  });

  // covers: FCT-0037
  it('hides only the layer of that variant instance', async () => {
    await mountWithLayers(mounted);
    await reveal('accent:0.0');
    await reveal('neutral:0.0');

    actionButton('accent:0.0', 'hide').click();

    await expect.poll(() => ownRow('accent:0.0').classList.contains('is-muted')).toBe(true);
    expect(ownRow('neutral:0.0').classList.contains('is-muted')).toBe(false);
    expect(treeItem('neutral:0.0').getAttribute('aria-label')).toBe(layerName('neutral:0.0'));
  });
});

describe('<ui-layer-tree> lock', () => {
  // covers: FCT-0037
  it('locks a layer from its button, says a click on the canvas now selects its parent', async () => {
    await mountWithLayers(mounted);
    await reveal('neutral:0.1');
    const name = layerName('neutral:0.1');

    actionButton('neutral:0.1', 'lock').click();

    await expect.poll(() => seen()).toContainEqual({
      title: `${name} verrouillé`,
      message: 'Un clic sur le canvas sélectionne désormais son parent.',
    });
    expect(treeItem('neutral:0.1').getAttribute('aria-label')).toBe(`${name}, verrouillé`);
    expect(isPinned('neutral:0.1')).toBe(true);
    expect(ownRow('neutral:0.1').classList.contains('is-muted')).toBe(false);
    expect(actionButton('neutral:0.1', 'lock').getAttribute('aria-label')).toBe(`Déverrouiller le calque ${name}`);
    expect(actionButton('neutral:0.1', 'lock').dataset['tip']).toBe('Déverrouiller (Ctrl Maj L)');
  });

  // covers: FCT-0037
  it('unlocks a locked layer from the same button', async () => {
    await mountWithLayers(mounted);
    await reveal('neutral:0.1');
    const name = layerName('neutral:0.1');
    actionButton('neutral:0.1', 'lock').click();
    await expect.poll(() => treeItem('neutral:0.1').getAttribute('aria-label')).toBe(`${name}, verrouillé`);

    actionButton('neutral:0.1', 'lock').click();

    await expect.poll(() => seen()).toContainEqual({
      title: `${name} déverrouillé`,
      message: 'Le calque se sélectionne de nouveau sur le canvas.',
    });
    expect(treeItem('neutral:0.1').getAttribute('aria-label')).toBe(name);
    expect(actionButton('neutral:0.1', 'lock').getAttribute('aria-label')).toBe(`Verrouiller le calque ${name}`);
  });

  // covers: FCT-0037
  it('still selects a locked layer from the tree', async () => {
    await mountWithLayers(mounted);
    await reveal('neutral:0.1');
    actionButton('neutral:0.1', 'lock').click();
    await expect.poll(() => isPinned('neutral:0.1')).toBe(true);

    await clickLayer('neutral:0.1');

    await expect.poll(() => selectedLayerIds()).toEqual(['neutral:0.1']);
  });
});

describe('<ui-layer-tree> shortcuts', () => {
  // covers: FCT-0037
  it('hides then shows the selected layer with Ctrl Maj H', async () => {
    await mountWithLayers(mounted);
    await clickLayer('accent:0.1');
    await expect.poll(() => selectedLayerIds()).toEqual(['accent:0.1']);
    const name = layerName('accent:0.1');

    await press('{Control>}{Shift>}H{/Shift}{/Control}');
    await expect.poll(() => seen()).toContainEqual({
      title: `${name} masqué`,
      message: 'Masqué dans le canvas seulement : le code ne change pas.',
    });
    expect(treeItem('accent:0.1').getAttribute('aria-label')).toBe(`${name}, masqué`);

    await press('{Control>}{Shift>}H{/Shift}{/Control}');
    await expect.poll(() => seen()).toContainEqual({ title: `${name} affiché`, message: 'Calque de nouveau visible.' });
    expect(treeItem('accent:0.1').getAttribute('aria-label')).toBe(name);
  });

  // covers: FCT-0037
  it('locks then unlocks the selected layer with Ctrl Maj L', async () => {
    await mountWithLayers(mounted);
    await clickLayer('neutral:0');
    await expect.poll(() => selectedLayerIds()).toEqual(['neutral:0']);
    const name = layerName('neutral:0');

    await press('{Control>}{Shift>}L{/Shift}{/Control}');
    await expect.poll(() => seen()).toContainEqual({
      title: `${name} verrouillé`,
      message: 'Un clic sur le canvas sélectionne désormais son parent.',
    });

    await press('{Control>}{Shift>}L{/Shift}{/Control}');
    await expect.poll(() => seen()).toContainEqual({
      title: `${name} déverrouillé`,
      message: 'Le calque se sélectionne de nouveau sur le canvas.',
    });
    expect(treeItem('neutral:0').getAttribute('aria-label')).toBe(name);
  });

  // covers: FCT-0037
  it('neither hides nor locks the root layer with the shortcuts', async () => {
    await mountWithLayers(mounted);
    const rootId = treeItems()[0]?.dataset['id'] ?? '';
    await clickLayer(rootId);
    await expect.poll(() => selectedLayerIds()).toEqual([rootId]);

    await press('{Control>}{Shift>}H{/Shift}{/Control}');
    await press('{Control>}{Shift>}L{/Shift}{/Control}');

    expect(seen()).toEqual([]);
    expect(treeItem(rootId).getAttribute('aria-label')).toBe('hz-plan-badge');
    expect(ownRow(rootId).classList.contains('is-muted')).toBe(false);
  });
});
