import { describe, expect, it } from 'vitest';
import type { Diagnostic } from '../../../../../src/modules/editor/domain/code/code-diagnostics.js';
import {
  diagnosticLocation,
  diagnosticMeta,
  diagnosticTitle,
  expressionSource,
  fileNameOf,
  problemCounter,
  syncStateOf,
} from '../../../../../src/modules/editor/domain/code/problem-summary.js';

const WARNING: Diagnostic = {
  range: { start: 4, end: 12 },
  line: 3,
  column: 9,
  severity: 'warning',
  code: 'ui-gen(visual-sync)',
  message: 'warning message',
};
const ERROR: Diagnostic = { ...WARNING, severity: 'error', code: 'ts(1005)', message: 'error message' };

describe('fileNameOf', () => {
  it('keeps the last segment of a Windows or POSIX path', () => {
    expect(fileNameOf('C:\\kit\\src\\hz-card.ts')).toBe('hz-card.ts');
    expect(fileNameOf('/kit/src/hz-card.ts')).toBe('hz-card.ts');
    expect(fileNameOf('hz-card.ts')).toBe('hz-card.ts');
  });
});

describe('syncStateOf', () => {
  it('is in sync when the code can be read, warnings included', () => {
    expect(syncStateOf([WARNING], false)).toEqual({ tone: 'success', code: 'Synchronisé avec le canvas', status: 'Synchronisé' });
  });

  it('is unreadable on a syntax error', () => {
    expect(syncStateOf([ERROR], false)).toEqual({
      tone: 'danger',
      code: 'Code illisible : le canvas garde la dernière version lisible',
      status: 'Erreur de syntaxe',
    });
  });

  it('says the analysis restarted first, whatever the code', () => {
    expect(syncStateOf([ERROR], true)).toEqual({ tone: 'accent', code: 'Analyse du code relancée…', status: "Relance de l'analyse…" });
  });
});

describe('problemCounter', () => {
  it('is absent without problems', () => {
    expect(problemCounter([])).toBeNull();
  });

  it('counts one warning in the singular', () => {
    expect(problemCounter([WARNING])).toEqual({ count: 1, hasError: false, word: 'avertissement', name: '1 avertissement, afficher les problèmes' });
  });

  it('counts several warnings in the plural', () => {
    expect(problemCounter([WARNING, WARNING])?.name).toBe('2 avertissements, afficher les problèmes');
  });

  it('counts an error as an error', () => {
    expect(problemCounter([ERROR])).toEqual({ count: 1, hasError: true, word: 'erreur', name: '1 erreur, afficher les problèmes' });
    expect(problemCounter([ERROR, ERROR])?.word).toBe('erreurs');
  });
});

describe('diagnostic texts', () => {
  it('titles a warning and an error', () => {
    expect(diagnosticTitle(WARNING)).toBe('Expression non représentable');
    expect(diagnosticTitle(ERROR)).toBe('Erreur de syntaxe');
  });

  it('locates a diagnostic by line and column', () => {
    expect(diagnosticLocation(WARNING)).toBe('ligne 3, col. 9');
    expect(diagnosticMeta('hz-card.ts', ERROR)).toBe('hz-card.ts, ligne 3, col. 9 · ts(1005)');
  });

  it('shows the source of an expression without its delimiters', () => {
    expect(expressionSource('abcd${this.x}ef', { ...WARNING, range: { start: 4, end: 13 } })).toBe('this.x');
  });

  it('shows the source as is when it is not wrapped in delimiters', () => {
    expect(expressionSource('abcdthis.xefgh', WARNING)).toBe('this.xef');
  });

  it('shows no source for an error', () => {
    expect(expressionSource('abcd${this.x}ef', ERROR)).toBeNull();
  });
});
