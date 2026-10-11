import { describe, expect, it } from 'vitest';
import { readHarnessCommand } from '../../../../src/modules/canvas/domain/harness-command';

describe('readHarnessCommand', () => {
  // covers: FCT-0036
  it('accepts a hit test at finite coordinates', () => {
    expect(readHarnessCommand({ type: 'hit-test', x: 12.5, y: -3 })).toEqual({ type: 'hit-test', x: 12.5, y: -3 });
  });

  // covers: FCT-0036
  it.each([
    ['NaN x', { type: 'hit-test', x: Number.NaN, y: 0 }],
    ['infinite y', { type: 'hit-test', x: 0, y: Number.POSITIVE_INFINITY }],
    ['string x', { type: 'hit-test', x: '1', y: 0 }],
    ['missing y', { type: 'hit-test', x: 1 }],
    ['extra key', { type: 'hit-test', x: 1, y: 2, z: 3 }],
  ])('refuses a hit test with a %s', (_label, command) => {
    expect(readHarnessCommand(command)).toBeNull();
  });

  // covers: FCT-0030
  it('accepts the dark preview switch only with a boolean', () => {
    expect(readHarnessCommand({ type: 'dark-preview', enabled: true })).toEqual({ type: 'dark-preview', enabled: true });
    expect(readHarnessCommand({ type: 'dark-preview', enabled: 1 })).toBeNull();
    expect(readHarnessCommand({ type: 'dark-preview' })).toBeNull();
  });

  // covers: FCT-0037
  it('accepts the hidden and locked layers as lists of strings', () => {
    expect(readHarnessCommand({ type: 'layer-state', hidden: ['pro:0.0'], locked: [] })).toEqual({
      type: 'layer-state',
      hidden: ['pro:0.0'],
      locked: [],
    });
  });

  // covers: FCT-0037
  it.each([
    ['a number among the hidden layers', { type: 'layer-state', hidden: ['a', 1], locked: [] }],
    ['locked layers that are not a list', { type: 'layer-state', hidden: [], locked: 'a' }],
    ['a missing list', { type: 'layer-state', hidden: [] }],
    ['an extra key', { type: 'layer-state', hidden: [], locked: [], more: [] }],
    ['an id that is too long', { type: 'layer-state', hidden: ['x'.repeat(513)], locked: [] }],
    ['too many ids', { type: 'layer-state', hidden: Array.from({ length: 10_001 }, () => 'a'), locked: [] }],
  ])('refuses a layer state with %s', (_label, command) => {
    expect(readHarnessCommand(command)).toBeNull();
  });

  // covers: FCT-0032
  it.each([null, 'hit-test', 3, [], { type: 'unknown' }, { x: 1, y: 2 }])('refuses %j', (data) => {
    expect(readHarnessCommand(data)).toBeNull();
  });
});
