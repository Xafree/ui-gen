import { describe, expect, it } from 'vitest';
import { collectRenderedIds } from '../../../../src/modules/canvas/application/harness-url';
import { instanceNodeId, renderedVariants } from '../../../../src/modules/canvas/application/instance-node-id';
import { PRICING_CARD, readFixtureModel } from '../../sync/sync-fixtures';

const { model } = readFixtureModel(PRICING_CARD);
const withoutVariant = { ...model, variantProperty: null, variants: [] };

describe('instance node id', () => {
  // covers: FCT-0036
  it('prefixes the node id with the variant value, and leaves it alone without variant', () => {
    expect(instanceNodeId('pro', '0.1')).toBe('pro:0.1');
    expect(instanceNodeId(null, '0.1')).toBe('0.1');
  });

  // covers: FCT-0035
  it('renders one instance per variant, or a single plain instance without variant', () => {
    expect(renderedVariants(model)).toEqual(model.variants);
    expect(renderedVariants(withoutVariant)).toEqual([null]);
    expect(renderedVariants({ ...model, variants: [] })).toEqual([null]);
    expect(renderedVariants({ ...model, variantProperty: null })).toEqual([null]);
  });

  // covers: FCT-0036
  it('knows each node of each rendered instance by its prefixed id', () => {
    const ids = collectRenderedIds(model);
    const plain = collectRenderedIds(withoutVariant);

    expect(ids.size).toBe(plain.size * model.variants.length);
    for (const variant of model.variants) {
      for (const id of plain) expect(ids.has(`${variant}:${id}`)).toBe(true);
    }
    expect([...plain].every((id) => !id.includes(':'))).toBe(true);
  });
});
