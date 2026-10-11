import type { DeclaredProperty } from '../domain/model/component-model.js';
import type { DiagnosticDraft } from '../domain/model/diagnostic.js';
import type { ParsedTemplateNode } from '../domain/model/template-syntax.js';
import type { ParsedStyleSheet } from '../domain/styles/style-syntax.js';

export interface SingleDeclaration {
  readonly kind: 'single';
  readonly tag: string;
  readonly className: string;
  readonly properties: readonly DeclaredProperty[];
  readonly template: readonly ParsedTemplateNode[];
  readonly styles: ParsedStyleSheet;
}

export type ParsedDeclaration = { readonly kind: 'missing' } | { readonly kind: 'ambiguous' } | SingleDeclaration;

export interface ParsedComponent {
  readonly version: string;
  readonly syntaxIssues: readonly DiagnosticDraft[];
  readonly declaration: ParsedDeclaration;
}

export interface ComponentParser {
  parse(text: string): ParsedComponent;
}
