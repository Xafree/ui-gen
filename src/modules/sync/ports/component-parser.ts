import type { DeclaredProperty } from '../domain/model/component-model.js';
import type { DiagnosticDraft } from '../domain/model/diagnostic.js';
import type { ParsedTemplateNode, TemplateExpression } from '../domain/model/template-syntax.js';

export interface SingleDeclaration {
  readonly kind: 'single';
  readonly tag: string;
  readonly className: string;
  readonly properties: readonly DeclaredProperty[];
  readonly template: readonly ParsedTemplateNode[];
  readonly looseExpressions: readonly TemplateExpression[];
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
