import type { TextRange } from '../model/text-range.js';

export interface ParsedDeclaration {
  readonly property: string;
  readonly value: string;
  readonly valueRange: TextRange;
}

export interface ParsedStyleRule {
  readonly kind: 'rule';
  readonly selector: string;
  readonly range: TextRange;
  readonly declarations: readonly ParsedDeclaration[];
}

export interface UnprojectableStyle {
  readonly kind: 'unprojectable';
  readonly range: TextRange;
}

export type ParsedStyleItem = ParsedStyleRule | UnprojectableStyle;

export type ParsedStyleSheet = readonly ParsedStyleItem[];
