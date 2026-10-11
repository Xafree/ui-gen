import ts from 'typescript';
import type { HighlightClassName } from '../../../ports/code-highlighter.js';
import type { HighlightContext } from './highlight-context.js';
import { highlightCss } from './highlight-css.js';
import { highlightHtml } from './highlight-html.js';
import { highlightInterpolation, matchAt, scanExpression, skipTemplate } from './template-scanning.js';

const KEYWORDS = new Set([
  'import',
  'from',
  'export',
  'const',
  'let',
  'var',
  'class',
  'extends',
  'static',
  'private',
  'public',
  'readonly',
  'return',
  'type',
  'declare',
  'global',
  'interface',
  'new',
  'this',
  'true',
  'false',
  'null',
  'undefined',
  'if',
  'else',
  'keyof',
  'typeof',
  'as',
  'void',
]);
const PRIMITIVE_TYPES = new Set(['string', 'number', 'boolean', 'unknown']);
const DECORATOR = /@[A-Za-z_]\w*/y;
const CALL_OR_TAG = /\s*\(|`/y;
const KEY_COLON = /\s*:(?!:)/y;
const CAPITALIZED = /^[A-Z]/;

interface WalkState {
  previousWord: string;
  previousSignificant: string;
}

function isWord(token: ts.SyntaxKind): boolean {
  return (
    token === ts.SyntaxKind.Identifier || (token >= ts.SyntaxKind.FirstKeyword && token <= ts.SyntaxKind.LastKeyword)
  );
}

function classifyWord(
  context: HighlightContext,
  word: string,
  end: number,
  limit: number,
  state: WalkState,
): HighlightClassName | null {
  const { text } = context;
  if (KEYWORDS.has(word)) {
    return 'tk-kw';
  }
  if (PRIMITIVE_TYPES.has(word)) {
    return 'tk-type';
  }
  if (matchAt(CALL_OR_TAG, text, end, limit) !== null) {
    return 'tk-fn';
  }
  if (state.previousSignificant === '.') {
    return 'tk-prop';
  }
  if (CAPITALIZED.test(word)) {
    return 'tk-type';
  }
  if (matchAt(KEY_COLON, text, end, limit) !== null && state.previousSignificant !== '?') {
    return 'tk-prop';
  }
  return null;
}

function highlightPlainTemplate(context: HighlightContext, start: number, end: number): void {
  const { text, sink } = context;
  let literalStart = start;
  let index = start;
  while (index < end) {
    if (text[index] === '$' && text[index + 1] === '{') {
      if (scanExpression(text, index + 2, end) < 0) {
        break;
      }
      sink.add(literalStart, index, 'tk-str');
      index = highlightInterpolation(context, index, end);
      literalStart = index;
    } else {
      index += 1;
    }
  }
  sink.add(literalStart, end, 'tk-str');
}

function highlightTemplate(context: HighlightContext, start: number, end: number, tag: string): number {
  const close = skipTemplate(context.text, start + 1, end);
  context.sink.add(start, start + 1, 'tk-punc');
  if (close < 0) {
    return start + 1;
  }
  if (tag === 'html') {
    highlightHtml(context, start + 1, close - 1);
  } else if (tag === 'css') {
    highlightCss(context, start + 1, close - 1);
  } else {
    highlightPlainTemplate(context, start + 1, close - 1);
  }
  context.sink.add(close - 1, close, 'tk-punc');
  return close;
}

export function highlightTypeScript(context: HighlightContext, start: number, end: number): void {
  const { text, sink } = context;
  const scanner = ts.createScanner(
    ts.ScriptTarget.Latest,
    false,
    ts.LanguageVariant.Standard,
    text,
    undefined,
    start,
    end - start,
  );
  const state: WalkState = { previousWord: '', previousSignificant: '' };
  let index = start;
  while (index < end) {
    if (text[index] === '`') {
      const next = highlightTemplate(context, index, end, state.previousWord);
      if (next > index + 1) {
        state.previousWord = '';
        state.previousSignificant = 's';
      }
      index = next;
      scanner.resetTokenState(index);
      continue;
    }
    const decorator = matchAt(DECORATOR, text, index, end);
    if (decorator !== null) {
      sink.add(index, index + decorator.length, 'tk-dec');
      index += decorator.length;
      scanner.resetTokenState(index);
      continue;
    }
    const token = scanner.scan();
    if (token === ts.SyntaxKind.EndOfFileToken) {
      break;
    }
    const tokenStart = scanner.getTokenStart();
    const tokenEnd = scanner.getTokenEnd();
    index = tokenEnd;
    if (token === ts.SyntaxKind.SingleLineCommentTrivia || token === ts.SyntaxKind.MultiLineCommentTrivia) {
      sink.add(tokenStart, tokenEnd, 'tk-com');
    } else if (token === ts.SyntaxKind.StringLiteral) {
      sink.add(tokenStart, tokenEnd, 'tk-str');
      state.previousSignificant = 's';
      state.previousWord = '';
    } else if (token === ts.SyntaxKind.NumericLiteral || token === ts.SyntaxKind.BigIntLiteral) {
      sink.add(tokenStart, tokenEnd, 'tk-num');
      state.previousSignificant = 'n';
    } else if (isWord(token)) {
      const word = text.slice(tokenStart, tokenEnd);
      const className = classifyWord(context, word, tokenEnd, end, state);
      if (className !== null) {
        sink.add(tokenStart, tokenEnd, className);
      }
      state.previousWord = word;
      state.previousSignificant = 'w';
    } else if (token < ts.SyntaxKind.FirstTriviaToken || token > ts.SyntaxKind.LastTriviaToken) {
      for (let offset = tokenStart; offset < tokenEnd; offset += 1) {
        sink.add(offset, offset + 1, 'tk-punc');
      }
      state.previousSignificant = text.charAt(tokenEnd - 1);
      state.previousWord = '';
    }
  }
}
