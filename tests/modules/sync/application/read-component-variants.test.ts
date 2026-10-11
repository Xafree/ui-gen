import { describe, expect, it } from 'vitest';
import { badgeWithStyles, findRule } from '../style-fixtures';
import { FEATURE_LIST, PLAN_BADGE, PRICING_CARD, read, readFixtureModel, replaceOnce } from '../sync-fixtures';

const BADGE_SIZE_UNION = "size: 'sm' | 'md' | 'lg' = 'md'";

function badgeWithUntypedSize(css: string): string {
  return replaceOnce(badgeWithStyles(css), BADGE_SIZE_UNION, "size = 'md'");
}

describe('variants of a component read with the TypeScript parser', () => {
  // covers: FCT-0030
  it('takes the property typed by a type alias of string literals declared in the file as the variant', () => {
    const { model } = readFixtureModel(PRICING_CARD);

    expect(model.variantProperty).toBe('variant');
    expect(model.variants).toEqual(['starter', 'pro', 'equipe']);
  });

  // covers: FCT-0030
  it('takes the property typed by an inline union of string literals as the variant, in declaration order', () => {
    const { model } = readFixtureModel(PLAN_BADGE);

    expect(model.variantProperty).toBe('size');
    expect(model.variants).toEqual(['sm', 'md', 'lg']);
  });

  // covers: FCT-0030
  it('resolves a type alias that is not exported', () => {
    const text = replaceOnce(
      replaceOnce(readFixtureModel(PLAN_BADGE).text, BADGE_SIZE_UNION, "size: Size = 'md'"),
      '/* Les tailles suivent',
      "type Size = 'xs' | 'xl';\n\n/* Les tailles suivent",
    );
    const model = read(text);

    expect(model.variantProperty).toBe('size');
    expect(model.variants).toEqual(['xs', 'xl']);
  });

  // covers: FCT-0030
  it('has no variant property and no variant for a component without union nor host attribute selector', () => {
    const list = readFixtureModel(FEATURE_LIST).model;
    const pricing = readFixtureModel(PRICING_CARD).model;

    expect([list.variantProperty, pricing.variantProperty]).toEqual([null, 'variant']);
    expect([list.variants, pricing.variants]).toEqual([[], ['starter', 'pro', 'equipe']]);
  });

  // covers: FCT-0030
  it('has no variant when the property is typed string and no host attribute selector targets it', () => {
    const text = replaceOnce(
      badgeWithStyles('.badge { font-size: 12px; }\n\t\t:host([disabled]) { color: gray; }'),
      BADGE_SIZE_UNION,
      "size: string = 'md'",
    );
    const model = read(text);

    expect(model.variantProperty).toBeNull();
    expect(model.variants).toEqual([]);
    expect(findRule(model, '.badge').variant).toBeNull();
  });

  // covers: FCT-0030
  it('takes a property targeted by :host attribute selectors with double or single quotes as the variant', () => {
    const model = read(
      badgeWithUntypedSize(':host([size="sm"]) { font-size: 12px; }\n\t\t:host([size=\'lg\']) { font-size: 16px; }'),
    );

    expect(model.variantProperty).toBe('size');
    expect(model.variants).toEqual(['sm', 'lg']);
  });

  // covers: FCT-0030
  it('lists a value targeted by several :host attribute selectors once', () => {
    const model = read(
      badgeWithUntypedSize(
        ':host([size="sm"]) { font-size: 12px; }\n\t\t:host([size="sm"]) strong { font-weight: 400; }\n\t\t:host([size="lg"]) { font-size: 16px; }',
      ),
    );

    expect(model.variantProperty).toBe('size');
    expect(model.variants).toEqual(['sm', 'lg']);
  });

  // covers: FCT-0030
  it('marks the rules of a variant with its property and value, and the other rules with no variant', () => {
    const { model } = readFixtureModel(PRICING_CARD);

    expect(findRule(model, ":host([variant='pro'])").variant).toEqual({ property: 'variant', value: 'pro' });
    expect(findRule(model, ":host([variant='pro']) .cta").variant).toEqual({ property: 'variant', value: 'pro' });
    expect(findRule(model, ':host').variant).toBeNull();
    expect(findRule(model, '.cta').variant).toBeNull();
    expect(findRule(model, '.plan').variant).toBeNull();
  });

  // covers: FCT-0030
  it('marks a rule targeted by a double-quoted host attribute selector with its variant', () => {
    const model = read(badgeWithUntypedSize(':host([size="lg"]) .badge { font-size: 16px; }'));

    expect(findRule(model, ':host([size="lg"]) .badge').variant).toEqual({ property: 'size', value: 'lg' });
  });
});
