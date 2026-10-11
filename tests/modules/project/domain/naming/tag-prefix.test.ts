import { describe, expect, it } from 'vitest';
import {
  TAG_PREFIX_PATTERN,
  suggestTagPrefix,
  tagPrefixOf,
  validateTagPrefix,
  type TagPrefixError,
} from '../../../../../src/modules/project/domain/naming/tag-prefix';

describe('suggestTagPrefix', () => {
  // covers: FCT-0002
  it('proposes the lowercase initials of the words followed by a dash', () => {
    expect(suggestTagPrefix('Kit Horizon')).toBe('kh-');
  });

  // covers: FCT-0002
  it('proposes the first two letters of a single-word name', () => {
    expect(suggestTagPrefix('Horizon')).toBe('ho-');
  });

  // covers: FCT-0002
  it('takes one initial per word for three words', () => {
    expect(suggestTagPrefix('my design kit')).toBe('mdk-');
  });

  // covers: FCT-0002
  it('ignores surrounding and repeated spaces', () => {
    expect(suggestTagPrefix('  Kit   Horizon ')).toBe('kh-');
  });

  // covers: FCT-0002
  it('proposes prefixes that pass validation', () => {
    expect(validateTagPrefix(suggestTagPrefix('Kit Horizon'))).toBeNull();
    expect(validateTagPrefix(suggestTagPrefix('Horizon'))).toBeNull();
  });

  it.each(['', '   ', '42', '2024 !'])(
    'proposes nothing for %j, which has no word starting with a letter',
    (name) => {
      expect(suggestTagPrefix(name)).toBe('');
    },
  );
});

describe('validateTagPrefix', () => {
  // covers: FCT-0005
  it.each(['kh-', 'ui2-', 'my-kit-', 'a-'])('accepts %j', (value) => {
    expect(validateTagPrefix(value)).toBeNull();
  });

  // covers: FCT-0005
  it.each<[string, TagPrefixError]>([
    ['', 'empty'],
    ['KH-', 'uppercase'],
    ['kH-', 'uppercase'],
    ['1kh-', 'not-starting-with-letter'],
    ['-kh-', 'not-starting-with-letter'],
    ['k_h-', 'invalid-characters'],
    ['k h-', 'invalid-characters'],
    ['kh.-', 'invalid-characters'],
    ['kh', 'missing-trailing-dash'],
    ['kh-button', 'missing-trailing-dash'],
    ['kh--', 'double-dash'],
    ['k--h-', 'double-dash'],
  ])('refuses %j with %s', (value, expected) => {
    expect(validateTagPrefix(value)).toBe(expected);
  });

  // covers: FCT-0005
  it.each<[string, TagPrefixError]>([
    ['1K', 'uppercase'],
    ['Kh', 'uppercase'],
    ['1k_h', 'not-starting-with-letter'],
    ['-kh', 'not-starting-with-letter'],
    ['k_h', 'invalid-characters'],
    ['k h--', 'invalid-characters'],
    ['k--h', 'missing-trailing-dash'],
  ])('reports the first error in rule order for %j: %s', (value, expected) => {
    expect(validateTagPrefix(value)).toBe(expected);
  });
});

describe('TAG_PREFIX_PATTERN', () => {
  // covers: FCT-0005
  it.each(['kh-', 'ui2-', 'my-kit-'])('matches the valid prefix %j', (value) => {
    expect(TAG_PREFIX_PATTERN.test(value)).toBe(true);
  });

  // covers: FCT-0005
  it.each(['', 'kh', 'KH-', '1kh-', '-kh-', 'k_h-', 'kh-button'])('rejects %j', (value) => {
    expect(TAG_PREFIX_PATTERN.test(value)).toBe(false);
  });
});

describe('tagPrefixOf', () => {
  it('keeps the tag start up to the first dash included', () => {
    expect(tagPrefixOf('kh-button')).toBe('kh-');
  });

  it('stops at the first dash of a multi-dash tag', () => {
    expect(tagPrefixOf('my-kit-button')).toBe('my-');
  });

  it('finds no prefix in a tag without dash', () => {
    expect(tagPrefixOf('button')).toBeNull();
  });

  it('finds no prefix when the tag starts with a dash', () => {
    expect(tagPrefixOf('-button')).toBeNull();
  });
});
