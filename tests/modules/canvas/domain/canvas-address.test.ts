import { describe, expect, it } from 'vitest';
import { readCanvasPath } from '../../../../src/modules/canvas/domain/canvas-address';

describe('reading a canvas path', () => {
  it('splits the path into decoded segments', () => {
    expect(readCanvasPath('/src/components/hz-card.ts')).toEqual(['src', 'components', 'hz-card.ts']);
    expect(readCanvasPath('/src/my%20folder/a.ts')).toEqual(['src', 'my folder', 'a.ts']);
  });

  it.each([
    ['an encoded slash', '/src/..%2f..%2fsecret.js'],
    ['an encoded backslash', '/src/..%5c..%5csecret.js'],
    ['a parent segment', '/src/%2e%2e/secret.js'],
    ['a dot segment', '/src/%2e/a.js'],
    ['a hidden file', '/.env'],
    ['an empty segment', '/src//a.js'],
    ['a trailing slash', '/src/'],
    ['a drive letter', '/C%3A/windows/win.ini'],
    ['a control character', '/src/a%00.js'],
    ['a malformed escape', '/src/%E0%A4%A.js'],
    ['no leading slash', 'src/a.js'],
  ])('refuses %s', (_label, pathname) => {
    expect(readCanvasPath(pathname)).toBeNull();
  });
});
