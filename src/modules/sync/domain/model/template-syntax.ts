import type { TemplateAttribute } from './template-node.js';
import type { TextRange } from './text-range.js';

export type ExpressionShape =
  | { readonly kind: 'this-property'; readonly name: string }
  | {
      readonly kind: 'conditional';
      readonly condition: string;
      readonly whenTrue: readonly ParsedTemplateNode[];
      readonly whenFalse: readonly ParsedTemplateNode[];
    }
  | { readonly kind: 'other' };

export interface TemplateExpression {
  readonly range: TextRange;
  readonly source: string;
  readonly shape: ExpressionShape;
}

export interface ParsedElement {
  readonly kind: 'element';
  readonly tag: string;
  readonly range: TextRange;
  readonly attributes: readonly TemplateAttribute[];
  readonly classes: readonly string[];
  readonly children: readonly ParsedTemplateNode[];
}

export interface ParsedText {
  readonly kind: 'text';
  readonly range: TextRange;
  readonly text: string;
}

export interface ParsedExpression {
  readonly kind: 'expression';
  readonly expression: TemplateExpression;
}

export type ParsedTemplateNode = ParsedElement | ParsedText | ParsedExpression;
