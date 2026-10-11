import { defaultTreeAdapter, parseFragment, type DefaultTreeAdapterTypes, type Token } from 'parse5';
import type { ParsedTemplateNode, TemplateExpression } from '../../domain/model/template-syntax.js';
import type { TextRange } from '../../domain/model/text-range.js';
import { readAttributes } from './attribute-reader.js';
import {
  maskTemplate,
  originalText,
  overlaps,
  overlapsExpression,
  rangeOfLocation,
  type MaskedTemplate,
} from './masked-template.js';

type Parse5Parent = DefaultTreeAdapterTypes.ParentNode;
type Parse5Child = DefaultTreeAdapterTypes.ChildNode;

const PARSER_OPTIONS = { sourceCodeLocationInfo: true };
const BLANK = /^[\t\n\f\r ]*$/;

export interface HtmlTemplateSource {
  readonly text: string;
  readonly offset: number;
  readonly expressions: readonly TemplateExpression[];
}

export interface HtmlTemplate {
  readonly nodes: readonly ParsedTemplateNode[];
  readonly looseExpressions: readonly TemplateExpression[];
}

function isTemplateElement(parent: Parse5Parent): parent is DefaultTreeAdapterTypes.Template {
  return parent.nodeName === 'template';
}

function childrenOf(parent: Parse5Parent): Parse5Child[] {
  return isTemplateElement(parent)
    ? defaultTreeAdapter.getTemplateContent(parent).childNodes
    : defaultTreeAdapter.getChildNodes(parent);
}

function startTagRanges(parent: Parse5Parent, template: MaskedTemplate): TextRange[] {
  return childrenOf(parent).flatMap((child) => {
    if (!defaultTreeAdapter.isElementNode(child)) {
      return [];
    }
    const startTag = defaultTreeAdapter.getNodeSourceCodeLocation(child)?.startTag;
    const own = startTag === undefined ? [] : [rangeOfLocation(template, startTag)];
    return [...own, ...startTagRanges(child, template)];
  });
}

function textNodes(range: TextRange, template: MaskedTemplate): ParsedTemplateNode[] {
  const text = originalText(template, range);
  return BLANK.test(text) ? [] : [{ kind: 'text', range, text }];
}

function splitAtExpressions(range: TextRange, template: MaskedTemplate): ParsedTemplateNode[] {
  const nodes: ParsedTemplateNode[] = [];
  let cursor = range.start;
  for (const expression of template.expressions) {
    if (expression.range.start >= cursor && expression.range.end <= range.end) {
      nodes.push(...textNodes({ start: cursor, end: expression.range.start }, template), {
        kind: 'expression',
        expression,
      });
      cursor = expression.range.end;
    }
  }
  nodes.push(...textNodes({ start: cursor, end: range.end }, template));
  return nodes;
}

function commentExpression(range: TextRange, template: MaskedTemplate): ParsedTemplateNode[] {
  const expression = template.expressions.find(
    (candidate) => candidate.range.start === range.start && candidate.range.end === range.end,
  );
  return expression === undefined ? [] : [{ kind: 'expression', expression }];
}

function readElement(
  element: DefaultTreeAdapterTypes.Element,
  location: Token.ElementLocation,
  template: MaskedTemplate,
): ParsedTemplateNode {
  const { attributes, classes } = readAttributes(location, template);
  return {
    kind: 'element',
    tag: defaultTreeAdapter.getTagName(element),
    range: rangeOfLocation(template, location),
    attributes,
    classes,
    children: readChildren(element, template),
  };
}

function hasDynamicTagName(
  element: DefaultTreeAdapterTypes.Element,
  location: Token.Location,
  template: MaskedTemplate,
): boolean {
  const start = template.offset + location.startOffset + 1;
  return overlapsExpression(template, { start, end: start + defaultTreeAdapter.getTagName(element).length });
}

function readChild(child: Parse5Child, template: MaskedTemplate): ParsedTemplateNode[] {
  const location = defaultTreeAdapter.getNodeSourceCodeLocation(child);
  if (defaultTreeAdapter.isElementNode(child)) {
    return location && !hasDynamicTagName(child, location, template)
      ? [readElement(child, location, template)]
      : readChildren(child, template);
  }
  if (!location) {
    return [];
  }
  const range = rangeOfLocation(template, location);
  if (defaultTreeAdapter.isTextNode(child)) {
    return splitAtExpressions(range, template);
  }
  return defaultTreeAdapter.isCommentNode(child) ? commentExpression(range, template) : [];
}

function readChildren(parent: Parse5Parent, template: MaskedTemplate): ParsedTemplateNode[] {
  return childrenOf(parent).flatMap((child) => readChild(child, template));
}

function placedExpressions(nodes: readonly ParsedTemplateNode[]): TemplateExpression[] {
  return nodes.flatMap((node) => {
    switch (node.kind) {
      case 'expression':
        return [node.expression];
      case 'element':
        return placedExpressions(node.children);
      case 'text':
        return [];
    }
  });
}

export function readHtmlTemplate({ text, offset, expressions }: HtmlTemplateSource): HtmlTemplate {
  const probe = maskTemplate(text, offset, expressions, () => 'filler');
  const tags = startTagRanges(parseFragment(probe.masked, PARSER_OPTIONS), probe);
  const template = maskTemplate(text, offset, expressions, ({ range }) =>
    tags.some((tag) => overlaps(tag, range)) ? 'filler' : 'comment',
  );
  const nodes = readChildren(parseFragment(template.masked, PARSER_OPTIONS), template);
  const placed = new Set(placedExpressions(nodes));
  return { nodes, looseExpressions: expressions.filter((expression) => !placed.has(expression)) };
}
