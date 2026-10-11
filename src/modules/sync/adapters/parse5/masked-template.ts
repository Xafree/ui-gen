import type { Token } from 'parse5';
import type { TemplateExpression } from '../../domain/model/template-syntax.js';
import type { TextRange } from '../../domain/model/text-range.js';

const FILLER = 'x';
const COMMENT_OPENING = '<?';
const COMMENT_CLOSING = '>';
const SHORTEST_COMMENT_MARKER = COMMENT_OPENING.length + COMMENT_CLOSING.length;

export type MarkerKind = 'filler' | 'comment';

export interface MaskedTemplate {
  readonly original: string;
  readonly masked: string;
  readonly offset: number;
  readonly expressions: readonly TemplateExpression[];
}

function marker(kind: MarkerKind, length: number): string {
  return kind === 'comment' && length >= SHORTEST_COMMENT_MARKER
    ? COMMENT_OPENING + FILLER.repeat(length - SHORTEST_COMMENT_MARKER) + COMMENT_CLOSING
    : FILLER.repeat(length);
}

export function maskTemplate(
  original: string,
  offset: number,
  expressions: readonly TemplateExpression[],
  kindOf: (expression: TemplateExpression) => MarkerKind,
): MaskedTemplate {
  let masked = '';
  let cursor = 0;
  for (const expression of expressions) {
    const start = expression.range.start - offset;
    const end = Math.min(expression.range.end - offset, original.length);
    if (start >= cursor && end >= start) {
      masked += original.slice(cursor, start) + marker(kindOf(expression), end - start);
      cursor = end;
    }
  }
  return { original, masked: masked + original.slice(cursor), offset, expressions };
}

export function originalText(template: MaskedTemplate, range: TextRange): string {
  return template.original.slice(range.start - template.offset, range.end - template.offset);
}

export function maskedText(template: MaskedTemplate, range: TextRange): string {
  return template.masked.slice(range.start - template.offset, range.end - template.offset);
}

export function rangeOfLocation(template: MaskedTemplate, location: Token.Location): TextRange {
  return { start: template.offset + location.startOffset, end: template.offset + location.endOffset };
}

export function overlaps(first: TextRange, second: TextRange): boolean {
  return first.start < second.end && second.start < first.end;
}

export function overlapsExpression(template: MaskedTemplate, range: TextRange): boolean {
  return template.expressions.some((expression) => overlaps(expression.range, range));
}
