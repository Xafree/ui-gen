import { describe, expect, it } from 'vitest';
import { inspectorValues } from '../../../../../src/modules/editor/domain/inspector/inspector-values.js';
import type { ComponentModel } from '../../../../../src/modules/sync/application/component-model.js';

type StyleRule = ComponentModel['styles'][number];
type VisualValue = StyleRule['values'][number];

const RANGE = { start: 0, end: 0 };

function value(key: VisualValue['key'], raw: string, token: string | null = null): VisualValue {
  return { key, raw, token, range: RANGE };
}

function rule(values: readonly VisualValue[], targets: readonly string[], variant: StyleRule['variant'] = null, state: StyleRule['state'] = null): StyleRule {
  return { selector: '.x', range: RANGE, values, targets, variant, state };
}

function modelWith(styles: readonly StyleRule[], variantProperty: string | null = 'variant'): ComponentModel {
  return {
    tag: 'hz-x',
    className: 'HzX',
    version: 'v1',
    template: [],
    properties: [],
    variantProperty,
    variants: ['neutral', 'accent'],
    styles,
    diagnostics: [],
  };
}

describe('inspectorValues', () => {
  it('shows the token name when the value is a token and the raw value otherwise', () => {
    const model = modelWith([rule([value('gap', 'var(--hz-space-2)', '--hz-space-2'), value('fontSize', '13px')], ['0'])]);

    expect(inspectorValues(model, '0', null)).toEqual({ gap: '--hz-space-2', fontSize: '13px' });
  });

  it('keeps the last value written for a key', () => {
    const model = modelWith([rule([value('fill', 'red')], ['0']), rule([value('fill', 'blue')], ['0'])]);

    expect(inspectorValues(model, '0', null)).toEqual({ fill: 'blue' });
  });

  it('ignores the rules of other nodes', () => {
    const model = modelWith([rule([value('fill', 'red')], ['1']), rule([value('gap', '4px')], ['0'])]);

    expect(inspectorValues(model, '0', null)).toEqual({ gap: '4px' });
  });

  it('lets the rule of the selected variant override the common rule, even when written before it', () => {
    const model = modelWith([
      rule([value('fill', 'green')], ['0'], { property: 'variant', value: 'accent' }),
      rule([value('fill', 'grey'), value('gap', '4px')], ['0']),
    ]);

    expect(inspectorValues(model, '0', 'accent')).toEqual({ fill: 'green', gap: '4px' });
    expect(inspectorValues(model, '0', 'neutral')).toEqual({ fill: 'grey', gap: '4px' });
    expect(inspectorValues(model, '0', null)).toEqual({ fill: 'grey', gap: '4px' });
  });

  it('ignores a variant rule written for another property than the variant one', () => {
    const model = modelWith([rule([value('fill', 'green')], ['0'], { property: 'size', value: 'accent' })]);

    expect(inspectorValues(model, '0', 'accent')).toEqual({});
  });

  it('ignores the rules of a state', () => {
    const model = modelWith([rule([value('fill', 'grey')], ['0']), rule([value('fill', 'black')], ['0'], null, 'hover')]);

    expect(inspectorValues(model, '0', null)).toEqual({ fill: 'grey' });
  });

  it('returns nothing for a node no rule targets', () => {
    expect(inspectorValues(modelWith([]), '0', null)).toEqual({});
  });
});
