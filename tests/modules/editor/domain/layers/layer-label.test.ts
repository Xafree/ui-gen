import { describe, expect, it } from 'vitest';
import { layerSubtitle, selectionLabel } from '../../../../../src/modules/editor/domain/layers/layer-label.js';
import type { LayerNode } from '../../../../../src/modules/editor/domain/layers/layer-tree.js';

function layer(partial: Partial<LayerNode> & Pick<LayerNode, 'id' | 'name' | 'kind'>): LayerNode {
  return { nodeId: null, meta: null, children: [], ...partial };
}

const CTA = layer({ id: 'pro:n3', nodeId: 'n3', name: 'hz-button.cta', kind: 'instance' });
const TITLE = layer({ id: 'starter:n1', nodeId: 'n1', name: 'h3.title', kind: 'text' });
const CARD = layer({ id: 'pro', name: 'pro', kind: 'variant' });
const SET = layer({ id: 'hz-pricing-card', name: 'hz-pricing-card', kind: 'set' });
const WRAPPER = layer({ id: 'n7', nodeId: 'n7', name: 'div.wrap', kind: 'container' });

describe('selectionLabel', () => {
  it('names an element of a variant after its tag and the card', () => {
    expect(selectionLabel(CTA)).toBe('hz-button · carte Pro');
    expect(selectionLabel(TITLE)).toBe('h3 · carte Starter');
  });

  it('names a variant after its card and the component after its tag', () => {
    expect(selectionLabel(CARD)).toBe('carte Pro');
    expect(selectionLabel(SET)).toBe('hz-pricing-card');
  });

  it('keeps the plain name outside any variant', () => {
    expect(selectionLabel(WRAPPER)).toBe('div.wrap');
  });
});

describe('layerSubtitle', () => {
  it('describes the kind of an element and its card', () => {
    expect(layerSubtitle(CTA, 'hz-pricing-card', 3)).toBe('Carte Pro · instance de hz-button.ts');
    expect(layerSubtitle(TITLE, 'hz-pricing-card', 3)).toBe('Carte Starter · texte');
    expect(layerSubtitle(WRAPPER, 'hz-pricing-card', 1)).toBe('conteneur div');
  });

  it('describes a variant and the component', () => {
    expect(layerSubtitle(CARD, 'hz-pricing-card', 3)).toBe('Variante pro de hz-pricing-card');
    expect(layerSubtitle(SET, 'hz-pricing-card', 3)).toBe('Composant, 3 variantes · hz-pricing-card.ts');
  });
});
