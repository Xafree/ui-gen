import { describe, expect, it } from 'vitest';
import { detectMajorityTagPrefix } from '../../../../../src/modules/project/domain/library/majority-prefix';

describe('detectMajorityTagPrefix', () => {
  // covers: FCT-0007
  it('returns the prefix shared by every tag', () => {
    expect(detectMajorityTagPrefix(['hz-button', 'hz-card', 'hz-chip'])).toBe('hz-');
  });

  // covers: FCT-0007
  it('returns the most used prefix', () => {
    expect(detectMajorityTagPrefix(['x-dialog', 'hz-button', 'acme-tab', 'hz-card'])).toBe('hz-');
  });

  // covers: FCT-0007
  it('cuts the prefix at the first hyphen', () => {
    expect(detectMajorityTagPrefix(['my-big-button', 'my-big-card', 'my-small-chip'])).toBe('my-');
  });

  // covers: FCT-0007
  it('returns the prefix of a single tag', () => {
    expect(detectMajorityTagPrefix(['acme-dialog'])).toBe('acme-');
  });

  // covers: FCT-0007
  it('breaks a tie with the alphabetically first prefix', () => {
    expect(detectMajorityTagPrefix(['zz-a', 'bb-a', 'zz-b', 'bb-b'])).toBe('bb-');
    expect(detectMajorityTagPrefix(['ui-x', 'ab-y'])).toBe('ab-');
  });

  // covers: FCT-0007
  it('counts duplicate tags', () => {
    expect(detectMajorityTagPrefix(['hz-tab', 'hz-tab', 'aa-x', 'aa-y'])).toBe('aa-');
    expect(detectMajorityTagPrefix(['hz-tab', 'hz-tab', 'hz-tab', 'aa-x', 'aa-y'])).toBe('hz-');
  });

  // covers: FCT-0007
  it('returns null without any tag', () => {
    expect(detectMajorityTagPrefix([])).toBeNull();
  });
});
