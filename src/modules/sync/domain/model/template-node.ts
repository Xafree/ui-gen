import type { TextRange } from './text-range.js';


export interface TemplateAttribute {
  readonly name: string;
  readonly value: string | null;
  readonly range: TextRange;
}

export interface ElementNode {
  readonly kind: 'element';
  readonly id: string;
  readonly tag: string;
  readonly range: TextRange;
  readonly attributes: readonly TemplateAttribute[];
  readonly classes: readonly string[];
  readonly children: readonly TemplateNode[];
}

export interface TextNode {
  readonly kind: 'text';
  readonly id: string;
  readonly range: TextRange;
  readonly text: string;
}

export interface BindingNode {
  readonly kind: 'binding';
  readonly id: string;
  readonly range: TextRange;
  readonly property: string;
}

export interface ConditionalNode {
  readonly kind: 'conditional';
  readonly id: string;
  readonly range: TextRange;
  readonly condition: string;
  readonly whenTrue: readonly TemplateNode[];
  readonly whenFalse: readonly TemplateNode[];
}

export interface OpaqueNode {
  readonly kind: 'opaque';
  readonly id: string;
  readonly range: TextRange;
  readonly source: string;
}

export type TemplateNode = ElementNode | TextNode | BindingNode | ConditionalNode | OpaqueNode;
