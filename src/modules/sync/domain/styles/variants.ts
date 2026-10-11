import type { DeclaredProperty, StyleRule } from '../model/component-model.js';

export interface Variants {
  readonly variantProperty: string | null;
  readonly variants: readonly string[];
}

export function deriveVariants(properties: readonly DeclaredProperty[], rules: readonly StyleRule[]): Variants {
  const union = properties.find(({ decorator, literalValues }) => decorator === 'property' && literalValues.length > 0);
  if (union !== undefined) {
    return { variantProperty: union.name, variants: union.literalValues };
  }
  const targeted = rules.flatMap(({ variant }) => (variant === null ? [] : [variant]));
  const [first] = targeted;
  if (first === undefined) {
    return { variantProperty: null, variants: [] };
  }
  const values = targeted.filter(({ property }) => property === first.property).map(({ value }) => value);
  return { variantProperty: first.property, variants: [...new Set(values)] };
}
