import { describe, expect, it } from 'vitest';
import { contentTypeOf, extensionOf } from '../../../../src/modules/canvas/domain/content-type';

describe('content types of served files', () => {
  it.each([
    ['a.ts', 'text/javascript; charset=utf-8'],
    ['a.JS', 'text/javascript; charset=utf-8'],
    ['tokens.css', 'text/css; charset=utf-8'],
    ['logo.svg', 'image/svg+xml'],
    ['font.woff2', 'font/woff2'],
  ])('knows %s', (name, expected) => {
    expect(contentTypeOf(name)).toBe(expected);
  });

  it('does not serve an unknown or missing extension', () => {
    expect(contentTypeOf('notes.html')).toBeNull();
    expect(contentTypeOf('LICENSE')).toBeNull();
    expect(extensionOf('LICENSE')).toBe('');
  });
});
