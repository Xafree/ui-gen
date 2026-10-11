import type { NodeId, TemplateNode } from '../model/template-node.js';
import type { ParsedTemplateNode } from '../model/template-syntax.js';
import type { TextRange } from '../model/text-range.js';
import { boundProperty } from './representability.js';

function childId(parentId: NodeId | null, index: number): NodeId {
  return parentId === null ? String(index) : `${parentId}.${String(index)}`;
}

export function buildTemplateTree(
  nodes: readonly ParsedTemplateNode[],
  declared: ReadonlySet<string>,
  parentId: NodeId | null = null,
): TemplateNode[] {
  return nodes.map((node, index): TemplateNode => {
    const id = childId(parentId, index);
    switch (node.kind) {
      case 'element':
        return {
          kind: 'element',
          id,
          tag: node.tag,
          range: node.range,
          attributes: node.attributes,
          classes: node.classes,
          children: buildTemplateTree(node.children, declared, id),
        };
      case 'text':
        return { kind: 'text', id, range: node.range, text: node.text };
      case 'expression': {
        const { range, source } = node.expression;
        const property = boundProperty(node.expression, declared);
        return property === null
          ? { kind: 'opaque', id, range, source }
          : { kind: 'binding', id, range, property };
      }
    }
  });
}

export function opaqueRanges(nodes: readonly TemplateNode[]): TextRange[] {
  return nodes.flatMap((node) => {
    switch (node.kind) {
      case 'opaque':
        return [node.range];
      case 'element':
        return opaqueRanges(node.children);
      case 'text':
      case 'binding':
        return [];
    }
  });
}
