import type { TextRange } from './text-range.js';

export type DiagnosticSeverity = 'error' | 'warning';

export interface Diagnostic {
  readonly range: TextRange;
  readonly line: number;
  readonly column: number;
  readonly severity: DiagnosticSeverity;
  readonly code: string;
  readonly message: string;
}

export type DiagnosticDraft = Pick<Diagnostic, 'range' | 'code' | 'message'>;
