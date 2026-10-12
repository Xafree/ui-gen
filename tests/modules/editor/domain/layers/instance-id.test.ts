import { describe, expect, it } from 'vitest';
import { instanceNodeId as canvasInstanceNodeId, renderedVariants as canvasRenderedVariants } from '../../../../../src/modules/canvas/application/instance-node-id.js';
import { instanceNodeId, renderedVariants } from '../../../../../src/modules/editor/domain/layers/instance-id.js';
import type { ComponentModel } from '../../../../../src/modules/sync/application/component-model.js';
import type { EditedComponent } from '../../../../../src/modules/editor/domain/model/edited-component.js';

function modelWith(variantProperty: string | null, variants: readonly string[]): ComponentModel {
  return {
    tag: 'hz-button',
    className: 'HzButton',
    version: 'v1',
    template: [],
    properties: [],
    variantProperty,
    variants,
    styles: [],
    diagnostics: [],
  };
}

describe('instance ids shared with the canvas', () => {
  it.each([
    [null, 'n1'],
    ['primary', 'n1'],
  ])('builds the same id as the canvas for variant %s', (variant, nodeId) => {
    expect(instanceNodeId(variant, nodeId)).toBe(canvasInstanceNodeId(variant, nodeId));
  });

  it.each([
    [null, []],
    ['variant', []],
    [null, ['primary']],
    ['variant', ['primary', 'secondary']],
  ])('renders the same variants as the canvas for property %s and variants %j', (property, variants) => {
    const model = modelWith(property, variants);
    const edited: EditedComponent = model;
    expect(renderedVariants(edited)).toEqual(canvasRenderedVariants(model));
  });
});
