import { describe, expect, it } from 'vitest';
import {
  findLayer,
  flattenLayers,
  layerIds,
  nearestDrawnAncestor,
  neighboursOf,
} from '../../../../../src/modules/editor/domain/layers/layer-lookup';
import type { LayerNode } from '../../../../../src/modules/editor/domain/layers/layer-tree';

function layer(id: string, children: readonly LayerNode[] = []): LayerNode {
  return { id, nodeId: null, name: id, kind: 'container', meta: null, children };
}

const TREE: readonly LayerNode[] = [
  layer('root', [layer('a', [layer('a:0', [layer('a:0.0'), layer('a:0.1'), layer('a:0.2')])]), layer('b')]),
];

describe('layer lookup', () => {
  // covers: FCT-0036
  it('lists every layer depth first, and finds one by id', () => {
    expect(flattenLayers(TREE).map((node) => node.id)).toEqual(['root', 'a', 'a:0', 'a:0.0', 'a:0.1', 'a:0.2', 'b']);
    expect(findLayer(TREE, 'a:0.1')?.name).toBe('a:0.1');
    expect(findLayer(TREE, 'missing')).toBeUndefined();
    expect(layerIds(TREE)).toEqual(new Set(['root', 'a', 'a:0', 'a:0.0', 'a:0.1', 'a:0.2', 'b']));
  });

  // covers: FCT-0036
  it('gives the parent and the previous drawn sibling of a layer', () => {
    const drawn = (id: string): boolean => id !== 'a:0.1';

    expect(neighboursOf(TREE, 'a:0.2', drawn)).toEqual({ parentId: 'a:0', previousId: 'a:0.0' });
    expect(neighboursOf(TREE, 'a:0.0', drawn)).toEqual({ parentId: 'a:0', previousId: null });
    expect(neighboursOf(TREE, 'root', drawn)).toEqual({ parentId: null, previousId: null });
    expect(neighboursOf(TREE, 'missing', drawn)).toEqual({ parentId: null, previousId: null });
  });

  // covers: FCT-0036
  it('climbs to the closest ancestor that is drawn in the canvas', () => {
    expect(nearestDrawnAncestor(TREE, 'a:0.0', (id) => id === 'a:0' || id === 'a:0.0')).toBe('a:0');
    expect(nearestDrawnAncestor(TREE, 'a:0.0', (id) => id === 'root')).toBe('root');
    expect(nearestDrawnAncestor(TREE, 'a:0', () => false)).toBeNull();
  });
});
