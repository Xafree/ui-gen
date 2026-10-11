import type { Diagnostic } from './code-diagnostics.js';

export type SyncTone = 'success' | 'danger' | 'accent';

export interface SyncState {
  readonly tone: SyncTone;
  readonly code: string;
  readonly status: string;
}

export interface ProblemCounter {
  readonly count: number;
  readonly hasError: boolean;
  readonly word: string;
  readonly name: string;
}

const SYNCED: SyncState = { tone: 'success', code: 'Synchronisé avec le canvas', status: 'Synchronisé' };
const RESTARTING: SyncState = { tone: 'accent', code: 'Analyse du code relancée…', status: "Relance de l'analyse…" };
const UNREADABLE: SyncState = {
  tone: 'danger',
  code: 'Code illisible : le canvas garde la dernière version lisible',
  status: 'Erreur de syntaxe',
};

const EXPRESSION_DELIMITERS = /^\$\{([\s\S]*)\}$/;

export function fileNameOf(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

export function hasSyntaxError(diagnostics: readonly Diagnostic[]): boolean {
  return diagnostics.some(({ severity }) => severity === 'error');
}

export function syncStateOf(diagnostics: readonly Diagnostic[], restarting: boolean): SyncState {
  if (restarting) return RESTARTING;
  return hasSyntaxError(diagnostics) ? UNREADABLE : SYNCED;
}

export function problemCounter(diagnostics: readonly Diagnostic[]): ProblemCounter | null {
  const count = diagnostics.length;
  if (count === 0) return null;
  const hasError = hasSyntaxError(diagnostics);
  const word = `${hasError ? 'erreur' : 'avertissement'}${count > 1 ? 's' : ''}`;
  return { count, hasError, word, name: `${String(count)} ${word}, afficher les problèmes` };
}

export function diagnosticTitle(diagnostic: Diagnostic): string {
  return diagnostic.severity === 'error' ? 'Erreur de syntaxe' : 'Expression non représentable';
}

export function diagnosticLocation(diagnostic: Diagnostic): string {
  return `ligne ${String(diagnostic.line)}, col. ${String(diagnostic.column)}`;
}

export function diagnosticMeta(fileName: string, diagnostic: Diagnostic): string {
  return `${fileName}, ${diagnosticLocation(diagnostic)} · ${diagnostic.code}`;
}

export function expressionSource(text: string, diagnostic: Diagnostic): string | null {
  if (diagnostic.severity === 'error') return null;
  const source = text.slice(diagnostic.range.start, diagnostic.range.end);
  return EXPRESSION_DELIMITERS.exec(source)?.[1] ?? source;
}
