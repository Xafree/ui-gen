import type { Diagnostic, DiagnosticDraft, DiagnosticSeverity } from '../model/diagnostic.js';
import type { TemplateNode } from '../model/template-node.js';
import { NOT_REPRESENTABLE_CODE, NOT_REPRESENTABLE_MESSAGE } from './representability.js';
import { opaqueRanges } from './template-tree.js';
import { createPositionLocator } from './text-position.js';

export type MissingDeclaration = 'missing' | 'ambiguous';

const DECLARATION_CODE = 'ui-gen(component)';

const DECLARATION_MESSAGES: Record<MissingDeclaration, string> = {
  missing: "Aucun élément personnalisé n'est déclaré dans ce fichier.",
  ambiguous: "Plusieurs éléments personnalisés sont déclarés dans ce fichier ; l'éditeur n'en lit qu'un par fichier.",
};

function diagnosticsAt(text: string, issues: readonly DiagnosticDraft[], severity: DiagnosticSeverity): Diagnostic[] {
  const locate = createPositionLocator(text);
  return issues
    .toSorted((first, second) => first.range.start - second.range.start)
    .map(({ range, code, message }) => {
      const { line, column } = locate(range.start);
      return { range, line, column, severity, code, message };
    });
}

export function syntaxDiagnostics(text: string, issues: readonly DiagnosticDraft[]): Diagnostic[] {
  return diagnosticsAt(text, issues, 'error');
}

export function declarationDiagnostics(text: string, problem: MissingDeclaration): Diagnostic[] {
  const issue = { range: { start: 0, end: 0 }, code: DECLARATION_CODE, message: DECLARATION_MESSAGES[problem] };
  return diagnosticsAt(text, [issue], 'error');
}

export function notRepresentableDiagnostics(text: string, template: readonly TemplateNode[]): Diagnostic[] {
  const issues = opaqueRanges(template).map((range) => ({
    range,
    code: NOT_REPRESENTABLE_CODE,
    message: NOT_REPRESENTABLE_MESSAGE,
  }));
  return diagnosticsAt(text, issues, 'warning');
}
