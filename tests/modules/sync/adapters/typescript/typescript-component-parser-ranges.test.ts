import { describe, expect, it } from 'vitest';
import {
  ALL_FIXTURES,
  FEATURE_LIST,
  PRICING_CARD,
  findElement,
  findProperty,
  positionOf,
  read,
  readFixture,
  readFixtureModel,
  replaceOnce,
  sliceOf,
  type Model,
  type TemplateNodeShape,
} from '../../sync-fixtures';

interface Range {
  readonly start: number;
  readonly end: number;
}

function expectInside(text: string, range: Range): void {
  expect(range.start).toBeGreaterThanOrEqual(0);
  expect(range.end).toBeGreaterThan(range.start);
  expect(range.end).toBeLessThanOrEqual(text.length);
}

function expectNodeRanges(text: string, nodes: readonly TemplateNodeShape[], parent: Range | null): void {
  let previousEnd = parent === null ? 0 : parent.start;
  for (const node of nodes) {
    expectInside(text, node.range);
    expect(node.range.start).toBeGreaterThanOrEqual(previousEnd);
    if (parent !== null) {
      expect(node.range.end).toBeLessThanOrEqual(parent.end);
    }
    previousEnd = node.range.end;
    const source = sliceOf(text, node.range);
    switch (node.kind) {
      case 'element':
        expect(source.startsWith(`<${node.tag}`)).toBe(true);
        expect(source.endsWith(`</${node.tag}>`)).toBe(true);
        for (const attribute of node.attributes) {
          expectInside(text, attribute.range);
          expect(attribute.range.start).toBeGreaterThan(node.range.start);
          expect(sliceOf(text, attribute.range).startsWith(attribute.name)).toBe(true);
          if (attribute.value !== null) {
            expect(sliceOf(text, attribute.range)).toContain(attribute.value);
          }
        }
        expectNodeRanges(text, node.children, node.range);
        break;
      case 'text':
        expect(source).toBe(node.text);
        break;
      case 'binding':
        expect(source).toBe(`\${this.${node.property}}`);
        break;
      case 'opaque':
        expect(source).toBe(node.source);
        expect(source.startsWith('${')).toBe(true);
        expect(source.endsWith('}')).toBe(true);
        break;
    }
  }
}

function expectAllRangesDesignateTheSource(text: string, model: Model): void {
  expectNodeRanges(text, model.template, null);
  for (const property of model.properties) {
    expectInside(text, property.range);
    expect(sliceOf(text, property.range)).toContain(property.name);
  }
  for (const diagnostic of model.diagnostics) {
    expect(diagnostic.range.start).toBeGreaterThanOrEqual(0);
    expect(diagnostic.range.end).toBeGreaterThanOrEqual(diagnostic.range.start);
    expect(diagnostic.range.end).toBeLessThanOrEqual(text.length);
    expect({ line: diagnostic.line, column: diagnostic.column }).toEqual(positionOf(text, diagnostic.range.start));
  }
}

describe('source ranges of the component model', () => {
  it('makes every range of every fixture designate exactly its text in the file', () => {
    for (const name of ALL_FIXTURES) {
      const text = readFixture(name);
      expectAllRangesDesignateTheSource(text, read(text));
    }
  });

  it('keeps ranges exact when the file uses CRLF line endings', () => {
    for (const name of ALL_FIXTURES) {
      const text = readFixture(name).replaceAll('\n', '\r\n');
      expectAllRangesDesignateTheSource(text, read(text));
    }
  });

  it('keeps ranges exact when an interpolation spans several lines and nests a template', () => {
    const { text, model } = readFixtureModel(FEATURE_LIST);

    const ol = findElement(model.template, 'ol');
    const opaque = ol.children.find((node) => node.kind === 'opaque');
    expect(opaque === undefined ? null : sliceOf(text, opaque.range)).toBe(
      [
        '${repeat(',
        '          this.features,',
        '          (feature) => feature,',
        '          (feature, index) => html`<li>${index + 1}. ${feature}</li>`,',
        '        )}',
      ].join('\n'),
    );
  });

  it('gives the range of a multi-line element from its opening tag to its closing tag', () => {
    const { text, model } = readFixtureModel(PRICING_CARD);

    const button = findElement(model.template, 'hz-button');
    const source = sliceOf(text, button.range);
    expect(source.startsWith('<hz-button\n        class="cta"\n')).toBe(true);
    expect(source.endsWith('nothing}\n      </hz-button>')).toBe(true);
  });

  it('gives a declared property a range that covers its declaration', () => {
    const { text, model } = readFixtureModel(PRICING_CARD);

    expect(sliceOf(text, findProperty(model, 'price').range)).toContain('price = 0');
    expect(sliceOf(text, findProperty(model, 'hint').range)).toContain("private hint = ''");
  });

  it('shifts every range with the text when lines are added above the class', () => {
    const original = readFixture(PRICING_CARD);
    const shifted = replaceOnce(original, "import './hz-badge.js';\n", "import './hz-badge.js';\n// note\n// note\n");
    const before = findElement(read(original).template, 'slot').range;
    const after = findElement(read(shifted).template, 'slot').range;

    expect(after).toEqual({ start: before.start + 16, end: before.end + 16 });
    expectAllRangesDesignateTheSource(shifted, read(shifted));
  });
});
