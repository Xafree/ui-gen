import { expect } from 'vitest';
import { userEvent } from 'vitest/browser';
import type { ComponentModel, TemplateNode } from '../../../../src/modules/sync/application/component-model.js';
import type { ComponentChange, OpenedComponent } from '../../../../src/modules/sync/application/components/component-views.js';
import type { Diagnostic } from '../../../../src/modules/sync/domain/model/diagnostic.js';
import { deepQueryAll, visibleText } from '../../home/shadow-dom.js';
import {
  BROKEN_TEXT,
  CARD_TEXT,
  EXPRESSION,
  NOT_REPRESENTABLE_MESSAGE,
  cardModel,
  codeLines,
  highlightOf,
  syntaxError,
} from '../code/code-fixtures.js';
import { PRICING_CARD, htmlElement, isShown, textOf } from '../editor-fixtures.js';

export const SECOND_EXPRESSION = '${this.features.map((feature) => html`<li>${feature}</li>`)}';

export const TWO_EXPRESSIONS_TEXT = `import { LitElement, html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

@customElement('hz-pricing-card')
export class HzPricingCard extends LitElement {
  @property() price = 29;
  @property() features: string[] = [];

  render() {
    return html\`
      <article class="card">
        <p class="price">\${formatPrice(this.price)}</p>
        <ul class="features">\${this.features.map((feature) => html\`<li>\${feature}</li>\`)}</ul>
      </article>
    \`;
  }
}
`;

export interface ShownLens {
  readonly line: number;
  readonly message: string;
}

export interface ProblemEntry {
  readonly text: string;
  readonly location: string;
  readonly warning: boolean;
}

export interface Counters {
  readonly tabCount: string | null;
  readonly status: string | null;
  readonly statusName: string | null;
}

export interface DiagnosticCard {
  readonly title: string;
  readonly text: string;
  readonly meta: string;
}

export interface SyncState {
  readonly code: string;
  readonly codeDot: string | null;
  readonly status: string;
  readonly statusDot: string | null;
}

function rangeIn(text: string, snippet: string): { readonly start: number; readonly end: number } {
  const start = text.indexOf(snippet);
  return { start, end: start + snippet.length };
}

function element(id: string, tag: string, open: string, close: string, children: readonly TemplateNode[]): TemplateNode {
  const start = TWO_EXPRESSIONS_TEXT.indexOf(open);
  const end = TWO_EXPRESSIONS_TEXT.indexOf(close, start) + close.length;
  return { kind: 'element', id, tag, range: { start, end }, attributes: [], classes: [], children };
}

function opaque(id: string, source: string): TemplateNode {
  return { kind: 'opaque', id, range: rangeIn(TWO_EXPRESSIONS_TEXT, source), source };
}

function expressionWarning(source: string, line: number, column: number): Diagnostic {
  return {
    range: rangeIn(TWO_EXPRESSIONS_TEXT, source),
    line,
    column,
    severity: 'warning',
    code: 'ui-gen(visual-sync)',
    message: NOT_REPRESENTABLE_MESSAGE,
  };
}

function twoExpressionsModel(): ComponentModel {
  return {
    tag: 'hz-pricing-card',
    className: 'HzPricingCard',
    version: 'v1',
    template: [
      element('0', 'article', '<article', '</article>', [
        element('0.0', 'p', '<p', '</p>', [opaque('0.0.0', EXPRESSION)]),
        element('0.1', 'ul', '<ul', '</ul>', [opaque('0.1.0', SECOND_EXPRESSION)]),
      ]),
    ],
    properties: [],
    variantProperty: null,
    variants: [],
    styles: [],
    diagnostics: [expressionWarning(EXPRESSION, 12, 26), expressionWarning(SECOND_EXPRESSION, 13, 30)],
  };
}

export function twoExpressionsComponent(): OpenedComponent {
  return { path: PRICING_CARD.path, text: TWO_EXPRESSIONS_TEXT, model: twoExpressionsModel(), highlight: [] };
}

export function brokenOpened(): OpenedComponent {
  return {
    path: PRICING_CARD.path,
    text: BROKEN_TEXT,
    model: {
      tag: '',
      className: '',
      version: 'v1',
      template: [],
      properties: [],
      variantProperty: null,
      variants: [],
      styles: [],
      diagnostics: [syntaxError()],
    },
    highlight: [],
  };
}

export function readableChange(diagnostics: readonly Diagnostic[], version = 'v2'): ComponentChange {
  return {
    path: PRICING_CARD.path,
    text: CARD_TEXT,
    model: cardModel(diagnostics, version),
    highlight: highlightOf(),
    diagnostics,
  };
}

export function plainLines(): string[] {
  return codeLines().map((line) => {
    const copy = line.cloneNode(true);
    if (!(copy instanceof HTMLElement)) throw new Error('A code line is not an HTML element');
    for (const lens of copy.querySelectorAll('.ui-code__lens')) lens.remove();
    return copy.textContent;
  });
}

export function underlines(): HTMLElement[] {
  return deepQueryAll('#code .ui-diag').filter((mark): mark is HTMLElement => mark instanceof HTMLElement);
}

export function underlineAt(index: number): HTMLElement {
  const mark = underlines()[index];
  if (mark === undefined) throw new Error(`There is no underline number ${String(index)}`);
  return mark;
}

export function isWarningUnderline(mark: Element): boolean {
  return mark.classList.contains('ui-diag--warning');
}

export function lineNumberOf(target: Element): number {
  return codeLines().findIndex((line) => line.contains(target)) + 1;
}

export function markedLines(mark: 'has-warning' | 'has-error'): number[] {
  return codeLines().flatMap((line, index) => (line.classList.contains(mark) ? [index + 1] : []));
}

export function gutterColor(lineNumber: number): string {
  const line = codeLines()[lineNumber - 1];
  if (line === undefined) throw new Error(`There is no code line ${String(lineNumber)}`);
  return getComputedStyle(line, '::before').color;
}

function generatedText(line: Element): string {
  const { content } = getComputedStyle(line, '::after');
  return content === 'none' || content === 'normal' ? '' : content.slice(1, -1);
}

function lensOf(line: HTMLElement): string {
  const row = line.getBoundingClientRect();
  const lenses = deepQueryAll('.ui-code__lens').filter((lens) => {
    const box = lens.getBoundingClientRect();
    return box.height > 0 && box.top >= row.top - 1 && box.bottom <= row.bottom + 1;
  });
  return lenses.length > 0 ? lenses.map((lens) => visibleText(lens)).join(' ') : generatedText(line);
}

export function shownLenses(): ShownLens[] {
  return codeLines().flatMap((line, index) => {
    const message = lensOf(line);
    return message === '' ? [] : [{ line: index + 1, message }];
  });
}

export function problemEntries(): ProblemEntry[] {
  return deepQueryAll('#problems .ui-problem').map((entry) => ({
    text: visibleText(entry),
    location: visibleText(entry.querySelector('.ui-problem__loc')),
    warning: entry.classList.contains('ui-problem--warning'),
  }));
}

export async function openProblemsTab(): Promise<void> {
  await userEvent.click(htmlElement('#tab-problems'));
  await expect.poll(() => htmlElement('#tab-problems').getAttribute('aria-selected')).toBe('true');
}

export function counters(): Counters {
  const statusShown = isShown('#status-warn');
  return {
    tabCount: isShown('#problem-count') ? textOf('#problem-count') : null,
    status: statusShown ? textOf('#status-warn-label') : null,
    statusName: statusShown ? htmlElement('#status-warn').getAttribute('aria-label') : null,
  };
}

export function diagnosticCard(): DiagnosticCard {
  return { title: textOf('#diag-title'), text: textOf('#diag-text'), meta: textOf('#diag-meta') };
}

function dotTone(indicator: string): string | null {
  const dot = deepQueryAll(`${indicator} .ui-status-dot`)[0];
  return /\bui-status-dot--([a-z]+)\b/.exec(dot?.className ?? '')?.[1] ?? null;
}

export function syncState(): SyncState {
  return {
    code: textOf('#code-sync'),
    codeDot: dotTone('#code-sync'),
    status: textOf('#status-sync-label'),
    statusDot: dotTone('#status-sync'),
  };
}
