import { createHash } from 'node:crypto';
import ts from 'typescript';
import type { ComponentParser, ParsedComponent, ParsedDeclaration } from '../../ports/component-parser.js';
import { installFrenchDiagnosticMessages, readSyntaxDiagnostics } from './compiler-internals.js';
import { findComponentClasses } from './component-declarations.js';
import { readDeclaredProperties } from './declared-properties.js';
import { readRenderTemplate } from './render-template.js';
import { readStaticStyles } from './styles/static-styles.js';

const SOURCE_FILE_NAME = 'component.ts';

function sha256Hex(text: string): string {
  return createHash('sha256').update(text, 'utf8').digest('hex');
}

function readDeclaration(sourceFile: ts.SourceFile): ParsedDeclaration {
  const [component, ...others] = findComponentClasses(sourceFile);
  if (component === undefined) {
    return { kind: 'missing' };
  }
  if (others.length > 0) {
    return { kind: 'ambiguous' };
  }
  return {
    kind: 'single',
    tag: component.tag,
    className: component.className,
    properties: readDeclaredProperties(component.node, sourceFile),
    template: readRenderTemplate(component.node, sourceFile),
    styles: readStaticStyles(component.node, sourceFile),
  };
}

function parse(text: string): ParsedComponent {
  installFrenchDiagnosticMessages();
  const sourceFile = ts.createSourceFile(SOURCE_FILE_NAME, text, ts.ScriptTarget.Latest, false, ts.ScriptKind.TS);
  return {
    version: sha256Hex(text),
    syntaxIssues: readSyntaxDiagnostics(sourceFile),
    declaration: readDeclaration(sourceFile),
  };
}

export function createTypeScriptComponentParser(): ComponentParser {
  return { parse };
}
