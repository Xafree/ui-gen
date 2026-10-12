import { describe, expect, it } from 'vitest';
import { MAX_RECT_VALUE, readHarnessMessage } from '../../../../src/modules/canvas/domain/harness-message';

const KNOWN = new Set(['0']);

function rectsMessage(rect: Record<string, unknown>): unknown {
  return { type: 'rects', rects: [{ id: '0', x: 0, y: 0, width: 1, height: 1, ...rect }] };
}

describe('the bounds of the rectangles sent by the canvas', () => {
  it('keeps values inside the bounds untouched', () => {
    const message = rectsMessage({ x: -MAX_RECT_VALUE, y: MAX_RECT_VALUE, width: 12.5, height: 0 });

    expect(readHarnessMessage(message, KNOWN)).toEqual({
      type: 'rects',
      rects: [{ id: '0', x: -MAX_RECT_VALUE, y: MAX_RECT_VALUE, width: 12.5, height: 0 }],
    });
  });

  it.each(['x', 'y', 'width', 'height'])('clamps a huge %s', (key) => {
    const high = readHarnessMessage(rectsMessage({ [key]: 1e300 }), KNOWN);
    const low = readHarnessMessage(rectsMessage({ [key]: -1e300 }), KNOWN);

    expect(high).toMatchObject({ type: 'rects', rects: [{ [key]: MAX_RECT_VALUE }] });
    expect(low).toMatchObject({ type: 'rects', rects: [{ [key]: -MAX_RECT_VALUE }] });
  });

  it.each([Number.NaN, Number.POSITIVE_INFINITY])('still refuses %s', (value) => {
    expect(readHarnessMessage(rectsMessage({ height: value }), KNOWN)).toBeNull();
  });

  it('bounds the bottom edge a rectangle can ask for', () => {
    const message = readHarnessMessage(rectsMessage({ y: MAX_RECT_VALUE, height: 1e300 }), KNOWN);

    expect(message).toMatchObject({ rects: [{ y: MAX_RECT_VALUE, height: MAX_RECT_VALUE }] });
  });
});

describe('the render failure message of the canvas', () => {
  it('reads an error message with no content', () => {
    expect(readHarnessMessage({ type: 'error' }, KNOWN)).toEqual({ type: 'error' });
  });

  it('refuses an error message that carries content', () => {
    expect(readHarnessMessage({ type: 'error', message: 'Boom' }, KNOWN)).toBeNull();
  });
});
