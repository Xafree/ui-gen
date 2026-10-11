import { describe, expect, it } from 'vitest';
import { layerIdForNode, nodeIdAtLine, nodeRange, touchesLine } from '../../../../../src/modules/editor/domain/code/code-targets.js';
import type { LayerNode } from '../../../../../src/modules/editor/domain/layers/layer-tree.js';
import type { ComponentModel, TemplateNode } from '../../../../../src/modules/sync/application/component-model.js';

const TEMPLATE: readonly TemplateNode[] = [
  {
    kind: 'element',
    id: '0',
    tag: 'div',
    range: { start: 10, end: 60 },
    attributes: [],
    classes: [],
    children: [
      { kind: 'element', id: '0.0', tag: 'p', range: { start: 20, end: 40 }, attributes: [], classes: [], children: [] },
      { kind: 'text', id: '0.1', range: { start: 42, end: 45 }, text: 'ab' },
    ],
  },
];

function modelWith(variants: readonly string[]): ComponentModel {
  return {
    tag: 'hz-x',
    className: 'HzX',
    version: 'v1',
    template: TEMPLATE,
    properties: [],
    variantProperty: variants.length === 0 ? null : 'variant',
    variants,
    styles: [],
    diagnostics: [],
  };
}

function layer(id: string, nodeId: string | null, kind: LayerNode['kind']): LayerNode {
  return { id, nodeId, name: id, kind, meta: null, children: [] };
}

describe('code line targets', () => {
  it('finds the range of a node and nothing for an unknown id', () => {
    expect(nodeRange(TEMPLATE, '0.0')).toEqual({ start: 20, end: 40 });
    expect(nodeRange(TEMPLATE, 'nope')).toBeNull();
  });

  it('maps a line to the node opening on it, else to the deepest node containing it', () => {
    expect(nodeIdAtLine(TEMPLATE, { start: 10, end: 19 })).toBe('0');
    expect(nodeIdAtLine(TEMPLATE, { start: 20, end: 30 })).toBe('0.0');
    expect(nodeIdAtLine(TEMPLATE, { start: 31, end: 39 })).toBe('0.0');
    expect(nodeIdAtLine(TEMPLATE, { start: 41, end: 59 })).toBe('0');
    expect(nodeIdAtLine(TEMPLATE, { start: 0, end: 9 })).toBeNull();
    expect(nodeIdAtLine(TEMPLATE, { start: 61, end: 70 })).toBeNull();
  });

  it('touches the lines a range covers, end excluded', () => {
    expect(touchesLine({ start: 10, end: 20 }, { start: 20, end: 30 })).toBe(false);
    expect(touchesLine({ start: 10, end: 21 }, { start: 20, end: 30 })).toBe(true);
  });

  it('picks the variant of the selected layer, else the first variant', () => {
    const model = modelWith(['starter', 'pro']);

    expect(layerIdForNode(model, layer('pro:0', '0', 'container'), '0.0')).toBe('pro:0.0');
    expect(layerIdForNode(model, layer('pro', null, 'variant'), '0')).toBe('pro:0');
    expect(layerIdForNode(model, layer('hz-x', null, 'set'), '0')).toBe('starter:0');
    expect(layerIdForNode(model, undefined, '0')).toBe('starter:0');
    expect(layerIdForNode(modelWith([]), undefined, '0.0')).toBe('0.0');
  });
});
