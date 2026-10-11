import { describe, expect, it } from 'vitest';
import {
  compareSummaries,
  componentDisplayName,
  samePath,
} from '../../../../../src/modules/sync/domain/library/component-summary';

describe('component display name', () => {
  it.each([
    ['hz-plan-badge', 'hz-', 'Plan Badge'],
    ['hz-pricing-card', 'hz-', 'Pricing Card'],
    ['at-button', 'hz-', 'At Button'],
    ['hz-', 'hz-', 'Hz'],
    ['x-y', '', 'X Y'],
  ])('names %s with prefix %s as %s', (tag, prefix, expected) => {
    expect(componentDisplayName(tag, prefix)).toBe(expected);
  });

  it('never returns an empty name', () => {
    expect(componentDisplayName('-', '')).toBe('-');
  });
});

// covers: FCT-0026
describe('component order', () => {
  it('sorts by tag then by path', () => {
    const summaries = [
      { path: 'b.ts', tag: 'hz-b', name: 'B' },
      { path: 'z.ts', tag: 'hz-a', name: 'A' },
      { path: 'a.ts', tag: 'hz-a', name: 'A' },
    ];

    expect(summaries.toSorted(compareSummaries).map(({ path }) => path)).toEqual(['a.ts', 'z.ts', 'b.ts']);
    const same = { path: 'a.ts', tag: 't', name: 'T' };
    expect(compareSummaries(same, { ...same })).toBe(0);
  });
});

describe('same path', () => {
  it('ignores the case', () => {
    expect(samePath('C:\\Kit\\A.ts', 'c:\\kit\\a.ts')).toBe(true);
    expect(samePath('C:\\Kit\\A.ts', 'C:\\Kit\\B.ts')).toBe(false);
  });
});
