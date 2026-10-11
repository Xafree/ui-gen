import type { HighlightContext } from './highlight-context.js';
import { highlightInterpolation, matchAt, skipQuote } from './template-scanning.js';

const TAG_OPENER = /<\/?(?=[A-Za-z])/y;
const TAG_NAME = /[A-Za-z][\w-]*/y;
const WHITESPACE = /\s+/y;
const ATTRIBUTE_NAME = /[@?.]?[A-Za-z_][\w-]*(?==)/y;
const PLAIN_RUN = /[^<$"=>\s]+/y;

export function highlightHtml(context: HighlightContext, start: number, end: number): void {
  const { text, sink } = context;
  let index = start;
  while (index < end) {
    const char = text[index];
    if (char === '$' && text[index + 1] === '{') {
      const next = highlightInterpolation(context, index, end);
      if (next < 0) {
        return;
      }
      index = next;
      continue;
    }
    const opener = matchAt(TAG_OPENER, text, index, end);
    if (opener !== null) {
      sink.add(index, index + opener.length, 'tk-punc');
      index += opener.length;
      const name = matchAt(TAG_NAME, text, index, end) ?? '';
      sink.add(index, index + name.length, 'tk-tag');
      index += name.length;
      continue;
    }
    const whitespace = matchAt(WHITESPACE, text, index, end);
    if (whitespace !== null) {
      index += whitespace.length;
      continue;
    }
    const attribute = matchAt(ATTRIBUTE_NAME, text, index, end);
    if (attribute !== null) {
      sink.add(index, index + attribute.length, 'tk-attr');
      index += attribute.length;
      continue;
    }
    if (char === '"') {
      const next = skipQuote(text, index, end);
      sink.add(index, next, 'tk-str');
      index = next;
      continue;
    }
    if (char === '/' && text[index + 1] === '>') {
      sink.add(index, index + 2, 'tk-punc');
      index += 2;
      continue;
    }
    if (char === '>' || char === '=') {
      sink.add(index, index + 1, 'tk-punc');
      index += 1;
      continue;
    }
    index += matchAt(PLAIN_RUN, text, index, end)?.length ?? 1;
  }
}
