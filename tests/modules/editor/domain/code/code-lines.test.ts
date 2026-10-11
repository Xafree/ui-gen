import { describe, expect, it } from 'vitest';
import { splitCodeLines } from '../../../../../src/modules/editor/domain/code/code-lines.js';

describe('splitCodeLines', () => {
  it('drops the phantom line after the final line break', () => {
    expect(splitCodeLines('a\nb\n', []).map((line) => line.text)).toEqual(['a', 'b']);
  });

  it('keeps one empty line for an empty file', () => {
    expect(splitCodeLines('', []).map((line) => line.text)).toEqual(['']);
  });

  it('keeps carriage returns out of the lines and offsets in the original text', () => {
    const lines = splitCodeLines('ab\r\ncd\r\n', []);

    expect(lines.map((line) => [line.text, line.start, line.end])).toEqual([
      ['ab', 0, 2],
      ['cd', 4, 6],
    ]);
  });

  it('cuts a token that spans several lines at every line break', () => {
    const lines = splitCodeLines('x /* a\nb */ y', [{ start: 2, end: 11, className: 'tk-com' }]);

    expect(lines.map((line) => line.segments)).toEqual([
      [
        { text: 'x ', className: null },
        { text: '/* a', className: 'tk-com' },
      ],
      [
        { text: 'b */', className: 'tk-com' },
        { text: ' y', className: null },
      ],
    ]);
  });

  it('ignores tokens that are empty or lie outside the text', () => {
    const lines = splitCodeLines('abc', [
      { start: 1, end: 1, className: 'tk-kw' },
      { start: 10, end: 12, className: 'tk-kw' },
    ]);

    expect(lines[0]?.segments).toEqual([{ text: 'abc', className: null }]);
  });
});
