import type { Diagnostic } from './diagnostic.js';
import type { TemplateNode } from './template-node.js';
import type { TextRange } from './text-range.js';

export interface DeclaredProperty {
  readonly name: string;
  readonly decorator: 'property' | 'state';
  readonly type: string | null;
  readonly literalValues: readonly string[];
  readonly defaultValue: string | null;
  readonly range: TextRange;
}

export type VisualKey =
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

export interface VisualValue {
  readonly key: VisualKey;
  readonly raw: string;
  readonly token: string | null;
  readonly range: TextRange;
}

export type StyleState = 'hover' | 'focus' | 'active' | 'disabled';

export interface StyleVariant {
  readonly property: string;
  readonly value: string;
}

export interface StyleRule {
  readonly selector: string;
  readonly range: TextRange;
  readonly values: readonly VisualValue[];
  readonly targets: readonly string[];
  readonly variant: StyleVariant | null;
  readonly state: StyleState | null;
}

export interface ComponentModel {
  readonly tag: string;
  readonly className: string;
  readonly version: string;
  readonly template: readonly TemplateNode[];
  readonly properties: readonly DeclaredProperty[];
  readonly variantProperty: string | null;
  readonly variants: readonly string[];
  readonly styles: readonly StyleRule[];
  readonly diagnostics: readonly Diagnostic[];
}
