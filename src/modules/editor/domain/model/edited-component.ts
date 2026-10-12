export interface TextRange {
  readonly start: number;
  readonly end: number;
}

export interface Diagnostic {
  readonly range: TextRange;
  readonly line: number;
  readonly column: number;
  readonly severity: 'error' | 'warning';
  readonly code: string;
  readonly message: string;
}

export interface HighlightToken {
  readonly start: number;
  readonly end: number;
  readonly className: string;
}

export type InspectorKey =
  | 'width'
  | 'height'
  | 'direction'
  | 'gap'
  | 'paddingBlock'
  | 'paddingInline'
  | 'align'
  | 'fill'
  | 'radius'
  | 'border'
  | 'shadow'
  | 'fontFamily'
  | 'fontSize'
  | 'fontWeight'
  | 'color';

export interface ElementNode {
  readonly kind: 'element';
  readonly id: string;
  readonly tag: string;
  readonly range: TextRange;
  readonly classes: readonly string[];
  readonly children: readonly TemplateNode[];
}

export interface TextNode {
  readonly kind: 'text';
  readonly id: string;
  readonly range: TextRange;
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
}

export type TemplateNode = ElementNode | TextNode | BindingNode | ConditionalNode | OpaqueNode;

export interface StyleRule {
  readonly state: string | null;
  readonly targets: readonly string[];
  readonly variant: { readonly property: string; readonly value: string } | null;
  readonly values: readonly { readonly key: InspectorKey; readonly raw: string; readonly token: string | null }[];
}

export interface EditedComponent {
  readonly tag: string;
  readonly template: readonly TemplateNode[];
  readonly variantProperty: string | null;
  readonly variants: readonly string[];
  readonly styles: readonly StyleRule[];
}

export interface DiagnosedComponent {
  readonly model: { readonly diagnostics: readonly Diagnostic[] };
}

export interface ChangedComponent extends DiagnosedComponent {
  readonly diagnostics: readonly Diagnostic[];
}
