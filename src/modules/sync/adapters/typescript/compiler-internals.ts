import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import ts from 'typescript';
import type { DiagnosticDraft } from '../../domain/model/diagnostic.js';

const FRENCH_CATALOGUE = 'typescript/lib/fr/diagnosticMessages.generated.json';

type MessageCatalogue = Record<string, string>;

interface LocalizableCompiler {
  setLocalizedDiagnosticMessages(messages: MessageCatalogue): void;
}

interface DiagnosedSourceFile extends ts.SourceFile {
  readonly parseDiagnostics: readonly ts.DiagnosticWithLocation[];
}

let frenchMessagesInstalled = false;

function isMessageCatalogue(value: unknown): value is MessageCatalogue {
  return (
    typeof value === 'object' && value !== null && Object.values(value).every((message) => typeof message === 'string')
  );
}

function isLocalizable(compiler: object): compiler is LocalizableCompiler {
  return 'setLocalizedDiagnosticMessages' in compiler && typeof compiler.setLocalizedDiagnosticMessages === 'function';
}

function hasParseDiagnostics(sourceFile: ts.SourceFile): sourceFile is DiagnosedSourceFile {
  return 'parseDiagnostics' in sourceFile && Array.isArray(sourceFile.parseDiagnostics);
}

export function installFrenchDiagnosticMessages(): void {
  if (frenchMessagesInstalled) {
    return;
  }
  const cataloguePath = createRequire(import.meta.url).resolve(FRENCH_CATALOGUE);
  const catalogue: unknown = JSON.parse(readFileSync(cataloguePath, 'utf8'));
  if (!isMessageCatalogue(catalogue) || !isLocalizable(ts)) {
    throw new Error('The TypeScript compiler cannot load its French diagnostic messages');
  }
  ts.setLocalizedDiagnosticMessages(catalogue);
  frenchMessagesInstalled = true;
}

export function readSyntaxDiagnostics(sourceFile: ts.SourceFile): DiagnosticDraft[] {
  if (!hasParseDiagnostics(sourceFile)) {
    throw new Error('The TypeScript compiler does not expose the syntax diagnostics of a source file');
  }
  return sourceFile.parseDiagnostics.map(({ start, length, code, messageText }) => ({
    range: { start, end: start + length },
    code: `ts(${String(code)})`,
    message: ts.flattenDiagnosticMessageText(messageText, '\n'),
  }));
}
