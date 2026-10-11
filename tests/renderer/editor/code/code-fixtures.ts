import { expect } from 'vitest';
import type { ComponentModel, TemplateNode } from '../../../../src/modules/sync/application/component-model.js';
import type { ComponentChange, OpenedComponent } from '../../../../src/modules/sync/application/components/component-views.js';
import type { Diagnostic } from '../../../../src/modules/sync/domain/model/diagnostic.js';
import type { HighlightToken } from '../../../../src/modules/sync/ports/code-highlighter.js';
import { deepQueryAll } from '../../home/shadow-dom.js';
import { PRICING_CARD, fakeEditorApi, mountEditor, type FakeEditorApi } from '../editor-fixtures.js';

export const CARD_TEXT = `import { LitElement, html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

@customElement('hz-pricing-card')
export class HzPricingCard extends LitElement {
  @property() price = 29;

  render() {
    return html\`
      <article class="card">
        <h3 class="plan">Pro</h3>
        <p class="price">\${formatPrice(this.price)}</p>
        <button class="cta">Choisir</button>
      </article>
    \`;
  }
}
`;

export const EXPRESSION = '${formatPrice(this.price)}';
export const NOT_REPRESENTABLE_MESSAGE =
  "Cette expression ne peut pas être affichée dans l'éditeur visuel. Elle est conservée dans le code.";
export const EXPRESSION_LINE = 12;
export const EXPRESSION_COLUMN = 26;
export const BROKEN_TEXT = CARD_TEXT.replace('price = 29;', 'price = 29');
export const BROKEN_LINE = 6;

function rangeOf(snippet: string, from = 0): { readonly start: number; readonly end: number } {
  const start = CARD_TEXT.indexOf(snippet, from);
  return { start, end: start + snippet.length };
}

function element(id: string, tag: string, open: string, close: string, children: readonly TemplateNode[]): TemplateNode {
  const start = CARD_TEXT.indexOf(open);
  const end = CARD_TEXT.indexOf(close, start) + close.length;
  return { kind: 'element', id, tag, range: { start, end }, attributes: [], classes: [], children };
}

const TEMPLATE: readonly TemplateNode[] = [
  element('0', 'article', '<article', '</article>', [
    element('0.0', 'h3', '<h3', '</h3>', []),
    element('0.1', 'p', '<p', '</p>', [{ kind: 'opaque', id: '0.1.0', range: rangeOf(EXPRESSION), source: EXPRESSION }]),
    element('0.2', 'button', '<button', '</button>', []),
  ]),
];

export function warning(): Diagnostic {
  return { range: rangeOf(EXPRESSION), line: EXPRESSION_LINE, column: EXPRESSION_COLUMN, severity: 'warning', code: 'ui-gen(visual-sync)', message: NOT_REPRESENTABLE_MESSAGE };
}

export function syntaxError(): Diagnostic {
  const at = BROKEN_TEXT.indexOf('price = 29') + 'price = 29'.length;
  return { range: { start: at, end: at + 1 }, line: BROKEN_LINE, column: 25, severity: 'error', code: 'ts(1005)', message: "';' attendu." };
}

export function cardModel(diagnostics: readonly Diagnostic[], version = 'v1'): ComponentModel {
  const value = (key: 'gap' | 'fill' | 'fontSize' | 'width', raw: string, token: string | null) => ({ key, raw, token, range: rangeOf(raw) });
  return {
    tag: 'hz-pricing-card',
    className: 'HzPricingCard',
    version,
    template: TEMPLATE,
    properties: [],
    variantProperty: null,
    variants: [],
    styles: [
      {
        selector: '.cta',
        range: { start: 0, end: 0 },
        values: [value('gap', 'var(--hz-space-4)', '--hz-space-4'), value('fill', 'var(--hz-color-accent)', '--hz-color-accent'), value('fontSize', '14px', null)],
        targets: ['0.2'],
        variant: null,
        state: null,
      },
    ],
    diagnostics,
  };
}

export function highlightOf(): HighlightToken[] {
  const token = (snippet: string, className: HighlightToken['className']): HighlightToken => ({ ...rangeOf(snippet), className });
  return [token('import', 'tk-kw'), token("'lit'", 'tk-str'), token('@customElement', 'tk-dec'), token('29', 'tk-num')];
}

export function cardComponent(diagnostics: readonly Diagnostic[] = [warning()], text = CARD_TEXT): OpenedComponent {
  return { path: PRICING_CARD.path, text, model: cardModel(diagnostics), highlight: text === CARD_TEXT ? highlightOf() : [] };
}

export function brokenChange(): ComponentChange {
  return { path: PRICING_CARD.path, text: BROKEN_TEXT, model: cardModel([warning()]), highlight: [], diagnostics: [syntaxError()] };
}

export function codeLines(): HTMLElement[] {
  return deepQueryAll('#code .ui-code__line').filter((line): line is HTMLElement => line instanceof HTMLElement);
}

export async function mountCard(mounted: Element[], component: OpenedComponent = cardComponent()): Promise<FakeEditorApi> {
  const fake = fakeEditorApi({ lastOpened: () => Promise.resolve(PRICING_CARD.path), open: () => Promise.resolve(component) });
  await mountEditor(fake.api, mounted);
  await expect.poll(() => codeLines().length).toBeGreaterThan(0);
  return fake;
}
