import type { HighlightContext } from './highlight-context.js';

export function matchAt(pattern: RegExp, text: string, index: number, end: number): string | null {
  pattern.lastIndex = index;
  const match = pattern.exec(text);
  if (match === null) {
    return null;
  }
  return index + match[0].length > end ? match[0].slice(0, end - index) : match[0];
}

export function skipQuote(text: string, index: number, end: number): number {
  const quote = text[index];
  let cursor = index + 1;
  while (cursor < end && text[cursor] !== quote) {
    cursor += text[cursor] === '\\' ? 2 : 1;
  }
  return Math.min(cursor + 1, end);
}

export function skipTemplate(text: string, index: number, end: number): number {
  let cursor = index;
  while (cursor < end) {
    if (text[cursor] === '\\') {
      cursor += 2;
    } else if (text[cursor] === '`') {
      return cursor + 1;
    } else if (text[cursor] === '$' && text[cursor + 1] === '{') {
      cursor = scanExpression(text, cursor + 2, end);
      if (cursor < 0) {
        return -1;
      }
    } else {
      cursor += 1;
    }
  }
  return -1;
}

export function scanExpression(text: string, index: number, end: number): number {
  let depth = 1;
  let cursor = index;
  while (cursor < end) {
    const char = text[cursor];
    if (char === "'" || char === '"') {
      cursor = skipQuote(text, cursor, end);
    } else if (char === '`') {
      cursor = skipTemplate(text, cursor + 1, end);
      if (cursor < 0) {
        return -1;
      }
    } else if (char === '{') {
      depth += 1;
      cursor += 1;
    } else if (char === '}') {
      depth -= 1;
      cursor += 1;
      if (depth === 0) {
        return cursor;
      }
    } else {
      cursor += 1;
    }
  }
  return -1;
}

export function highlightInterpolation(context: HighlightContext, index: number, end: number): number {
  const close = scanExpression(context.text, index + 2, end);
  if (close < 0) {
    return -1;
  }
  context.sink.add(index, index + 2, 'tk-punc');
  context.highlightExpression(index + 2, close - 1);
  context.sink.add(close - 1, close, 'tk-punc');
  return close;
}
