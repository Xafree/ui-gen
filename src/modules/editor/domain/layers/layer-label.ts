import { variantOf } from '../code/code-targets.js';
import type { LayerNode } from './layer-tree.js';

const NO_BREAK_SPACE = String.fromCodePoint(0xa0);

function capitalized(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}

function tagOf(layer: LayerNode): string {
  return layer.name.split('.')[0] ?? layer.name;
}

function cardLabel(variant: string): string {
  return `carte ${capitalized(variant)}`;
}

function kindLabel(layer: LayerNode): string {
  switch (layer.kind) {
    case 'instance':
      return `instance de ${tagOf(layer)}.ts`;
    case 'container':
      return `conteneur ${tagOf(layer)}`;
    case 'text':
      return 'texte';
    default:
      return layer.name;
  }
}

export function selectionLabel(layer: LayerNode): string {
  const variant = variantOf(layer);
  if (layer.kind === 'variant') return cardLabel(layer.id);
  if (variant === null) return layer.name;
  return `${tagOf(layer)} · ${cardLabel(variant)}`;
}

export function layerSubtitle(layer: LayerNode, tag: string, variantCount: number): string {
  if (layer.kind === 'set') return `Composant, ${String(variantCount)}${NO_BREAK_SPACE}variantes · ${tag}.ts`;
  if (layer.kind === 'variant') return `Variante ${layer.id} de ${tag}`;
  const variant = variantOf(layer);
  const prefix = variant === null ? '' : `Carte ${capitalized(variant)} · `;
  return `${prefix}${kindLabel(layer)}`;
}
