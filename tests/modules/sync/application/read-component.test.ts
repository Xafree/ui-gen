import { createHash } from 'node:crypto';
import { describe, expect, it } from 'vitest';
import {
  ALL_FIXTURES,
  FEATURE_LIST,
  PLAN_BADGE,
  PRICING_CARD,
  findProperty,
  read,
  readFixture,
  readFixtureModel,
  replaceOnce,
} from '../sync-fixtures';

function sha256(text: string): string {
  return createHash('sha256').update(text, 'utf8').digest('hex');
}

describe('readComponent', () => {
  it('names the custom element declared by @customElement and its class', () => {
    const { model } = readFixtureModel(PRICING_CARD);

    expect(model.tag).toBe('hz-pricing-card');
    expect(model.className).toBe('HzPricingCard');
  });

  it('names the custom element registered by customElements.define and its class', () => {
    const { model } = readFixtureModel(PLAN_BADGE);

    expect(model.tag).toBe('hz-plan-badge');
    expect(model.className).toBe('HzPlanBadge');
  });

  it('versions the model with the sha-256 hex digest of the file text', () => {
    for (const name of ALL_FIXTURES) {
      const text = readFixture(name);
      const { version } = read(text);

      expect(version).toMatch(/^[0-9a-f]{64}$/);
      expect(version).toBe(sha256(text));
    }
  });

  it('gives the same version to the same text and a new version to any change, even whitespace', () => {
    const text = readFixture(PLAN_BADGE);

    expect(read(text).version).toBe(read(text).version);
    expect(read(`${text}\n`).version).not.toBe(read(text).version);
    expect(read(text.replaceAll('\n', '\r\n')).version).not.toBe(read(text).version);
  });

  it('lists the @property and @state declarations in source order with their decorator', () => {
    const { model } = readFixtureModel(PRICING_CARD);

    expect(model.properties.map(({ name, decorator }) => ({ name, decorator }))).toEqual([
      { name: 'variant', decorator: 'property' },
      { name: 'plan', decorator: 'property' },
      { name: 'description', decorator: 'property' },
      { name: 'price', decorator: 'property' },
      { name: 'locale', decorator: 'property' },
      { name: 'period', decorator: 'property' },
      { name: 'badge', decorator: 'property' },
      { name: 'cta', decorator: 'property' },
      { name: 'disabled', decorator: 'property' },
      { name: 'hint', decorator: 'state' },
    ]);
  });

  it('ignores class members that carry neither @property nor @state', () => {
    const { model } = readFixtureModel(FEATURE_LIST);

    expect(model.properties.map((property) => property.name)).toEqual(['features', 'price', 'compact', 'heading']);
  });

  it('reads the annotated type of a declaration', () => {
    const pricing = readFixtureModel(PRICING_CARD).model;
    const badge = readFixtureModel(PLAN_BADGE).model;
    const list = readFixtureModel(FEATURE_LIST).model;

    expect(findProperty(pricing, 'variant').type).toBe('Plan');
    expect(findProperty(badge, 'size').type).toBe("'sm' | 'md' | 'lg'");
    expect(findProperty(badge, 'note').type).toBe('string');
    expect(findProperty(list, 'features').type).toBe('string[]');
  });

  it('reads the literal values of a union of string literals, in order', () => {
    const badge = readFixtureModel(PLAN_BADGE).model;

    expect(findProperty(badge, 'size').literalValues).toEqual(['sm', 'md', 'lg']);
    expect(findProperty(badge, 'note').literalValues).toEqual([]);
    expect(findProperty(badge, 'label').literalValues).toEqual([]);
  });

  it('reads the default value of a declaration, and null without initializer', () => {
    const pricing = readFixtureModel(PRICING_CARD).model;
    const badge = readFixtureModel(PLAN_BADGE).model;

    expect(findProperty(pricing, 'price').defaultValue).toBe('0');
    expect(findProperty(pricing, 'disabled').defaultValue).toBe('false');
    expect(findProperty(badge, 'count').defaultValue).toBe('0');
    expect(findProperty(badge, 'open').defaultValue).toBe('false');
    expect(findProperty(badge, 'open').decorator).toBe('state');
    expect(findProperty(badge, 'note').defaultValue).toBeNull();
  });

  it('reads a declaration whose decorator sits on the line above', () => {
    const badge = readFixtureModel(PLAN_BADGE).model;

    expect(badge.properties.map((property) => property.name)).toEqual(['size', 'label', 'count', 'open', 'note']);
  });

  it('reads a component whose file only changed in comments to the same tree', () => {
    const text = readFixture(PRICING_CARD);
    const commented = replaceOnce(text, '  render() {\n', '  // Rendu de la carte.\n  render() {\n');

    expect(read(commented).properties.map((property) => property.name)).toEqual(
      read(text).properties.map((property) => property.name),
    );
    expect(read(commented).template.map((node) => node.id)).toEqual(read(text).template.map((node) => node.id));
  });
});
