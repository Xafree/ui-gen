import type { HighlightContext } from './highlight-context.js';
import { highlightInterpolation, matchAt, skipQuote } from './template-scanning.js';

type HtmlStep = (context: HighlightContext, index: number, end: number) => number | null;

const TAG_OPENER = /<\/?(?=[A-Za-z])/y;
const TAG_NAME = /[A-Za-z][\w-]*/y;
const WHITESPACE = /\s+/y;
const ATTRIBUTE_NAME = /[@?.]?[A-Za-z_][\w-]*(?==)/y;
const PLAIN_RUN = /[^<$"=>\s]+/y;

const interpolationStep: HtmlStep = (context, index, end) =>
  context.text[index] === '$' && context.text[index + 1] === '{' ? highlightInterpolation(context, index, end) : null;

const tagStep: HtmlStep = (context, index, end) => {
  const opener = matchAt(TAG_OPENER, context.text, index, end);
  if (opener === null) {
    return null;
  }
  const nameStart = index + opener.length;
  const name = matchAt(TAG_NAME, context.text, nameStart, end) ?? '';
  context.sink.add(index, nameStart, 'tk-punc');
  context.sink.add(nameStart, nameStart + name.length, 'tk-tag');
  return nameStart + name.length;
};

const whitespaceStep: HtmlStep = (context, index, end) => {
  const whitespace = matchAt(WHITESPACE, context.text, index, end);
  return whitespace === null ? null : index + whitespace.length;
};

const attributeStep: HtmlStep = (context, index, end) => {
  const attribute = matchAt(ATTRIBUTE_NAME, context.text, index, end);
  if (attribute === null) {
    return null;
  }
  context.sink.add(index, index + attribute.length, 'tk-attr');
  return index + attribute.length;
};

const quotedStep: HtmlStep = (context, index, end) => {
  if (context.text[index] !== '"') {
    return null;
  }
  const next = skipQuote(context.text, index, end);
  context.sink.add(index, next, 'tk-str');
  return next;
};

const selfClosingStep: HtmlStep = (context, index) => {
  if (context.text[index] !== '/' || context.text[index + 1] !== '>') {
    return null;
  }
  context.sink.add(index, index + 2, 'tk-punc');
  return index + 2;
};

const punctuationStep: HtmlStep = (context, index) => {
  if (context.text[index] !== '>' && context.text[index] !== '=') {
    return null;
  }
  context.sink.add(index, index + 1, 'tk-punc');
  return index + 1;
};

const STEPS: readonly HtmlStep[] = [
  interpolationStep,
  tagStep,
  whitespaceStep,
  attributeStep,
  quotedStep,
  selfClosingStep,
  punctuationStep,
];

function highlightNext(context: HighlightContext, index: number, end: number): number {
  for (const step of STEPS) {
    const next = step(context, index, end);
    if (next !== null) {
      return next;
    }
  }
  return index + (matchAt(PLAIN_RUN, context.text, index, end)?.length ?? 1);
}

export function highlightHtml(context: HighlightContext, start: number, end: number): void {
  let index = start;
  while (index >= 0 && index < end) {
    index = highlightNext(context, index, end);
  }
}
