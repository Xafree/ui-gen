import type { StyleRule } from '../model/component-model.js';
import type { ElementNode, TemplateNode } from '../model/template-node.js';
import type { TextRange } from '../model/text-range.js';
import { matchesSelectors, parseSelectors, stateOf, variantOf } from './selectors.js';
import type { ParsedStyleRule, ParsedStyleSheet } from './style-syntax.js';
import { projectDeclarations } from './visual-values.js';

export interface ProjectedStyles {
  readonly rules: readonly StyleRule[];
  readonly unprojectable: readonly TextRange[];
}

interface Placement {
  readonly element: ElementNode;
  readonly ancestors: readonly ElementNode[];
}

function placementsOf(nodes: readonly TemplateNode[], ancestors: readonly ElementNode[] = []): Placement[] {
  return nodes.flatMap((node): Placement[] => {
    switch (node.kind) {
      case 'element':
        return [{ element: node, ancestors }, ...placementsOf(node.children, [...ancestors, node])];
      case 'conditional':
        return placementsOf([...node.whenTrue, ...node.whenFalse], ancestors);
      case 'text':
      case 'binding':
      case 'opaque':
        return [];
    }
  });
}

function styleRule({ selector, range, declarations }: ParsedStyleRule, placements: readonly Placement[]): StyleRule {
  const selectors = parseSelectors(selector);
  return {
    selector,
    range,
    values: projectDeclarations(declarations),
    targets: placements
      .filter(({ element, ancestors }) => matchesSelectors(selectors, element, ancestors))
      .map(({ element }) => element.id),
    variant: variantOf(selector),
    state: stateOf(selector),
  };
}

export function projectStyles(sheet: ParsedStyleSheet, template: readonly TemplateNode[]): ProjectedStyles {
  const placements = placementsOf(template);
  return {
    rules: sheet.flatMap((item) => (item.kind === 'rule' ? [styleRule(item, placements)] : [])),
    unprojectable: sheet.flatMap((item) => (item.kind === 'unprojectable' ? [item.range] : [])),
  };
}
