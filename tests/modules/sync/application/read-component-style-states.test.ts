import { describe, expect, it } from 'vitest';
import { badgeWithStyles, findRule, summariesOf } from '../style-fixtures';
import { PRICING_CARD, findElement, read, readFixtureModel } from '../sync-fixtures';

const STATE_RULES = [
  '.badge { color: black; }',
  '\t\t.badge:hover { background: var(--hz-color-hover); }',
  '\t\tbutton:focus-visible { color: var(--hz-color-focus); }',
  '\t\tbutton:focus { color: blue; }',
  '\t\tbutton:active { color: navy; }',
  '\t\tbutton[disabled] { color: gray; }',
  '\t\t:host([disabled]) strong { color: silver; }',
].join('\n');

describe('states of the style rules of a component', () => {
  // covers: FCT-0041
  it('reads hover, focus, active and disabled states from the selector, and no state otherwise', () => {
    const model = read(badgeWithStyles(STATE_RULES));

    expect(findRule(model, '.badge').state).toBeNull();
    expect(findRule(model, '.badge:hover').state).toBe('hover');
    expect(findRule(model, 'button:focus-visible').state).toBe('focus');
    expect(findRule(model, 'button:focus').state).toBe('focus');
    expect(findRule(model, 'button:active').state).toBe('active');
    expect(findRule(model, 'button[disabled]').state).toBe('disabled');
    expect(findRule(model, ':host([disabled]) strong').state).toBe('disabled');
  });

  // covers: FCT-0041
  it('targets the element the state applies to and keeps its values', () => {
    const model = read(badgeWithStyles(STATE_RULES));
    const badge = findElement(model.template, 'span').id;
    const button = findElement(model.template, 'button').id;
    const strong = findElement(model.template, 'strong').id;

    expect(findRule(model, '.badge:hover').targets).toEqual([badge]);
    expect(findRule(model, 'button:focus-visible').targets).toEqual([button]);
    expect(findRule(model, 'button[disabled]').targets).toEqual([button]);
    expect(findRule(model, ':host([disabled]) strong').targets).toEqual([strong]);
    expect(summariesOf(findRule(model, '.badge:hover'))).toEqual([
      { key: 'fill', raw: 'var(--hz-color-hover)', token: '--hz-color-hover' },
    ]);
  });

  // covers: FCT-0041
  it('does not take a host attribute without value as a variant', () => {
    const model = read(badgeWithStyles(STATE_RULES));

    expect(findRule(model, ':host([disabled]) strong').variant).toBeNull();
    expect(model.variants).toEqual(['sm', 'md', 'lg']);
  });

  // covers: FCT-0041
  it('gives no state to the rules of the mockup pricing card', () => {
    const { model } = readFixtureModel(PRICING_CARD);

    expect(model.styles.length).toBeGreaterThanOrEqual(11);
    expect(model.styles.map((rule) => rule.state).filter((state) => state !== null)).toEqual([]);
  });
});
