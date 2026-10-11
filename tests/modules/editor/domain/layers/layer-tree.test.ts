import { describe, expect, it } from 'vitest';
import { buildLayerTree, countLayers, type LayerNode } from '../../../../../src/modules/editor/domain/layers/layer-tree.js';
import type { ComponentModel, TemplateNode } from '../../../../../src/modules/sync/application/component-model.js';

const RANGE = { start: 0, end: 1 };

function element(id: string, tag: string, children: readonly TemplateNode[] = []): TemplateNode {
  return { kind: 'element', id, tag, range: RANGE, attributes: [], classes: [], children };
}

function text(id: string, value: string): TemplateNode {
  return { kind: 'text', id, range: RANGE, text: value };
}

function binding(id: string, property: string): TemplateNode {
  return { kind: 'binding', id, range: RANGE, property };
}

function conditional(id: string, whenTrue: readonly TemplateNode[], whenFalse: readonly TemplateNode[]): TemplateNode {
  return { kind: 'conditional', id, range: RANGE, condition: 'this.featured', whenTrue, whenFalse };
}

function opaque(id: string): TemplateNode {
  return { kind: 'opaque', id, range: RANGE, source: '${this.items.map((item) => html`<li>${item}</li>`)}' };
}

function model(template: readonly TemplateNode[], variants: readonly string[] = []): ComponentModel {
  return {
    tag: 'hz-pricing-card',
    className: 'HzPricingCard',
    version: 'v1',
    template,
    properties: [],
    variantProperty: variants.length === 0 ? null : 'variant',
    variants,
    styles: [],
    diagnostics: [],
  };
}

const CARD_TEMPLATE: readonly TemplateNode[] = [
  element('0', 'article', [
    element('0.0', 'header', [
      element('0.0.0', 'h3', [text('0.0.0.0', 'Starter')]),
      element('0.0.1', 'p', [binding('0.0.1.0', 'description')]),
    ]),
    element('0.1', 'ul', [element('0.1.0', 'li', [text('0.1.0.0', '1 projet')]), element('0.1.1', 'li', [text('0.1.1.0', '5 composants')])]),
    element('0.2', 'hz-button', [text('0.2.0', 'Commencer')]),
  ]),
];

const CARD_ELEMENT_IDS = ['0', '0.0', '0.0.0', '0.0.1', '0.1', '0.1.0', '0.1.1', '0.2'];

function flatten(nodes: readonly LayerNode[]): LayerNode[] {
  return nodes.flatMap((node) => [node, ...flatten(node.children)]);
}

function onlyRoot(tree: readonly LayerNode[]): LayerNode {
  expect(tree).toHaveLength(1);
  const root = tree[0];
  if (root === undefined) throw new Error('The layer tree has no root');
  return root;
}

function variantBranches(tree: readonly LayerNode[]): LayerNode[] {
  return onlyRoot(tree).children.filter((child) => child.kind === 'variant');
}

function branchNamed(tree: readonly LayerNode[], value: string): LayerNode {
  const branch = variantBranches(tree).find((candidate) => candidate.name === value);
  if (branch === undefined) throw new Error(`No variant branch named ${value}`);
  return branch;
}

function elementNodeIdsIn(nodes: readonly LayerNode[], elementIds: readonly string[]): (string | null)[] {
  return flatten(nodes)
    .map((node) => node.nodeId)
    .filter((nodeId) => nodeId !== null && elementIds.includes(nodeId));
}

function layerWithNodeId(nodes: readonly LayerNode[], nodeId: string): LayerNode {
  const layer = flatten(nodes).find((candidate) => candidate.nodeId === nodeId);
  if (layer === undefined) throw new Error(`No layer for node ${nodeId}`);
  return layer;
}

describe('buildLayerTree', () => {
  // covers: FCT-0035
  it('has a single root named by the component tag, of kind set, tied to no template node', () => {
    const root = onlyRoot(buildLayerTree(model(CARD_TEMPLATE, ['starter', 'pro', 'equipe'])));

    expect(root.name).toBe('hz-pricing-card');
    expect(root.kind).toBe('set');
    expect(root.nodeId).toBeNull();
  });

  // covers: FCT-0035
  it('makes one branch per variant instance, in the order of the variants, named by the value with the value as meta', () => {
    const tree = buildLayerTree(model(CARD_TEMPLATE, ['starter', 'pro', 'equipe']));

    expect(variantBranches(tree).map(({ name, meta }) => ({ name, meta }))).toEqual([
      { name: 'starter', meta: 'starter' },
      { name: 'pro', meta: 'pro' },
      { name: 'equipe', meta: 'equipe' },
    ]);
    expect(onlyRoot(tree).children).toEqual(variantBranches(tree));
  });

  // covers: FCT-0035
  it('reproduces the template elements in their order inside each variant branch', () => {
    const tree = buildLayerTree(model(CARD_TEMPLATE, ['starter', 'pro']));

    for (const value of ['starter', 'pro']) {
      expect(elementNodeIdsIn(branchNamed(tree, value).children, CARD_ELEMENT_IDS)).toEqual(CARD_ELEMENT_IDS);
    }
  });

  // covers: FCT-0035
  it('keeps the nesting of the template: a child element sits under the layer of its parent element', () => {
    const tree = buildLayerTree(model(CARD_TEMPLATE, ['pro']));
    const branch = branchNamed(tree, 'pro');
    const list = layerWithNodeId(branch.children, '0.1');

    expect(elementNodeIdsIn(list.children, CARD_ELEMENT_IDS)).toEqual(['0.1.0', '0.1.1']);
    expect(elementNodeIdsIn(layerWithNodeId(branch.children, '0').children, ['0.0', '0.1', '0.2'])).toEqual(['0.0', '0.1', '0.2']);
    expect(branch.children.map((child) => child.nodeId)).toEqual(['0']);
  });

  // covers: FCT-0035
  it('prefixes the layer ids of a variant branch with the variant value, so each instance has its own layers', () => {
    const tree = buildLayerTree(model(CARD_TEMPLATE, ['starter', 'pro']));

    expect(layerWithNodeId(branchNamed(tree, 'starter').children, '0.0.0').id).toBe('starter:0.0.0');
    expect(layerWithNodeId(branchNamed(tree, 'pro').children, '0.0.0').id).toBe('pro:0.0.0');
    expect(layerWithNodeId(branchNamed(tree, 'pro').children, '0.1.1').id).toBe('pro:0.1.1');
    const ids = flatten(tree).map((layer) => layer.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  // covers: FCT-0035
  it('gives the template layers no meta', () => {
    const tree = buildLayerTree(model(CARD_TEMPLATE, ['pro']));

    for (const nodeId of CARD_ELEMENT_IDS) {
      expect(layerWithNodeId(tree, nodeId).meta).toBeNull();
    }
  });

  // covers: FCT-0035
  it('tells a component instance and a container apart', () => {
    const tree = buildLayerTree(model(CARD_TEMPLATE, ['pro']));

    expect(layerWithNodeId(tree, '0.2').kind).toBe('instance');
    expect(layerWithNodeId(tree, '0.1').kind).toBe('container');
    expect(layerWithNodeId(tree, '0.0').kind).toBe('container');
  });

  // covers: FCT-0035
  it('shows bindings, conditions and expressions the editor does not represent as layers of their own kind', () => {
    const template: readonly TemplateNode[] = [
      element('0', 'article', [
        element('0.0', 'h3', [binding('0.0.0', 'plan')]),
        conditional('0.1', [element('0.1.t.0', 'hz-badge', [text('0.1.t.0.0', 'Le plus choisi')])], []),
        element('0.2', 'ul', [opaque('0.2.0')]),
      ]),
    ];
    const tree = buildLayerTree(model(template, ['pro']));

    expect(layerWithNodeId(tree, '0.0.0').kind).toBe('binding');
    expect(layerWithNodeId(tree, '0.1').kind).toBe('conditional');
    expect(layerWithNodeId(tree, '0.2.0').kind).toBe('opaque');
    expect(layerWithNodeId(tree, '0.0.0').id).toBe('pro:0.0.0');
  });

  // covers: FCT-0035
  it('lists the elements of both branches of a condition, true branch first, after the elements before it', () => {
    const template: readonly TemplateNode[] = [
      element('0', 'article', [
        element('0.0', 'h3', [text('0.0.0', 'Pro')]),
        conditional('0.1', [element('0.1.t.0', 'hz-badge'), element('0.1.t.1', 'span')], [element('0.1.f.0', 'p')]),
        element('0.2', 'footer'),
      ]),
    ];
    const order = ['0', '0.0', '0.1.t.0', '0.1.t.1', '0.1.f.0', '0.2'];

    expect(elementNodeIdsIn(buildLayerTree(model(template, ['pro'])), order)).toEqual(order);
  });

  // covers: FCT-0035
  it('shows the template elements once, with no variant branch, when the component has no variant', () => {
    const tree = buildLayerTree(model(CARD_TEMPLATE));
    const root = onlyRoot(tree);

    expect(root.name).toBe('hz-pricing-card');
    expect(root.kind).toBe('set');
    expect(flatten(tree).filter((layer) => layer.kind === 'variant')).toEqual([]);
    expect(elementNodeIdsIn(root.children, CARD_ELEMENT_IDS)).toEqual(CARD_ELEMENT_IDS);
  });

  // covers: FCT-0035
  it('keeps only the root and the variant branches for an empty template', () => {
    const tree = buildLayerTree(model([], ['neutral', 'accent']));

    expect(variantBranches(tree).map((branch) => branch.children)).toEqual([[], []]);
    expect(flatten(tree)).toHaveLength(3);
  });
});

describe('countLayers', () => {
  const leaf = (id: string, kind: LayerNode['kind']): LayerNode => ({ id, nodeId: null, name: id, kind, meta: null, children: [] });

  // covers: FCT-0035
  it('counts every layer of the tree, the root and the variant branches included', () => {
    const tree: LayerNode[] = [
      {
        id: 'root',
        nodeId: null,
        name: 'hz-plan-badge',
        kind: 'set',
        meta: null,
        children: [
          { id: 'neutral', nodeId: null, name: 'neutral', kind: 'variant', meta: 'neutral', children: [leaf('neutral:0', 'text')] },
          { id: 'accent', nodeId: null, name: 'accent', kind: 'variant', meta: 'accent', children: [leaf('accent:0', 'text')] },
        ],
      },
    ];

    expect(countLayers(tree)).toBe(5);
  });

  // covers: FCT-0035
  it('counts a lone root as one layer and an empty tree as none', () => {
    expect(countLayers([leaf('root', 'set')])).toBe(1);
    expect(countLayers([])).toBe(0);
  });

  // covers: FCT-0035
  it('counts the layers built from a component: the root, each variant branch and the elements of each instance', () => {
    const template = [element('0', 'article', [element('0.0', 'header', [element('0.0.0', 'h3')]), element('0.1', 'hz-button')])];

    expect(countLayers(buildLayerTree(model(template, ['starter', 'pro', 'equipe'])))).toBe(16);
    expect(countLayers(buildLayerTree(model(template)))).toBe(5);
  });
});
