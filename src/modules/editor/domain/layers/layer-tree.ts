import type { EditedComponent, ElementNode, TemplateNode } from '../model/edited-component.js';
import { instanceNodeId, renderedVariants } from './instance-id.js';

export type LayerKind = 'set' | 'variant' | 'container' | 'instance' | 'text' | 'binding' | 'conditional' | 'opaque';

export interface LayerNode {
  readonly id: string;
  readonly nodeId: string | null;
  readonly name: string;
  readonly kind: LayerKind;
  readonly meta: string | null;
  readonly children: readonly LayerNode[];
}

function elementName(element: ElementNode): string {
  const [firstClass] = element.classes;
  return firstClass === undefined ? element.tag : `${element.tag}.${firstClass}`;
}

function isStructural(node: TemplateNode): boolean {
  return node.kind === 'element' || node.kind === 'conditional' || node.kind === 'opaque';
}

function elementKind(element: ElementNode): LayerKind {
  if (element.tag.includes('-')) return 'instance';
  return element.children.some(isStructural) ? 'container' : 'text';
}

function layersOf(nodes: readonly TemplateNode[], variant: string | null): LayerNode[] {
  return nodes.flatMap((node): LayerNode[] => {
    const id = instanceNodeId(variant, node.id);
    switch (node.kind) {
      case 'element': {
        const children = layersOf(node.children, variant);
        return [{ id, nodeId: node.id, name: elementName(node), kind: elementKind(node), meta: null, children }];
      }
      case 'conditional':
        return [
          {
            id,
            nodeId: node.id,
            name: node.condition,
            kind: 'conditional',
            meta: null,
            children: layersOf([...node.whenTrue, ...node.whenFalse], variant),
          },
        ];
      case 'binding':
        return [{ id, nodeId: node.id, name: node.property, kind: 'binding', meta: null, children: [] }];
      case 'opaque':
        return [{ id, nodeId: node.id, name: 'expression', kind: 'opaque', meta: null, children: [] }];
      case 'text':
        return [];
    }
  });
}

export function buildLayerTree(model: EditedComponent): LayerNode[] {
  const branches = renderedVariants(model).map((variant): LayerNode[] => {
    const layers = layersOf(model.template, variant);
    return variant === null ? layers : [{ id: variant, nodeId: null, name: variant, kind: 'variant', meta: variant, children: layers }];
  });
  return [{ id: model.tag, nodeId: null, name: model.tag, kind: 'set', meta: null, children: branches.flat() }];
}

export function countLayers(layers: readonly LayerNode[]): number {
  return layers.reduce((total, layer) => total + 1 + countLayers(layer.children), 0);
}
