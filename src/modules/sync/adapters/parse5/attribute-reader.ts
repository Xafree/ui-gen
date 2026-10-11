import type { Token } from 'parse5';
import type { TemplateAttribute } from '../../domain/model/template-node.js';
import type { TextRange } from '../../domain/model/text-range.js';
import {
  maskedText,
  originalText,
  overlapsExpression,
  rangeOfLocation,
  type MaskedTemplate,
} from './masked-template.js';

const CLASS_NAME = /[^\t\n\f\r ]+/g;

export interface ReadAttributes {
  readonly attributes: readonly TemplateAttribute[];
  readonly classes: readonly string[];
}

interface AttributeParts {
  readonly nameLength: number;
  readonly value: TextRange | null;
}

interface ReadAttribute {
  readonly attribute: TemplateAttribute;
  readonly valueRange: TextRange | null;
}

function splitAttribute(source: string): AttributeParts {
  const equals = source.indexOf('=', 1);
  if (equals === -1) {
    return { nameLength: source.length, value: null };
  }
  const nameLength = source.slice(0, equals).trimEnd().length;
  const rest = source.slice(equals + 1).trimStart();
  const restStart = source.length - rest.length;
  const quote = rest.charAt(0);
  if (quote !== '"' && quote !== "'") {
    return { nameLength, value: { start: restStart, end: source.length } };
  }
  const closed = rest.length > 1 && source.endsWith(quote);
  return { nameLength, value: { start: restStart + 1, end: closed ? source.length - 1 : source.length } };
}

function shifted(range: TextRange, by: number): TextRange {
  return { start: range.start + by, end: range.end + by };
}

function readAttribute(location: Token.Location, template: MaskedTemplate): ReadAttribute | null {
  const range = rangeOfLocation(template, location);
  const parts = splitAttribute(maskedText(template, range));
  const nameRange = { start: range.start, end: range.start + parts.nameLength };
  if (overlapsExpression(template, nameRange)) {
    return null;
  }
  const valueRange = parts.value === null ? null : shifted(parts.value, range.start);
  const value = valueRange === null ? null : originalText(template, valueRange);
  return { attribute: { name: originalText(template, nameRange), value, range }, valueRange };
}

function staticClassNames(valueRange: TextRange, template: MaskedTemplate): string[] {
  const names: string[] = [];
  for (const match of maskedText(template, valueRange).matchAll(CLASS_NAME)) {
    const start = valueRange.start + match.index;
    const range = { start, end: start + match[0].length };
    if (!overlapsExpression(template, range)) {
      names.push(originalText(template, range));
    }
  }
  return names;
}

export function readAttributes(location: Token.ElementLocation, template: MaskedTemplate): ReadAttributes {
  const read = Object.values(location.attrs ?? {})
    .toSorted((first, second) => first.startOffset - second.startOffset)
    .flatMap((attributeLocation) => {
      const item = readAttribute(attributeLocation, template);
      return item === null ? [] : [item];
    });
  const classAttribute = read.find(({ attribute }) => attribute.name.toLowerCase() === 'class');
  return {
    attributes: read.map(({ attribute }) => attribute),
    classes: classAttribute?.valueRange ? staticClassNames(classAttribute.valueRange, template) : [],
  };
}
