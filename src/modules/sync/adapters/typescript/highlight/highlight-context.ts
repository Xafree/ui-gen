import type { HighlightClassName, HighlightToken } from '../../../ports/code-highlighter.js';

export interface TokenSink {
  add(start: number, end: number, className: HighlightClassName): void;
  tokens(): HighlightToken[];
}

export interface HighlightContext {
  readonly text: string;
  readonly sink: TokenSink;
  highlightExpression(start: number, end: number): void;
}

export function createTokenSink(text: string): TokenSink {
  const collected: HighlightToken[] = [];
  let lastEnd = 0;
  return {
    add(start, end, className) {
      if (start < lastEnd || end <= start || text.slice(start, end).trim() === '') {
        return;
      }
      collected.push({ start, end, className });
      lastEnd = end;
    },
    tokens: () => collected,
  };
}
