import type { EditedComponent } from '../model/edited-component.js';

export function instanceNodeId(variant: string | null, nodeId: string): string {
  return variant === null ? nodeId : `${variant}:${nodeId}`;
}

export function renderedVariants(model: EditedComponent): readonly (string | null)[] {
  return model.variantProperty === null || model.variants.length === 0 ? [null] : model.variants;
}
