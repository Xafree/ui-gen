import { describe, expect, it } from 'vitest';
import { createTypeScriptHighlighter } from '../../../../../src/modules/sync/adapters/typescript/typescript-highlighter';
import { ALL_FIXTURES, FEATURE_LIST, PLAN_BADGE, PRICING_CARD, readFixture } from '../../sync-fixtures';

type Token = ReturnType<ReturnType<typeof createTypeScriptHighlighter>['highlight']>[number];

const MOCKUP_CLASSES = [
  'tk-kw',
  'tk-str',
  'tk-num',
  'tk-tag',
  'tk-attr',
  'tk-fn',
  'tk-prop',
  'tk-type',
  'tk-dec',
  'tk-com',
  'tk-punc',
  'tk-var',
];

function highlight(text: string): readonly Token[] {
  return createTypeScriptHighlighter().highlight(text);
}

function classAt(text: string, tokens: readonly Token[], before: string, snippet: string, after = ''): string | null {
  const pattern = before + snippet + after;
  const index = text.indexOf(pattern);
  if (index < 0 || text.includes(pattern, index + 1)) {
    throw new Error(`expected exactly one occurrence of ${pattern}`);
  }
  const start = index + before.length;
  const token = tokens.find((candidate) => candidate.start === start && candidate.end === start + snippet.length);
  return token === undefined ? null : token.className;
}

function classesCovering(text: string, tokens: readonly Token[], snippet: string): string[] {
  const start = text.indexOf(snippet);
  if (start < 0) {
    throw new Error(`no ${snippet} in the text`);
  }
  const classes = new Set<string>();
  for (let offset = start; offset < start + snippet.length; offset += 1) {
    if (text[offset]?.trim() === '') {
      continue;
    }
    const covering = tokens.find((token) => token.start <= offset && offset < token.end);
    classes.add(covering === undefined ? 'none' : covering.className);
  }
  return [...classes];
}

function expectWellFormed(text: string, tokens: readonly Token[]) {
  let previousEnd = 0;
  for (const token of tokens) {
    expect(MOCKUP_CLASSES).toContain(token.className);
    expect(token.start).toBeGreaterThanOrEqual(previousEnd);
    expect(token.end).toBeGreaterThan(token.start);
    expect(token.end).toBeLessThanOrEqual(text.length);
    expect(text.slice(token.start, token.end).trim()).not.toBe('');
    previousEnd = token.end;
  }
}

describe('createTypeScriptHighlighter', () => {
  // covers: FCT-0038
  it('returns ordered, disjoint tokens within the text, with the mockup tk-* classes only', () => {
    for (const name of ALL_FIXTURES) {
      const text = readFixture(name);
      const tokens = highlight(text);

      expect(tokens.length).toBeGreaterThan(0);
      expectWellFormed(text, tokens);
    }
  });

  // covers: FCT-0038
  it('returns no token for an empty text', () => {
    expect(highlight('')).toEqual([]);
  });

  // covers: FCT-0038
  it('still colors a file whose syntax does not parse, without leaving the text', () => {
    const text = readFixture(PRICING_CARD);
    const truncated = text.slice(0, text.indexOf('<p class="description">'));
    const tokens = highlight(truncated);

    expectWellFormed(truncated, tokens);
    expect(classAt(truncated, tokens, '', 'import', ' { LitElement')).toBe('tk-kw');
  });

  // covers: FCT-0038
  it('colors TypeScript keywords, strings, numbers, decorators, types and comments', () => {
    const text = readFixture(PRICING_CARD);
    const tokens = highlight(text);
    const at = (before: string, snippet: string, after = '') => classAt(text, tokens, before, snippet, after);

    expect(at('', 'import', ' { LitElement, css')).toBe('tk-kw');
    expect(at("import { LitElement, css, html, nothing } ", 'from', " 'lit'")).toBe('tk-kw');
    expect(at('import { LitElement, css, html, nothing } from ', "'lit'")).toBe('tk-str');
    expect(at('', 'export', ' type Plan')).toBe('tk-kw');
    expect(at('export type ', 'Plan', ' =')).toBe('tk-type');
    expect(at('export type Plan = ', "'starter'")).toBe('tk-str');
    expect(at('', '@customElement', "('hz-pricing-card')")).toBe('tk-dec');
    expect(at('export ', 'class', ' HzPricingCard')).toBe('tk-kw');
    expect(at('export class ', 'HzPricingCard', ' extends')).toBe('tk-type');
    expect(at('export class HzPricingCard ', 'extends', ' LitElement')).toBe('tk-kw');
    expect(at('export class HzPricingCard extends ', 'LitElement', ' {')).toBe('tk-type');
    expect(at('export class HzPricingCard extends LitElement ', '{')).toBe('tk-punc');
    expect(at('', '@property', '({ type: Number }) price')).toBe('tk-dec');
    expect(at('@property({ type: ', 'Number', ' }) price')).toBe('tk-type');
    expect(at('@property({ type: Number }) price = ', '0', ';')).toBe('tk-num');
    expect(at('  ', 'static', ' styles')).toBe('tk-kw');
    expect(at('  private emit(name: ', 'string', ')')).toBe('tk-type');
    expect(at('  ', 'private', ' emit(')).toBe('tk-kw');
    expect(at('    bubbles: ', 'true', ',')).toBe('tk-kw');
    expect(at('', 'declare', ' global')).toBe('tk-kw');
    expect(classesCovering(text, tokens, "/** Variante du bouton d'action pour chaque offre. */")).toEqual(['tk-com']);
    expect(classesCovering(text, tokens, ' * Carte d\'offre tarifaire du Kit Horizon.')).toEqual(['tk-com']);
  });

  // covers: FCT-0038
  it('colors function calls, template tags and member properties', () => {
    const text = readFixture(PRICING_CARD);
    const tokens = highlight(text);
    const at = (before: string, snippet: string, after = '') => classAt(text, tokens, before, snippet, after);

    expect(at('  ', 'render', '() {')).toBe('tk-fn');
    expect(at('    return ', 'html', '`\n      <header>')).toBe('tk-fn');
    expect(at('static styles = ', 'css', '`')).toBe('tk-fn');
    expect(at('    ', 'this', '.dispatchEvent(')).toBe('tk-kw');
    expect(at('    this.', 'dispatchEvent', '(')).toBe('tk-fn');
    expect(at('detail: { plan: this.', 'variant', ' }')).toBe('tk-prop');
    expect(at("    this.hint = ", "''", ';\n  }\n\n  render')).toBe('tk-str');
  });

  // covers: FCT-0038
  it('colors the content of html templates: tags, attributes, attribute values and interpolations', () => {
    const text = readFixture(PRICING_CARD);
    const tokens = highlight(text);
    const at = (before: string, snippet: string, after = '') => classAt(text, tokens, before, snippet, after);

    expect(at('<', 'h3', ' class="plan">')).toBe('tk-tag');
    expect(at('<h3 ', 'class', '="plan">')).toBe('tk-attr');
    expect(at('<h3 class=', '"plan"', '>')).toBe('tk-str');
    expect(at('<h3 class="plan">', '${', 'this.plan}')).toBe('tk-punc');
    expect(at('<h3 class="plan">${', 'this', '.plan}')).toBe('tk-kw');
    expect(at('<h3 class="plan">${this.', 'plan', '}</h3>')).toBe('tk-prop');
    expect(at('<h3 class="plan">${this.plan', '}', '</h3>')).toBe('tk-punc');
    expect(at('${this.plan}</', 'h3', '>')).toBe('tk-tag');
    expect(at('      <', 'hz-button', '\n')).toBe('tk-tag');
    expect(at('        ', '?disabled', '=${this.disabled}')).toBe('tk-attr');
    expect(at('        ', '@click', '=${')).toBe('tk-attr');
    expect(at('this.showHint(', "'Essai gratuit de 14 jours'", ')')).toBe('tk-str');
    expect(at('${', 'formatPrice', '(this.price, this.locale)}')).toBe('tk-fn');
  });

  // covers: FCT-0038
  it('leaves the static text of an html template uncolored', () => {
    const badge = readFixture(PLAN_BADGE);
    const list = readFixture(FEATURE_LIST);

    expect(classesCovering(badge, highlight(badge), 'Encore')).toEqual(['none']);
    expect(classesCovering(badge, highlight(badge), 'places')).toEqual(['none']);
    expect(classesCovering(list, highlight(list), 'Sans engagement')).toEqual(['none']);
  });

  // covers: FCT-0038
  it('colors the content of css templates: selectors, properties, functions, custom properties and numbers', () => {
    const text = readFixture(PRICING_CARD);
    const tokens = highlight(text);
    const at = (before: string, snippet: string, after = '') => classAt(text, tokens, before, snippet, after);

    expect(at('    ', ':host', ' {\n      position: relative;')).toBe('tk-kw');
    expect(at('\n      ', 'position', ': relative;')).toBe('tk-prop');
    expect(at('\n      ', 'gap', ': var(--hz-space-4);')).toBe('tk-prop');
    expect(at('gap: ', 'var', '(--hz-space-4)')).toBe('tk-fn');
    expect(at('gap: var(', '--hz-space-4', ')')).toBe('tk-var');
    expect(at('border: ', '1px', ' solid')).toBe('tk-num');
    expect(at(':host([', 'variant', "='pro']) {")).toBe('tk-attr');
    expect(at(':host([variant=', "'pro'", ']) {')).toBe('tk-str');
    expect(at('\n    ', '.plan', ' {')).toBe('tk-tag');
    expect(at('\n    ', 'header', ' {')).toBe('tk-tag');
    expect(at('    ', '::slotted', '(li) {')).toBe('tk-kw');
    expect(at('      ', '--hz-button-direction', ': row;')).toBe('tk-var');
    expect(at('margin-top: auto;\n      width: ', '100%', ';')).toBe('tk-num');
    expect(at('bottom: ', 'calc', '(100%')).toBe('tk-fn');
  });

  // covers: FCT-0038
  it('colors the css and html templates of a hand-written file with tabs and double quotes', () => {
    const text = readFixture(PLAN_BADGE);
    const tokens = highlight(text);
    const at = (before: string, snippet: string, after = '') => classAt(text, tokens, before, snippet, after);

    expect(at('import {LitElement, html, css} from ', '"lit"')).toBe('tk-str');
    expect(at('\t\t', ':host', ' { display')).toBe('tk-kw');
    expect(at(':host { ', 'display', ': inline-flex; }')).toBe('tk-prop');
    expect(at('<', 'span', ' class="badge  badge--round"')).toBe('tk-tag');
    expect(at('<span class="badge  badge--round" ', 'role', '="status">')).toBe('tk-attr');
    expect(at('', '// Taille de la pastille.')).toBe('tk-com');
    expect(at('@property({type: ', 'Number', '}) count')).toBe('tk-type');
  });
});
