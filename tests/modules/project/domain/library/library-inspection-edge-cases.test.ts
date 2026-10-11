import { describe, expect, it } from 'vitest';
import { findDeclaredTags } from '../../../../../src/modules/project/domain/library/declared-tags';
import { detectMajorityTagPrefix } from '../../../../../src/modules/project/domain/library/majority-prefix';

describe('findDeclaredTags on reformatted sources', () => {
  // covers: FCT-0007
  it.each([
    ['a space after the parenthesis', "customElements.define( 'hz-card', HzCard);"],
    ['a decorator with spaces around the name', "@customElement( 'hz-card' )"],
    ['a call split over several lines', ['customElements.define(', "  'hz-card',", '  HzCard,', ');'].join('\n')],
  ])('finds a tag declared with %s', (_label, source) => {
    expect(findDeclaredTags(source)).toEqual(['hz-card']);
  });

  // covers: FCT-0007
  it('ignores a tag name built from a template expression', () => {
    expect(findDeclaredTags(`customElements.define(\`hz-\${name}\`, Dynamic);`)).toEqual([]);
  });
});

describe('detectMajorityTagPrefix with names that have no prefix', () => {
  // covers: FCT-0007
  it.each([
    ['without a hyphen', ['button', 'card', 'hz-chip']],
    ['starting with a hyphen', ['-button', '-card', 'hz-chip']],
  ])('ignores names %s even when they outnumber the prefixed ones', (_label, tags) => {
    expect(detectMajorityTagPrefix(tags)).toBe('hz-');
  });

  // covers: FCT-0007
  it('returns null when no name has a prefix', () => {
    expect(detectMajorityTagPrefix(['button', '-card', ''])).toBeNull();
  });
});
