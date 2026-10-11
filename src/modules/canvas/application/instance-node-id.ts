import type { ComponentModel, NodeId } from '../../sync/application/component-model.js';

export function instanceNodeId(variant: string | null, nodeId: NodeId): string {
  return variant === null ? nodeId : `${variant}:${nodeId}`;
}

export function renderedVariants(model: ComponentModel): readonly (string | null)[] {
  return model.variantProperty === null || model.variants.length === 0 ? [null] : model.variants;
}
