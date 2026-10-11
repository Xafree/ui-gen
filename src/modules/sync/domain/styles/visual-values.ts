import type { VisualKey, VisualValue } from '../model/component-model.js';
import type { TextRange } from '../model/text-range.js';
import { isWhitespace, splitTopLevel, type Segment } from '../rules/top-level-split.js';
import type { ParsedDeclaration } from './style-syntax.js';

const DIRECT_KEYS: ReadonlyMap<string, VisualKey> = new Map([
  ['width', 'width'],
  ['height', 'height'],
  ['gap', 'gap'],
  ['padding-block', 'paddingBlock'],
  ['padding-inline', 'paddingInline'],
  ['align-items', 'align'],
  ['background', 'fill'],
  ['background-color', 'fill'],
  ['border-radius', 'radius'],
  ['border', 'border'],
  ['box-shadow', 'shadow'],
  ['font-family', 'fontFamily'],
  ['font-size', 'fontSize'],
  ['font-weight', 'fontWeight'],
  ['color', 'color'],
]);

const FLEX_DISPLAYS: ReadonlySet<string> = new Set(['flex', 'inline-flex']);
const SINGLE_VAR_CALL = /^var\(\s*(--[^\s,)]+)\s*(?:,.*)?\)$/is;
const MAX_PADDING_SIDES = 4;

function tokenOf(raw: string): string | null {
  const isSingleComponent = splitTopLevel(raw, isWhitespace).length === 1;
  return (isSingleComponent ? SINGLE_VAR_CALL.exec(raw)?.[1] : undefined) ?? null;
}

function visualValue(key: VisualKey, raw: string, range: TextRange): VisualValue {
  return { key, raw, token: tokenOf(raw), range };
}

function sideValue(key: VisualKey, side: Segment, valueStart: number): VisualValue {
  const start = valueStart + side.start;
  return visualValue(key, side.text, { start, end: start + side.text.length });
}

function paddingValues({ value, valueRange }: ParsedDeclaration): VisualValue[] {
  const sides = splitTopLevel(value, isWhitespace);
  const [top, right, bottom, left] = sides;
  if (top === undefined || sides.length > MAX_PADDING_SIDES) {
    return [];
  }
  const values: VisualValue[] = [];
  if (top.text === (bottom ?? top).text) {
    values.push(sideValue('paddingBlock', top, valueRange.start));
  }
  if ((right ?? top).text === (left ?? right ?? top).text) {
    values.push(sideValue('paddingInline', right ?? top, valueRange.start));
  }
  return values;
}

function declarationValues(declaration: ParsedDeclaration): VisualValue[] {
  const property = declaration.property.toLowerCase();
  if (property === 'padding') {
    return paddingValues(declaration);
  }
  const key = DIRECT_KEYS.get(property);
  return key === undefined ? [] : [visualValue(key, declaration.value, declaration.valueRange)];
}

function lastDeclaration(declarations: readonly ParsedDeclaration[], property: string): ParsedDeclaration | undefined {
  return declarations.findLast((declaration) => declaration.property.toLowerCase() === property);
}

function directionValues(declarations: readonly ParsedDeclaration[]): VisualValue[] {
  const display = lastDeclaration(declarations, 'display');
  const direction = lastDeclaration(declarations, 'flex-direction');
  const isFlex = display !== undefined && FLEX_DISPLAYS.has(display.value.toLowerCase());
  return isFlex && direction !== undefined ? [visualValue('direction', direction.value, direction.valueRange)] : [];
}

export function projectDeclarations(declarations: readonly ParsedDeclaration[]): VisualValue[] {
  const effective = new Map<VisualKey, VisualValue>();
  for (const value of [...declarations.flatMap(declarationValues), ...directionValues(declarations)]) {
    effective.delete(value.key);
    effective.set(value.key, value);
  }
  return [...effective.values()];
}
