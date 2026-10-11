import { describe, expect, it } from 'vitest';
import { instrumentComponentSource } from '../../../../src/modules/canvas/application/instrument-component';
import {
  ALL_FIXTURES,
  FEATURE_LIST,
  PRICING_CARD,
  allNodes,
  outline,
  read,
  readFixture,
  readFixtureModel,
  type Model,
  type TemplateNodeShape,
} from '../../sync/sync-fixtures';

type ElementShape = Extract<TemplateNodeShape, { kind: 'element' }>;

interface Mark {
  readonly id: string;
  readonly offsetInOutput: number;
}

const MARK_PATTERN = /data-uigen-node="([^"]*)"/g;

const FORM_FIELD = `import { LitElement, html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

@customElement('hz-form-field')
export class HzFormField extends LitElement {
  @property({ type: Boolean }) multiline = false;
  @property() label = '';

  render() {
    return html\`<label class="field">
      <span>\${this.label}</span>
      \${this.multiline ? html\`<textarea rows="3"></textarea>\` : html\`<input type="text" /><br>\`}
    </label>\`;
  }
}
`;

const GREETING = `import { LitElement, html } from 'lit';
import { customElement } from 'lit/decorators.js';

@customElement('hz-greeting')
export class HzGreeting extends LitElement {
  render() {
    return html\`Bonjour\`;
  }
}
`;

function elementsOf(model: Model): ElementShape[] {
  return allNodes(model.template).filter((node): node is ElementShape => node.kind === 'element');
}

function marksOf(output: string): Mark[] {
  return [...output.matchAll(MARK_PATTERN)].map((match) => ({ id: match[1] ?? '', offsetInOutput: match.index }));
}

function withoutMarks(output: string): string[] {
  return [/ data-uigen-node="[^"]*"/g, /data-uigen-node="[^"]*" /g].map((pattern) => output.replace(pattern, ''));
}

function originalOffsetOf(output: string, mark: Mark): number {
  const before = output.slice(0, mark.offsetInOutput);
  return withoutMarks(before).reduce((shortest, candidate) => Math.min(shortest, candidate.length), before.length);
}

function openingTagEnd(element: ElementShape): number {
  const firstChild = element.children[0];
  return firstChild === undefined ? element.range.end : firstChild.range.start;
}

function expectMarkedOncePerElement(text: string, model: Model, output: string): void {
  const marks = marksOf(output);
  for (const element of elementsOf(model)) {
    const own = marks.filter((mark) => mark.id === element.id);
    expect(own, `element ${element.id} <${element.tag}>`).toHaveLength(1);
    const [mark] = own;
    if (mark === undefined) continue;
    const offset = originalOffsetOf(output, mark);
    expect(offset, `mark of ${element.id} sits in its opening tag`).toBeGreaterThanOrEqual(
      element.range.start + 1 + element.tag.length,
    );
    expect(offset, `mark of ${element.id} sits in its opening tag`).toBeLessThanOrEqual(openingTagEnd(element));
    expect(text.slice(element.range.start, element.range.start + 1 + element.tag.length)).toBe(`<${element.tag}`);
  }
}

describe('instrumentComponentSource', () => {
  // covers: FCT-0030
  it('marks every element node of the render template of the pricing card with its node id', () => {
    const { text, model } = readFixtureModel(PRICING_CARD);

    const output = instrumentComponentSource(text, model);

    expectMarkedOncePerElement(text, model, output);
    expect(elementsOf(model).map(({ id }) => id)).toEqual([
      '0',
      '0.0.t.0',
      '0.1',
      '0.2',
      '0.3',
      '0.3.1',
      '1',
      '1.0',
      '2',
      '2.1.t.0',
      '2.1.t.0.0',
      '3.t.0',
    ]);
  });

  // covers: FCT-0030
  it('marks every element node of each fixture exactly once, inside its own opening tag', () => {
    for (const name of ALL_FIXTURES) {
      const { text, model } = readFixtureModel(name);

      expectMarkedOncePerElement(text, model, instrumentComponentSource(text, model));
    }
  });

  // covers: FCT-0031
  it('leaves every character outside the inserted marks unchanged', () => {
    for (const name of ALL_FIXTURES) {
      const { text, model } = readFixtureModel(name);

      const output = instrumentComponentSource(text, model);

      expect(output).not.toBe(text);
      expect(withoutMarks(output)).toContain(text);
    }
  });

  // covers: FCT-0031
  it('returns a copy and never alters the text it was given', () => {
    const { text, model } = readFixtureModel(PRICING_CARD);

    instrumentComponentSource(text, model);

    expect(text).toBe(readFixture(PRICING_CARD));
    expect(read(text)).toEqual(model);
  });

  // covers: FCT-0030
  it('keeps the copy a readable component with the same tree, each element carrying its id as an attribute', () => {
    for (const name of ALL_FIXTURES) {
      const { text, model } = readFixtureModel(name);

      const copy = read(instrumentComponentSource(text, model));

      expect(copy.tag).toBe(model.tag);
      expect(outline(copy.template)).toEqual(outline(model.template));
      expect(copy.diagnostics.filter(({ severity }) => severity === 'error')).toEqual([]);
      const originals = elementsOf(model);
      const copied = elementsOf(copy);
      expect(copied.map(({ id }) => id)).toEqual(originals.map(({ id }) => id));
      for (const [index, element] of copied.entries()) {
        const original = originals[index];
        const marks = element.attributes.filter(({ name: attribute }) => attribute === 'data-uigen-node');
        expect(marks.map(({ value }) => value)).toEqual([element.id]);
        expect(
          element.attributes
            .filter(({ name: attribute }) => attribute !== 'data-uigen-node')
            .map(({ name: attribute, value }) => [attribute, value]),
        ).toEqual(original?.attributes.map(({ name: attribute, value }) => [attribute, value]));
      }
    }
  });

  // covers: FCT-0030
  it('marks the elements of both branches of a conditional with their branch ids', () => {
    const model = read(FORM_FIELD);
    const ids = elementsOf(model).map(({ id, tag }) => `${tag}:${id}`);
    expect(ids).toEqual(['label:0', 'span:0.0', 'textarea:0.1.t.0', 'input:0.1.f.0', 'br:0.1.f.1']);

    const output = instrumentComponentSource(FORM_FIELD, model);

    expectMarkedOncePerElement(FORM_FIELD, model, output);
    expect(withoutMarks(output)).toContain(FORM_FIELD);
    expect(
      marksOf(output)
        .map(({ id }) => id)
        .toSorted(),
    ).toEqual(['0', '0.0', '0.1.f.0', '0.1.f.1', '0.1.t.0']);
  });

  // covers: FCT-0030
  it('gives no element id of their own to elements produced by an opaque expression, nor to templates outside render', () => {
    const { text, model } = readFixtureModel(FEATURE_LIST);
    const elementIds = new Set(elementsOf(model).map(({ id }) => id));
    const opaqueIds = new Set(
      allNodes(model.template)
        .filter((node) => node.kind === 'opaque')
        .map(({ id }) => id),
    );

    const output = instrumentComponentSource(text, model);

    const marks = marksOf(output);
    for (const { id } of marks) expect(elementIds.has(id) || opaqueIds.has(id), `mark ${id}`).toBe(true);
    expect(marks.filter(({ id }) => elementIds.has(id))).toHaveLength(elementIds.size);
    expect(output).toContain('return html`<footer>Sans engagement</footer>`;');
  });

  // covers: FCT-0031
  it('returns the text unchanged when the render template has no element', () => {
    const model = read(GREETING);
    expect(elementsOf(model)).toEqual([]);

    expect(instrumentComponentSource(GREETING, model)).toBe(GREETING);
  });
});
