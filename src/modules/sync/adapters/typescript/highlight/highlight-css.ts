import type { HighlightClassName } from '../../../ports/code-highlighter.js';
import type { HighlightContext } from './highlight-context.js';
import { highlightInterpolation, matchAt, scanExpression, skipQuote } from './template-scanning.js';

type Mode = 'start' | 'selector' | 'value';
type TokenRule = readonly [RegExp, HighlightClassName | null];

const WHITESPACE = /\s+/y;
const DECLARATION_NAME = /[-\w]+/y;
const COLON_AFTER_SPACES = /\s*:/y;
const QUOTED = /'[^']*'|"[^"]*"/y;
const SELECTOR_RULES: readonly TokenRule[] = [
  [/@[\w-]+/y, 'tk-kw'],
  [/::?[\w-]+/y, 'tk-kw'],
  [/\.[\w-]+/y, 'tk-tag'],
  [QUOTED, 'tk-str'],
  [/[\w-]+(?==)/y, 'tk-attr'],
  [/[\w-]+/y, 'tk-tag'],
];
const VALUE_RULES: readonly TokenRule[] = [
  [/[a-z-]+(?=\()/y, 'tk-fn'],
  [/--[\w-]+/y, 'tk-var'],
  [/-?\d+(?:\.\d+)?(?:px|%|em|rem|ms|s)?/y, 'tk-num'],
  [QUOTED, 'tk-str'],
  [/[(),+*/]/y, 'tk-punc'],
  [/[a-z-]+/iy, null],
];

function nextTerminator(context: HighlightContext, start: number, end: number): string {
  const { text } = context;
  let index = start;
  while (index < end) {
    const char = text[index];
    if (char === '{' || char === '}' || char === ';') {
      return char;
    }
    if (char === "'" || char === '"') {
      index = skipQuote(text, index, end);
    } else if (char === '$' && text[index + 1] === '{') {
      const next = scanExpression(text, index + 2, end);
      if (next < 0) {
        return '';
      }
      index = next;
    } else {
      index += 1;
    }
  }
  return '';
}

function highlightRuleToken(
  context: HighlightContext,
  rules: readonly TokenRule[],
  index: number,
  end: number,
  fallback: HighlightClassName | null,
): number {
  for (const [pattern, className] of rules) {
    const matched = matchAt(pattern, context.text, index, end);
    if (matched !== null) {
      if (className !== null) {
        context.sink.add(index, index + matched.length, className);
      }
      return index + matched.length;
    }
  }
  if (fallback !== null) {
    context.sink.add(index, index + 1, fallback);
  }
  return index + 1;
}

function highlightDeclarationStart(context: HighlightContext, index: number, end: number): number | null {
  const { text, sink } = context;
  if (nextTerminator(context, index, end) === '{') {
    return null;
  }
  const name = matchAt(DECLARATION_NAME, text, index, end);
  if (name === null) {
    return null;
  }
  const colon = matchAt(COLON_AFTER_SPACES, text, index + name.length, end);
  if (colon === null) {
    return null;
  }
  sink.add(index, index + name.length, name.startsWith('--') ? 'tk-var' : 'tk-prop');
  const colonIndex = index + name.length + colon.length - 1;
  sink.add(colonIndex, colonIndex + 1, 'tk-punc');
  return colonIndex + 1;
}

function highlightBlockComment(context: HighlightContext, index: number, end: number): number {
  const close = context.text.indexOf('*/', index + 2);
  const next = close < 0 || close + 2 > end ? end : close + 2;
  context.sink.add(index, next, 'tk-com');
  return next;
}

function highlightTrivia(context: HighlightContext, index: number, end: number): number | null {
  const { text } = context;
  const whitespace = matchAt(WHITESPACE, text, index, end);
  if (whitespace !== null) {
    return index + whitespace.length;
  }
  if (text[index] === '$' && text[index + 1] === '{') {
    return highlightInterpolation(context, index, end);
  }
  return text[index] === '/' && text[index + 1] === '*' ? highlightBlockComment(context, index, end) : null;
}

function isBoundary(char: string | undefined): boolean {
  return char === '{' || char === '}' || char === ';';
}

export function highlightCss(context: HighlightContext, start: number, end: number): void {
  let mode: Mode = 'start';
  let index = start;
  while (index >= 0 && index < end) {
    const trivia = highlightTrivia(context, index, end);
    if (trivia !== null) {
      index = trivia;
    } else if (isBoundary(context.text[index])) {
      context.sink.add(index, index + 1, 'tk-punc');
      index += 1;
      mode = 'start';
    } else if (mode === 'start') {
      const afterColon = highlightDeclarationStart(context, index, end);
      mode = afterColon === null ? 'selector' : 'value';
      index = afterColon ?? index;
    } else if (mode === 'selector') {
      index = highlightRuleToken(context, SELECTOR_RULES, index, end, 'tk-punc');
    } else {
      index = highlightRuleToken(context, VALUE_RULES, index, end, null);
    }
  }
}
