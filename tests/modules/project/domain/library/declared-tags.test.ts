import { describe, expect, it } from 'vitest';
import { findDeclaredTags } from '../../../../../src/modules/project/domain/library/declared-tags';

describe('findDeclaredTags', () => {
  // covers: FCT-0007
  it('finds a tag declared with the customElement decorator', () => {
    const source = [
      "import { LitElement, html } from 'lit';",
      "import { customElement } from 'lit/decorators.js';",
      '',
      "@customElement('hz-button')",
      'export class HzButton extends LitElement {',
      '  render() {',
      '    return html`<button><slot></slot></button>`;',
      '  }',
      '}',
    ].join('\n');
    expect(findDeclaredTags(source)).toEqual(['hz-button']);
  });

  // covers: FCT-0007
  it('finds a tag declared with customElements.define', () => {
    const source = [
      "import { LitElement } from 'lit';",
      'export class HzCard extends LitElement {}',
      "customElements.define('hz-card', HzCard);",
    ].join('\n');
    expect(findDeclaredTags(source)).toEqual(['hz-card']);
  });

  // covers: FCT-0007
  it('finds a tag declared with window.customElements.define', () => {
    expect(findDeclaredTags("window.customElements.define('hz-chip', HzChip);")).toEqual(['hz-chip']);
  });

  // covers: FCT-0007
  it('accepts single, double and backtick quotes', () => {
    const source = [
      "customElements.define('a-single', A);",
      'customElements.define("b-double", B);',
      'customElements.define(`c-backtick`, C);',
      "@customElement('d-single')",
      '@customElement("e-double")',
      '@customElement(`f-backtick`)',
    ].join('\n');
    expect(findDeclaredTags(source)).toEqual([
      'a-single',
      'b-double',
      'c-backtick',
      'd-single',
      'e-double',
      'f-backtick',
    ]);
  });

  // covers: FCT-0007
  it('keeps the file order across both declaration forms', () => {
    const source = [
      "@customElement('zz-first')",
      'class First extends LitElement {}',
      "customElements.define('aa-second', Second);",
      "@customElement('mm-third')",
      'class Third extends LitElement {}',
    ].join('\n');
    expect(findDeclaredTags(source)).toEqual(['zz-first', 'aa-second', 'mm-third']);
  });

  // covers: FCT-0007
  it('keeps duplicate declarations', () => {
    const source = ["customElements.define('hz-tab', Tab);", "@customElement('hz-tab')"].join('\n');
    expect(findDeclaredTags(source)).toEqual(['hz-tab', 'hz-tab']);
  });

  // covers: FCT-0007
  it('keeps tag names with several hyphens and digits', () => {
    expect(findDeclaredTags("@customElement('my-big-button2')")).toEqual(['my-big-button2']);
  });

  // covers: FCT-0007
  it.each([
    ['without a hyphen', "customElements.define('button', B);"],
    ['starting with an uppercase letter', "customElements.define('Hz-button', B);"],
    ['starting with a digit', "@customElement('1-button')"],
    ['starting with a hyphen', "@customElement('-button')"],
    ['empty', "@customElement('')"],
  ])('ignores a tag name %s', (_label, source) => {
    expect(findDeclaredTags(source)).toEqual([]);
  });

  // covers: FCT-0007
  it('ignores tag names that are not declarations', () => {
    const source = [
      "const el = document.createElement('hz-button');",
      "customElements.get('hz-card');",
      'const tpl = html`<hz-chip></hz-chip>`;',
      "const name = 'hz-tab';",
    ].join('\n');
    expect(findDeclaredTags(source)).toEqual([]);
  });

  // covers: FCT-0007
  it('returns an empty list for an empty source', () => {
    expect(findDeclaredTags('')).toEqual([]);
  });
});
