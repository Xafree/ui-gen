import { describe, expect, it } from 'vitest';
import {
  componentLabel,
  filterComponents,
  matchesSearch,
} from '../../../../../src/modules/editor/domain/library/component-label.js';

const FEATURE_LIST = { tag: 'hz-feature-list', name: 'Feature List' };
const PLAN_BADGE = { tag: 'hz-plan-badge', name: 'Plan Badge' };
const PRICING_CARD = { tag: 'hz-pricing-card', name: 'Pricing Card' };
const LIBRARY = [FEATURE_LIST, PLAN_BADGE, PRICING_CARD];

describe('component labels', () => {
  // covers: FCT-0028
  it('joins the words of the name in PascalCase', () => {
    expect(componentLabel(PRICING_CARD)).toBe('PricingCard');
    expect(componentLabel({ tag: 'hz-badge', name: 'Badge' })).toBe('Badge');
  });
});

describe('component search', () => {
  // covers: FCT-0029
  it('keeps the components whose label contains the search, whatever its case', () => {
    expect(filterComponents(LIBRARY, 'PRICING')).toEqual([PRICING_CARD]);
    expect(filterComponents(LIBRARY, 'badge')).toEqual([PLAN_BADGE]);
    expect(filterComponents(LIBRARY, 'cardp')).toEqual([]);
  });

  // covers: FCT-0029
  it('keeps the components whose tag contains the search', () => {
    expect(filterComponents(LIBRARY, 'hz-feat')).toEqual([FEATURE_LIST]);
    expect(filterComponents(LIBRARY, 'HZ-P')).toEqual([PLAN_BADGE, PRICING_CARD]);
  });

  // covers: FCT-0029
  it('keeps everything for an empty or blank search, and nothing for an unknown name', () => {
    expect(filterComponents(LIBRARY, '')).toEqual(LIBRARY);
    expect(filterComponents(LIBRARY, '   ')).toEqual(LIBRARY);
    expect(filterComponents(LIBRARY, 'Switch')).toEqual([]);
    expect(matchesSearch(FEATURE_LIST, ' feature ')).toBe(true);
  });
});
