import type { TemplateExpression } from '../model/template-syntax.js';

export const NOT_REPRESENTABLE_CODE = 'ui-gen(visual-sync)';

export const NOT_REPRESENTABLE_MESSAGE =
  "Cette expression ne peut pas être affichée dans l'éditeur visuel. Elle est conservée dans le code.";

export function boundProperty(expression: TemplateExpression, declared: ReadonlySet<string>): string | null {
  const { shape } = expression;
  return shape.kind === 'this-property' && declared.has(shape.name) ? shape.name : null;
}
