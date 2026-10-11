import { CssSyntaxError, parse, type ChildNode, type Declaration, type Root, type Rule } from 'postcss';
import type { TextRange } from '../../domain/model/text-range.js';
import type {
  ParsedDeclaration,
  ParsedStyleItem,
  ParsedStyleRule,
  ParsedStyleSheet,
} from '../../domain/styles/style-syntax.js';

export interface CssTemplateSource {
  readonly text: string;
  readonly offset: number;
  readonly interpolations: readonly TextRange[];
}

const FILLER = 'x';
const IMPORTANT_FLAG = /!\s*important$/i;

function maskInterpolations({ text, offset, interpolations }: CssTemplateSource): string {
  let masked = '';
  let cursor = 0;
  for (const { start, end } of interpolations) {
    masked += text.slice(cursor, start - offset) + FILLER.repeat(end - start);
    cursor = end - offset;
  }
  return masked + text.slice(cursor);
}

function parseMasked(source: CssTemplateSource): Root | null {
  try {
    return parse(maskInterpolations(source));
  } catch (error) {
    if (error instanceof CssSyntaxError) {
      return null;
    }
    throw error;
  }
}

function valueWithoutTrailer(afterColon: string): string {
  const closed = afterColon.trimEnd();
  const unterminated = closed.endsWith(';') ? closed.slice(0, -1) : closed;
  return unterminated.trimEnd().replace(IMPORTANT_FLAG, '').trim();
}

function readDeclaration(declaration: Declaration, { text, offset }: CssTemplateSource): ParsedDeclaration | null {
  const { start, end } = declaration.rangeBy();
  const afterColon = text.slice(start.offset, end.offset).replace(/^[^:]*:/, '');
  const value = valueWithoutTrailer(afterColon);
  if (value === '') {
    return null;
  }
  const valueStart = end.offset - afterColon.length + (afterColon.length - afterColon.trimStart().length);
  return {
    property: declaration.prop,
    value,
    valueRange: { start: offset + valueStart, end: offset + valueStart + value.length },
  };
}

function readRule(rule: Rule, range: TextRange, source: CssTemplateSource): ParsedStyleRule {
  return {
    kind: 'rule',
    selector: rule.selector,
    range,
    declarations: rule.nodes.flatMap((node) => {
      const declaration = node.type === 'decl' ? readDeclaration(node, source) : null;
      return declaration === null ? [] : [declaration];
    }),
  };
}

function isPlainRule(node: ChildNode): node is Rule {
  return node.type === 'rule' && node.nodes.every(({ type }) => type === 'decl' || type === 'comment');
}

function overlapsInterpolation(range: TextRange, { interpolations }: CssTemplateSource): boolean {
  return interpolations.some(({ start, end }) => start < range.end && range.start < end);
}

function readNode(node: ChildNode, source: CssTemplateSource): ParsedStyleItem[] {
  if (node.type === 'comment') {
    return [];
  }
  const { start, end } = node.rangeBy();
  const range = { start: source.offset + start.offset, end: source.offset + end.offset };
  if (isPlainRule(node) && !overlapsInterpolation(range, source)) {
    return [readRule(node, range, source)];
  }
  return [{ kind: 'unprojectable', range }];
}

export function readCssTemplate(source: CssTemplateSource): ParsedStyleSheet {
  const root = parseMasked(source);
  if (root === null) {
    return [{ kind: 'unprojectable', range: { start: source.offset, end: source.offset + source.text.length } }];
  }
  return root.nodes.flatMap((node) => readNode(node, source));
}
