import { describe, expect, it } from 'vitest';
import { badgeWithStyles, findRule, rulesWithSelector, sortedSummaries, summariesOf } from '../style-fixtures';
import { PRICING_CARD, findElement, read, readFixtureModel, sliceOf } from '../sync-fixtures';

function byText(left: string, right: string): number {
  return left.localeCompare(right);
}

describe('styles of a component read with the TypeScript parser', () => {
  // covers: FCT-0041
  it('projects the host declarations of the mockup pricing card on visual values with their token', () => {
    const { model } = readFixtureModel(PRICING_CARD);

    expect(summariesOf(findRule(model, ':host'))).toEqual(
      sortedSummaries([
        { key: 'direction', raw: 'column', token: null },
        { key: 'gap', raw: 'var(--hz-space-4)', token: '--hz-space-4' },
        { key: 'paddingBlock', raw: 'var(--hz-space-6)', token: '--hz-space-6' },
        { key: 'paddingInline', raw: 'var(--hz-space-6)', token: '--hz-space-6' },
        { key: 'border', raw: '1px solid var(--hz-color-border)', token: null },
        { key: 'radius', raw: 'var(--hz-radius-lg)', token: '--hz-radius-lg' },
        { key: 'fill', raw: 'var(--hz-color-bg)', token: '--hz-color-bg' },
        { key: 'color', raw: 'var(--hz-color-text)', token: '--hz-color-text' },
        { key: 'fontFamily', raw: 'var(--hz-font-sans)', token: '--hz-font-sans' },
      ]),
    );
  });

  // covers: FCT-0041
  it('projects only the visual declarations of a rule and leaves the others out', () => {
    const { model } = readFixtureModel(PRICING_CARD);

    expect(summariesOf(findRule(model, ':host([variant=\'pro\'])'))).toEqual(
      sortedSummaries([
        { key: 'gap', raw: 'var(--hz-space-5)', token: '--hz-space-5' },
        { key: 'paddingBlock', raw: 'var(--hz-space-8)', token: '--hz-space-8' },
        { key: 'paddingInline', raw: 'var(--hz-space-8)', token: '--hz-space-8' },
        { key: 'shadow', raw: 'var(--hz-shadow-md)', token: '--hz-shadow-md' },
      ]),
    );
    expect(summariesOf(findRule(model, 'header'))).toEqual([
      { key: 'gap', raw: 'var(--hz-space-1)', token: '--hz-space-1' },
    ]);
    expect(summariesOf(findRule(model, '.plan'))).toEqual(
      sortedSummaries([
        { key: 'fontSize', raw: 'var(--hz-text-lg)', token: '--hz-text-lg' },
        { key: 'fontWeight', raw: 'var(--hz-weight-bold)', token: '--hz-weight-bold' },
      ]),
    );
    expect(summariesOf(findRule(model, '.cta'))).toEqual([{ key: 'width', raw: '100%', token: null }]);
    expect(summariesOf(findRule(model, ":host([variant='pro']) .cta"))).toEqual([
      { key: 'width', raw: '100%', token: null },
    ]);
    expect(summariesOf(findRule(model, '.hint'))).toEqual([]);
  });

  // covers: FCT-0041
  it('keeps the code range of each rule and of each projected value inside it', () => {
    const { text, model } = readFixtureModel(PRICING_CARD);

    expect(model.styles.length).toBeGreaterThan(0);
    for (const rule of model.styles) {
      const source = sliceOf(text, rule.range);

      expect(source.startsWith(rule.selector)).toBe(true);
      expect(source.trimEnd().endsWith('}')).toBe(true);
      for (const value of rule.values) {
        expect(value.range.start).toBeGreaterThanOrEqual(rule.range.start);
        expect(value.range.end).toBeLessThanOrEqual(rule.range.end);
        expect(sliceOf(text, value.range)).toContain(value.raw);
      }
    }
  });

  // covers: FCT-0041
  it('lists the rules of the static styles in source order', () => {
    const { model } = readFixtureModel(PRICING_CARD);
    const selectors = model.styles.map((rule) => rule.selector);

    expect(selectors.indexOf(':host')).toBe(0);
    expect(selectors.indexOf(':host')).toBeLessThan(selectors.indexOf(":host([variant='pro'])"));
    expect(selectors.indexOf(":host([variant='pro'])")).toBeLessThan(selectors.indexOf('header'));
    expect(selectors.indexOf('header')).toBeLessThan(selectors.indexOf('.plan'));
    expect(selectors.indexOf('.cta')).toBeLessThan(selectors.indexOf(":host([variant='pro']) .cta"));
    expect(selectors.indexOf(":host([variant='pro']) .cta")).toBeLessThan(selectors.indexOf('.hint'));
  });

  // covers: FCT-0041
  it('reads the token of a var() with a fallback, and no token for a literal or a computed value', () => {
    const model = read(
      badgeWithStyles(
        [
          '.badge {',
          '\t\t\tdisplay: flex;',
          '\t\t\tflex-direction: row;',
          '\t\t\talign-items: center;',
          '\t\t\tgap: var(--hz-space-2, 8px);',
          '\t\t\tpadding-block: 4px;',
          '\t\t\tpadding-inline: var(--hz-space-3);',
          '\t\t\theight: calc(var(--hz-space-6) * 2);',
          '\t\t\tbackground-color: #fff;',
          '\t\t\tborder: none;',
          '\t\t\tbox-shadow: 0 1px 2px rgb(0 0 0 / 0.2);',
          '\t\t\tborder-radius: 999px;',
          "\t\t\tfont-family: 'Inter', sans-serif;",
          '\t\t\tfont-size: 0.875rem;',
          '\t\t\tfont-weight: 600;',
          '\t\t\tcolor: var(--hz-color-text);',
          '\t\t}',
        ].join('\n'),
      ),
    );

    expect(summariesOf(findRule(model, '.badge'))).toEqual(
      sortedSummaries([
        { key: 'direction', raw: 'row', token: null },
        { key: 'align', raw: 'center', token: null },
        { key: 'gap', raw: 'var(--hz-space-2, 8px)', token: '--hz-space-2' },
        { key: 'paddingBlock', raw: '4px', token: null },
        { key: 'paddingInline', raw: 'var(--hz-space-3)', token: '--hz-space-3' },
        { key: 'height', raw: 'calc(var(--hz-space-6) * 2)', token: null },
        { key: 'fill', raw: '#fff', token: null },
        { key: 'border', raw: 'none', token: null },
        { key: 'shadow', raw: '0 1px 2px rgb(0 0 0 / 0.2)', token: null },
        { key: 'radius', raw: '999px', token: null },
        { key: 'fontFamily', raw: "'Inter', sans-serif", token: null },
        { key: 'fontSize', raw: '0.875rem', token: null },
        { key: 'fontWeight', raw: '600', token: null },
        { key: 'color', raw: 'var(--hz-color-text)', token: '--hz-color-text' },
      ]),
    );
  });

  // covers: FCT-0041
  it('splits a two-value padding into its block and inline values', () => {
    const model = read(badgeWithStyles('.badge { padding: 4px var(--hz-space-3); }'));

    expect(summariesOf(findRule(model, '.badge'))).toEqual(
      sortedSummaries([
        { key: 'paddingBlock', raw: '4px', token: null },
        { key: 'paddingInline', raw: 'var(--hz-space-3)', token: '--hz-space-3' },
      ]),
    );
  });

  // covers: FCT-0041
  it('gives no direction to a grid or a block container', () => {
    const model = read(badgeWithStyles('.badge { display: grid; gap: 4px; }\n\t\tstrong { display: block; width: 3rem; }'));

    expect(summariesOf(findRule(model, '.badge'))).toEqual([{ key: 'gap', raw: '4px', token: null }]);
    expect(summariesOf(findRule(model, 'strong'))).toEqual([{ key: 'width', raw: '3rem', token: null }]);
  });

  // covers: FCT-0041
  it('targets the template nodes matched by a class, a tag or a descendant selector', () => {
    const { model } = readFixtureModel(PRICING_CARD);

    expect(findRule(model, 'header').targets).toEqual([findElement(model.template, 'header').id]);
    expect(findRule(model, '.plan').targets).toEqual([findElement(model.template, 'h3').id]);
    expect(findRule(model, '.period').targets).toEqual([findElement(model.template, 'span').id]);
    expect(findRule(model, '.cta').targets).toEqual([findElement(model.template, 'hz-button').id]);
    expect(findRule(model, ":host([variant='pro']) .cta").targets).toEqual([
      findElement(model.template, 'hz-button').id,
    ]);
  });

  // covers: FCT-0041
  it('targets a node placed inside a conditional branch of the template', () => {
    const { model } = readFixtureModel(PRICING_CARD);
    const tooltip = findElement(model.template, 'hz-tooltip');

    expect(tooltip.id).toMatch(/\.t\.\d+$/);
    expect(findRule(model, '.hint').targets).toEqual([tooltip.id]);
  });

  // covers: FCT-0041
  it('targets every node matched by a selector list or a descendant selector of the badge', () => {
    const model = read(
      badgeWithStyles('strong, button { color: red; }\n\t\t.badge strong { font-weight: 700; }\n\t\t.missing { color: blue; }'),
    );
    const strong = findElement(model.template, 'strong').id;
    const button = findElement(model.template, 'button').id;

    expect(findRule(model, 'strong, button').targets.toSorted(byText)).toEqual([strong, button].toSorted(byText));
    expect(findRule(model, '.badge strong').targets).toEqual([strong]);
    expect(rulesWithSelector(model, '.missing').flatMap((rule) => rule.targets)).toEqual([]);
  });
});
