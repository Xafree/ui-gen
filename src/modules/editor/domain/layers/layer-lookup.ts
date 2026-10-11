import type { LayerNode } from './layer-tree.js';

export interface LayerNeighbours {
  readonly parentId: string | null;
  readonly previousId: string | null;
}

export function flattenLayers(layers: readonly LayerNode[]): LayerNode[] {
  return layers.flatMap((layer) => [layer, ...flattenLayers(layer.children)]);
}

export function layerIds(layers: readonly LayerNode[]): Set<string> {
  return new Set(flattenLayers(layers).map((layer) => layer.id));
}

export function findLayer(layers: readonly LayerNode[], id: string): LayerNode | undefined {
  return flattenLayers(layers).find((layer) => layer.id === id);
}

function siblingsOf(layers: readonly LayerNode[], id: string, parent: LayerNode | null): { readonly parent: LayerNode | null; readonly siblings: readonly LayerNode[] } | null {
  if (layers.some((layer) => layer.id === id)) return { parent, siblings: layers };
  for (const layer of layers) {
    const found = siblingsOf(layer.children, id, layer);
    if (found !== null) return found;
  }
  return null;
}

export function neighboursOf(layers: readonly LayerNode[], id: string, isDrawn: (layerId: string) => boolean): LayerNeighbours {
  const found = siblingsOf(layers, id, null);
  if (found === null) return { parentId: null, previousId: null };
  const index = found.siblings.findIndex((layer) => layer.id === id);
  const previous = found.siblings.slice(0, index).findLast((layer) => isDrawn(layer.id));
  return { parentId: found.parent?.id ?? null, previousId: previous?.id ?? null };
}

export function nearestDrawnAncestor(layers: readonly LayerNode[], id: string, isDrawn: (layerId: string) => boolean): string | null {
  let current = neighboursOf(layers, id, isDrawn).parentId;
  while (current !== null && !isDrawn(current)) current = neighboursOf(layers, current, isDrawn).parentId;
  return current;
}
