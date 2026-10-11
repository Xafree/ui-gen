import { instanceNodeId, renderedVariants } from '../../../canvas/application/instance-node-id.js';
import type { ComponentModel, TemplateNode } from '../../../sync/application/component-model.js';
import type { LayerNode } from '../layers/layer-tree.js';

export interface TextRange {
  readonly start: number;
  readonly end: number;
}

interface PlacedNode {
  readonly node: Exclude<TemplateNode, { kind: 'text' }>;
  readonly depth: number;
}

function placed(nodes: readonly TemplateNode[], depth = 0): PlacedNode[] {
  return nodes.flatMap((node): PlacedNode[] => {
    switch (node.kind) {
      case 'text':
        return [];
      case 'element':
        return [{ node, depth }, ...placed(node.children, depth + 1)];
      case 'conditional':
        return [{ node, depth }, ...placed([...node.whenTrue, ...node.whenFalse], depth + 1)];
      default:
        return [{ node, depth }];
    }
  });
}

export function touchesLine(range: TextRange, line: TextRange): boolean {
  return range.start <= line.end && range.end > line.start;
}

export function nodeRange(template: readonly TemplateNode[], nodeId: string): TextRange | null {
  return placed(template).find(({ node }) => node.id === nodeId)?.node.range ?? null;
}

export function nodeIdAtLine(template: readonly TemplateNode[], line: TextRange): string | null {
  const nodes = placed(template);
  const opening = nodes.find(({ node }) => node.range.start >= line.start && node.range.start <= line.end);
  if (opening !== undefined) return opening.node.id;
  const containing = nodes.filter(({ node }) => touchesLine(node.range, line));
  return containing.reduce<PlacedNode | null>((deepest, candidate) => (deepest === null || candidate.depth >= deepest.depth ? candidate : deepest), null)?.node.id ?? null;
}

export function variantOf(layer: LayerNode): string | null {
  if (layer.kind === 'variant') return layer.id;
  if (layer.nodeId === null || layer.id === layer.nodeId) return null;
  return layer.id.slice(0, layer.id.length - layer.nodeId.length - 1);
}

export function layerIdForNode(model: ComponentModel, selected: LayerNode | undefined, nodeId: string): string {
  const variant = selected === undefined ? null : variantOf(selected);
  return instanceNodeId(variant ?? renderedVariants(model)[0] ?? null, nodeId);
}
