import { describe, expect, it } from 'vitest';
import {
  MAX_COMPUTE_TEXT_LENGTH,
  readComputeMessage,
  readComputeRequest,
} from '../../../../../src/modules/sync/adapters/utility-process/compute-protocol';
import { modelOf } from '../../application/components/component-doubles';

describe('compute requests', () => {
  it('reads an analysis request', () => {
    expect(readComputeRequest({ id: 3, operation: 'analyze', text: 'a' })).toEqual({
      id: 3,
      operation: 'analyze',
      text: 'a',
    });
    expect(readComputeRequest({ id: 0, operation: 'read-model', text: '' })).toEqual({
      id: 0,
      operation: 'read-model',
      text: '',
    });
  });

  it.each([
    null,
    'analyze',
    [],
    { id: -1, operation: 'analyze', text: 'a' },
    { id: 1.5, operation: 'analyze', text: 'a' },
    { id: '1', operation: 'analyze', text: 'a' },
    { id: 1, operation: 'format', text: 'a' },
    { id: 1, operation: 'analyze', text: 4 },
    { id: 1, operation: 'analyze', text: 'x'.repeat(MAX_COMPUTE_TEXT_LENGTH + 1) },
  ])('refuses %j', (value) => {
    expect(readComputeRequest(value)).toBeNull();
  });

  it('accepts a text of exactly the maximum length', () => {
    const text = 'x'.repeat(1_000_000);

    expect(readComputeRequest({ id: 1, operation: 'analyze', text })?.text).toBe(text);
  });
});

describe('compute messages', () => {
  const model = modelOf('tag=hz-a');

  it('reads the ready notice and the three kinds of reply', () => {
    expect(readComputeMessage({ outcome: 'ready' })).toEqual({ outcome: 'ready' });
    expect(readComputeMessage({ outcome: 'failed', id: 2 })).toEqual({ outcome: 'failed', id: 2 });
    expect(readComputeMessage({ outcome: 'model', id: 2, model })).toEqual({ outcome: 'model', id: 2, model });
    expect(readComputeMessage({ outcome: 'analysis', id: 2, model, highlight: [] })).toEqual({
      outcome: 'analysis',
      id: 2,
      model,
      highlight: [],
    });
  });

  it.each([
    null,
    [],
    {},
    { outcome: 'unknown', id: 1 },
    { outcome: 'failed' },
    { outcome: 'model', id: 1 },
    { outcome: 'model', id: 1, model: { tag: 'a' } },
    { outcome: 'model', id: 1, model: { ...modelOf('x'), variantProperty: 3 } },
    { outcome: 'analysis', id: 1, model: modelOf('x') },
    { outcome: 'analysis', id: 1, model: modelOf('x'), highlight: 'none' },
  ])('refuses %j', (value) => {
    expect(readComputeMessage(value)).toBeNull();
  });

  it('accepts a variant property name', () => {
    expect(readComputeMessage({ outcome: 'model', id: 1, model: { ...model, variantProperty: 'size' } })).not.toBeNull();
  });
});
