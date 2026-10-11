import { userEvent } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { deepQueryAll } from '../../home/shadow-dom.js';
import { htmlElement } from '../editor-fixtures.js';
import { clickLayer, mountWithLayers, reveal, selectedLayerIds } from '../layers/layer-fixtures.js';
import { SELECTION_HARNESS_URL } from './selection-harness.js';

vi.mock('../../../../src/modules/canvas/application/harness-url.js', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../../../src/modules/canvas/application/harness-url.js')>()),
  buildHarnessUrl: () => SELECTION_HARNESS_URL,
}));

const mounted: Element[] = [];
const FRAME_WIDTH = 1024;

interface PageBox {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}

function frameZoom(): number {
  return htmlElement('ui-canvas-frame').getBoundingClientRect().width / FRAME_WIDTH;
}

function boxOf(selector: string): PageBox {
  const { left, top, width, height } = htmlElement(selector).getBoundingClientRect();
  return { left, top, width, height };
}

function expectedBox(x: number, y: number, width: number, height: number): PageBox {
  const frame = htmlElement('ui-canvas-frame').getBoundingClientRect();
  const zoom = frameZoom();
  return { left: frame.left + x * zoom, top: frame.top + y * zoom, width: width * zoom, height: height * zoom };
}

function expectNear(actual: PageBox, expected: PageBox): void {
  expect(actual.left).toBeCloseTo(expected.left, 0);
  expect(actual.top).toBeCloseTo(expected.top, 0);
  expect(actual.width).toBeCloseTo(expected.width, 0);
  expect(actual.height).toBeCloseTo(expected.height, 0);
}

function isHidden(selector: string): boolean {
  return htmlElement(selector).hasAttribute('hidden');
}

beforeEach(() => {
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
  for (const element of mounted.splice(0)) element.remove();
});

describe('the selection overlay', () => {
  // covers: FCT-0036
  it('shows nothing while no layer is selected', async () => {
    await mountWithLayers(mounted);

    await expect.poll(() => isHidden('#sel-box')).toBe(true);
    expect(isHidden('#hover-box')).toBe(true);
  });

  // covers: FCT-0036
  it('frames the element of the selected layer from its rectangle and the viewport transform, with its CSS size', async () => {
    await mountWithLayers(mounted);

    await clickLayer('accent:0.0');

    await expect.poll(() => isHidden('#sel-box')).toBe(false);
    await expect.poll(() => htmlElement('#sel-size').textContent.trim()).toBe('100 × 22');
    await expect.poll(() => boxOf('#sel-box').width).toBeCloseTo(100 * frameZoom(), 0);
    expectNear(boxOf('#sel-box'), expectedBox(320, 40, 100, 22));
  });

  // covers: FCT-0036
  it('moves the frame along with the selection, and hides it for a layer that is not drawn', async () => {
    await mountWithLayers(mounted);
    await clickLayer('accent:0.0');
    await expect.poll(() => isHidden('#sel-box')).toBe(false);

    await clickLayer('neutral:0.1');
    await expect.poll(() => htmlElement('#sel-size').textContent.trim()).toBe('80 × 14');
    expectNear(boxOf('#sel-box'), expectedBox(30, 80, 80, 14));

    await clickLayer('accent');
    await expect.poll(() => isHidden('#sel-box')).toBe(true);
  });

  // covers: FCT-0036
  it('outlines the element under the pointer in the tree, but not the selected one', async () => {
    await mountWithLayers(mounted);
    await clickLayer('accent:0.0');
    const item = await reveal('neutral:0.1');
    const name = item.querySelector(':scope > .ui-tree__row .ui-tree__name');
    if (!(name instanceof HTMLElement)) throw new Error('The layer has no name');

    await userEvent.hover(name);

    await expect.poll(() => isHidden('#hover-box')).toBe(false);
    expectNear(boxOf('#hover-box'), expectedBox(30, 80, 80, 14));

    await userEvent.unhover(name);
    await expect.poll(() => isHidden('#hover-box')).toBe(true);

    const selected = await reveal('accent:0.0');
    const selectedName = selected.querySelector(':scope > .ui-tree__row .ui-tree__name');
    if (!(selectedName instanceof HTMLElement)) throw new Error('The layer has no name');
    await userEvent.hover(selectedName);
    expect(isHidden('#hover-box')).toBe(true);
  });

  // covers: FCT-0036
  it('selects the layer the harness finds under a click on the canvas', async () => {
    await mountWithLayers(mounted);
    await expect.poll(() => htmlElement('ui-canvas-frame').getBoundingClientRect().width).toBeGreaterThan(0);

    const viewport = deepQueryAll('#viewport')[0];
    if (!(viewport instanceof HTMLElement)) throw new Error('The canvas has no viewport');
    await userEvent.click(viewport);

    await expect.poll(() => selectedLayerIds()).toEqual(['accent:0.1']);
    await expect.poll(() => htmlElement('#sel-size').textContent.trim()).toBe('80 × 14');
  });

  // covers: FCT-0036
  it('draws the distance guides between the selected element and its parent', async () => {
    await mountWithLayers(mounted);

    await clickLayer('accent:0.1');

    await expect.poll(() => deepQueryAll('#guides .ed-guide').length).toBeGreaterThan(0);
    const labels = deepQueryAll('#guides .ed-guide__label').map((label) => label.textContent.trim());
    expect(labels.toSorted()).toEqual(['18', '20', '36']);
  });
});
