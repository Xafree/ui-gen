import { describe, expect, it } from 'vitest';
import {
  FEATURE_LIST,
  NOT_REPRESENTABLE_MESSAGE,
  PLAN_BADGE,
  PRICING_CARD,
  allNodes,
  outline,
  read,
  readFixture,
  readFixtureModel,
  replaceOnce,
  sliceOf,
  type Model,
} from '../../sync-fixtures';

function located(model: Model, text: string) {
  return model.diagnostics.map((diagnostic) => ({
    severity: diagnostic.severity,
    code: diagnostic.code,
    message: diagnostic.message,
    line: diagnostic.line,
    column: diagnostic.column,
    source: sliceOf(text, diagnostic.range),
  }));
}

function warning(line: number, column: number, source: string) {
  return { severity: 'warning', code: 'ui-gen(visual-sync)', message: NOT_REPRESENTABLE_MESSAGE, line, column, source };
}

describe('diagnostics of a component read with the TypeScript parser', () => {
  // covers: FCT-0039
  it('warns once per non-representable expression of the template, at its line and column', () => {
    const { text, model } = readFixtureModel(FEATURE_LIST);

    expect(located(model, text)).toEqual([
      warning(22, 9, '${this.features.map((feature) => html`<li>${feature}</li>`)}'),
      warning(
        25,
        9,
        [
          '${repeat(',
          '          this.features,',
          '          (feature) => feature,',
          '          (feature, index) => html`<li>${index + 1}. ${feature}</li>`,',
          '        )}',
        ].join('\n'),
      ),
      warning(31, 10, '${formatPrice(this.price)}'),
      warning(32, 10, "${this.compact ? 'Compact' : 'Complet'}"),
      warning(33, 10, '${this.ariaLabel}'),
      warning(34, 7, '${this.renderFooter()}'),
    ]);
  });

  // covers: FCT-0039
  it('underlines exactly the range of each opaque node', () => {
    for (const name of [FEATURE_LIST, PRICING_CARD]) {
      const { model } = readFixtureModel(name);
      const opaqueRanges = allNodes(model.template).flatMap((node) => (node.kind === 'opaque' ? [node.range] : []));
      const warningRanges = model.diagnostics
        .filter((diagnostic) => diagnostic.code === 'ui-gen(visual-sync)')
        .map((diagnostic) => diagnostic.range);

      expect(opaqueRanges.length).toBeGreaterThan(0);
      for (const range of opaqueRanges) {
        expect(warningRanges).toContainEqual(range);
      }
    }
  });

  // covers: FCT-0039
  it('warns exactly on the content expressions of the mockup pricing card, not on its attribute and listener expressions', () => {
    const { text, model } = readFixtureModel(PRICING_CARD);

    expect(located(model, text)).toEqual([
      warning(148, 9, '${this.badge ? html`<hz-badge tone="accent">${this.badge}</hz-badge>` : nothing}'),
      warning(152, 11, '${formatPrice(this.price, this.locale)}'),
      warning(
        168,
        9,
        '${this.variant === \'pro\' ? html`<svg slot="end" aria-hidden="true"><use href="#arrow-right"></use></svg>` : nothing}',
      ),
      warning(170, 7, '${this.hint ? html`<hz-tooltip class="hint">${this.hint}</hz-tooltip>` : nothing}'),
    ]);
  });

  // covers: FCT-0040
  it('reports no diagnostic for expressions in attribute, boolean attribute, property or event listener position', () => {
    const text = replaceOnce(
      readFixture(PLAN_BADGE),
      '<button type="button" disabled>Choisir</button>',
      '<button type=${this.size === \'sm\' ? \'reset\' : \'button\'} ?disabled=${!this.open} .title=${formatTitle(this.label)} @click=${() => this.choose()}>Choisir</button>',
    );
    const model = read(text);

    expect(model.diagnostics).toEqual([]);
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

  // covers: FCT-0040
  it('reports no diagnostic for a directive placed on an element', () => {
    const text = replaceOnce(readFixture(PLAN_BADGE), '<strong>${this.label}</strong>', '<strong ${ref(this.labelRef)}>${this.label}</strong>');

    expect(read(text).diagnostics).toEqual([]);
  });

  // covers: FCT-0039
  it('treats an interpolation of an undeclared member as non-representable', () => {
    const text = replaceOnce(readFixture(PLAN_BADGE), '${this.label}', '${this.title}');
    const model = read(text);

    expect(located(model, text)).toEqual([warning(22, 12, '${this.title}')]);
    expect(allNodes(model.template).filter((node) => node.kind === 'binding')).toHaveLength(1);
  });

  // covers: FCT-0040
  it('reports no diagnostic for a component whose template holds only elements, text and property bindings', () => {
    const { model } = readFixtureModel(PLAN_BADGE);

    expect(model.diagnostics).toEqual([]);
  });

  // covers: FCT-0043
  it('reports a syntax error with the compiler code, message and position', () => {
    const text = replaceOnce(readFixture(PRICING_CARD), "@property() locale = 'fr-FR';", '@property() locale = ;');
    const model = read(text);

    expect(located(model, text).filter((diagnostic) => diagnostic.severity === 'error')).toEqual([
      { severity: 'error', code: 'ts(1109)', message: 'Expression attendue.', line: 120, column: 24, source: ';' },
    ]);
  });

  // covers: FCT-0043
  it('reports every syntax error of a file with an unclosed method body', () => {
    const text = replaceOnce(
      readFixture(PRICING_CARD),
      "  private hideHint() {\n    this.hint = '';\n  }",
      "  private hideHint() {\n    this.hint = '';\n",
    );
    const model = read(text);

    expect(
      located(model, text)
        .filter((diagnostic) => diagnostic.severity === 'error')
        .map(({ code, message, line, column }) => ({ code, message, line, column })),
    ).toEqual([
      { code: 'ts(1005)', message: "';' attendu.", line: 145, column: 12 },
      {
        code: 'ts(1068)',
        message: 'Jeton inattendu. Un constructeur, une méthode, un accesseur ou une propriété est attendu.',
        line: 175,
        column: 1,
      },
    ]);
  });
});
