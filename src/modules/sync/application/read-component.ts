import type { ComponentModel } from '../domain/model/component-model.js';
import type { Diagnostic } from '../domain/model/diagnostic.js';
import { declarationDiagnostics, syntaxDiagnostics, visualSyncDiagnostics } from '../domain/rules/diagnostics.js';
import { buildTemplateTree } from '../domain/rules/template-tree.js';
import { projectStyles } from '../domain/styles/style-rules.js';
import { deriveVariants } from '../domain/styles/variants.js';
import type { ComponentParser } from '../ports/component-parser.js';

function unreadableModel(version: string, diagnostics: readonly Diagnostic[]): ComponentModel {
  return {
    tag: '',
    className: '',
    version,
    template: [],
    properties: [],
    variantProperty: null,
    variants: [],
    styles: [],
    diagnostics,
  };
}

export function readComponent(text: string, parser: ComponentParser): ComponentModel {
  const { version, syntaxIssues, declaration } = parser.parse(text);
  const syntaxErrors = syntaxDiagnostics(text, syntaxIssues);
  if (declaration.kind !== 'single') {
    const errors = syntaxErrors.length > 0 ? syntaxErrors : declarationDiagnostics(text, declaration.kind);
    return unreadableModel(version, errors);
  }
  const declared = new Set(declaration.properties.map(({ name }) => name));
  const template = buildTemplateTree(declaration.template, declared);
  const styles = projectStyles(declaration.styles, template);
  return {
    tag: declaration.tag,
    className: declaration.className,
    version,
    template,
    properties: declaration.properties,
    ...deriveVariants(declaration.properties, styles.rules),
    styles: styles.rules,
    diagnostics: syntaxErrors.length > 0 ? syntaxErrors : visualSyncDiagnostics(text, template, styles.unprojectable),
  };
}
