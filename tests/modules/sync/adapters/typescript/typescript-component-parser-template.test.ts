import { describe, expect, it } from 'vitest';
import {
  ALL_FIXTURES,
  FEATURE_LIST,
  PLAN_BADGE,
  PRICING_CARD,
  allNodes,
  findElement,
  outline,
  readFixtureModel,
  sliceOf,
  type TemplateNodeShape,
} from '../../sync-fixtures';

function expectIdsArePaths(nodes: readonly TemplateNodeShape[], parentId: string | null): void {
  nodes.forEach((node, index) => {
    const expectedId = parentId === null ? String(index) : `${parentId}.${String(index)}`;
    expect(node.id).toBe(expectedId);
    if (node.kind === 'element') {
      expectIdsArePaths(node.children, node.id);
    }
  });
}

describe('template tree of a component read with the TypeScript parser', () => {
  // covers: FCT-0035
  it('reproduces the elements of the render() template of the mockup pricing card in source order', () => {
    const { model } = readFixtureModel(PRICING_CARD);

    expect(outline(model.template)).toEqual([
      [
        'header',
        [
          'opaque',
          ['h3.plan', ['binding:plan']],
          ['p.description', ['binding:description']],
          ['p.price', ['opaque', ['span.period', ['binding:period']]]],
        ],
      ],
      ['ul.features', [['slot', []]]],
      ['hz-button.cta', ['binding:cta', 'opaque']],
      'opaque',
    ]);
  });

  // covers: FCT-0035
  it('reads a hand-formatted template with tabs, mixed text and bindings and a boolean attribute', () => {
    const { model } = readFixtureModel(PLAN_BADGE);

    expect(outline(model.template)).toEqual([
      [
        'span.badge.badge--round',
        [
          ['strong', ['binding:label']],
          'text:Encore',
          'binding:count',
          'text:places',
          ['button', ['text:Choisir']],
        ],
      ],
    ]);
  });

  // covers: FCT-0035
  it('takes the tree from render() only and keeps map, repeat, calls, ternaries and other members as single opaque nodes', () => {
    const { model } = readFixtureModel(FEATURE_LIST);

    expect(outline(model.template)).toEqual([
      ['h4', ['binding:heading']],
      ['ul', ['opaque']],
      ['ol', ['opaque']],
      ['p', ['opaque']],
      ['p', ['opaque']],
      ['p', ['opaque']],
      'opaque',
    ]);
    const tags = allNodes(model.template).flatMap((node) => (node.kind === 'element' ? [node.tag] : []));
    expect(tags).not.toContain('li');
    expect(tags).not.toContain('footer');
  });

  // covers: FCT-0035
  it('identifies every node by its index path from the template root', () => {
    for (const name of ALL_FIXTURES) {
      const { model } = readFixtureModel(name);
      expect(model.template.length).toBeGreaterThan(0);
      expectIdsArePaths(model.template, null);
    }
  });

  // covers: FCT-0035
  it('gives the root element of a template without surrounding whitespace the id 0', () => {
    const { model } = readFixtureModel(PLAN_BADGE);

    expect(model.template).toHaveLength(1);
    expect(model.template[0]?.id).toBe('0');
  });

  // covers: FCT-0035
  it('keeps every node id unique within one version', () => {
    for (const name of ALL_FIXTURES) {
      const ids = allNodes(readFixtureModel(name).model.template).map((node) => node.id);
      expect(new Set(ids).size).toBe(ids.length);
    }
  });

  it('reads static attributes with their value, null for a boolean attribute, and splits classes', () => {
    const { text, model } = readFixtureModel(PLAN_BADGE);

    const span = findElement(model.template, 'span');
    expect(span.classes).toEqual(['badge', 'badge--round']);
    expect(span.attributes.map(({ name, value }) => ({ name, value }))).toEqual([
      { name: 'class', value: 'badge  badge--round' },
      { name: 'role', value: 'status' },
    ]);

    const button = findElement(model.template, 'button');
    expect(button.classes).toEqual([]);
    expect(button.attributes.map(({ name, value }) => ({ name, value }))).toEqual([
      { name: 'type', value: 'button' },
      { name: 'disabled', value: null },
    ]);
    expect(button.attributes.map((attribute) => sliceOf(text, attribute.range))).toEqual(['type="button"', 'disabled']);
  });

  it('reads the named slot of the mockup pricing card', () => {
    const { text, model } = readFixtureModel(PRICING_CARD);

    const slot = findElement(model.template, 'slot');
    expect(slot.attributes.map(({ name, value }) => ({ name, value }))).toEqual([{ name: 'name', value: 'features' }]);
    expect(slot.attributes.map((attribute) => sliceOf(text, attribute.range))).toEqual(['name="features"']);
    expect(sliceOf(text, slot.range)).toBe('<slot name="features"></slot>');
  });

  it('turns only the child interpolations of a declared property into bindings', () => {
    const { model } = readFixtureModel(PRICING_CARD);

    const bindings = allNodes(model.template).flatMap((node) => (node.kind === 'binding' ? [node.property] : []));
    expect(bindings).toEqual(['plan', 'description', 'period', 'cta']);
  });
});
