import type { HighlightToken } from '../model/edited-component.js';

export interface CodeSegment {
  readonly text: string;
  readonly className: string | null;
}

export interface CodeLine {
  readonly number: number;
  readonly start: number;
  readonly end: number;
  readonly text: string;
  readonly segments: readonly CodeSegment[];
}

interface Span {
  readonly start: number;
  readonly end: number;
}

const LINE_BREAK = /\r\n|[\n\r\u2028\u2029]/g;

function spansOf(text: string): Span[] {
  const spans: Span[] = [];
  let start = 0;
  for (const match of text.matchAll(LINE_BREAK)) {
    spans.push({ start, end: match.index });
    start = match.index + match[0].length;
  }
  if (start < text.length || spans.length === 0) spans.push({ start, end: text.length });
  return spans;
}

function segmentsOf(text: string, span: Span, tokens: readonly HighlightToken[], firstToken: number): CodeSegment[] {
  const segments: CodeSegment[] = [];
  let cursor = span.start;
  for (const token of tokens.slice(firstToken)) {
    if (token.start >= span.end) break;
    const from = Math.max(token.start, cursor);
    const to = Math.min(token.end, span.end);
    if (to <= from) continue;
    if (from > cursor) segments.push({ text: text.slice(cursor, from), className: null });
    segments.push({ text: text.slice(from, to), className: token.className });
    cursor = to;
  }
  if (cursor < span.end) segments.push({ text: text.slice(cursor, span.end), className: null });
  return segments;
}

export function splitCodeLines(text: string, highlight: readonly HighlightToken[]): CodeLine[] {
  const tokens = [...highlight].sort((a, b) => a.start - b.start);
  let firstToken = 0;
  return spansOf(text).map((span, index) => {
    while ((tokens[firstToken]?.end ?? span.start + 1) <= span.start) firstToken++;
    return { number: index + 1, ...span, text: text.slice(span.start, span.end), segments: segmentsOf(text, span, tokens, firstToken) };
  });
}
