import { describe, expect, it } from 'vitest';
import { readHarnessMessage } from '../../../../src/modules/canvas/domain/harness-message';

const KNOWN_IDS: ReadonlySet<string> = new Set(['0', '0.1', '0.0.t.0', '2.1.t.0.0']);

const HEADER_RECT = { id: '0', x: 24, y: 16.5, width: 320, height: 88.25 };
const PLAN_RECT = { id: '0.1', x: -12, y: 0, width: 0, height: 22 };

function sparseRects(): unknown[] {
  const rects: unknown[] = [HEADER_RECT];
  rects.length = 2;
  return rects;
}

describe('readHarnessMessage', () => {
  describe('rects', () => {
    // covers: FCT-0032
    it('accepts the rectangles of known nodes with finite coordinates', () => {
      const message = { type: 'rects', rects: [HEADER_RECT, PLAN_RECT] };

      expect(readHarnessMessage(message, KNOWN_IDS)).toEqual({
        type: 'rects',
        rects: [
          { id: '0', x: 24, y: 16.5, width: 320, height: 88.25 },
          { id: '0.1', x: -12, y: 0, width: 0, height: 22 },
        ],
      });
    });

    // covers: FCT-0032
    it('accepts an empty list of rectangles', () => {
      expect(readHarnessMessage({ type: 'rects', rects: [] }, KNOWN_IDS)).toEqual({ type: 'rects', rects: [] });
    });

    // covers: FCT-0032
    it('refuses a rectangle whose node id is not in the current model', () => {
      const message = { type: 'rects', rects: [HEADER_RECT, { ...PLAN_RECT, id: '9.9' }] };

      expect(readHarnessMessage(message, KNOWN_IDS)).toBeNull();
    });

    // covers: FCT-0032
    it.each([
      ['NaN x', { ...HEADER_RECT, x: Number.NaN }],
      ['infinite y', { ...HEADER_RECT, y: Number.POSITIVE_INFINITY }],
      ['negative infinite width', { ...HEADER_RECT, width: Number.NEGATIVE_INFINITY }],
      ['string height', { ...HEADER_RECT, height: '88' }],
      ['missing width', { id: '0', x: 24, y: 16.5, height: 88.25 }],
      ['numeric id', { ...HEADER_RECT, id: 0 }],
      ['extra key', { ...HEADER_RECT, label: 'header' }],
      ['null rect', null],
      ['array rect', ['0', 24, 16.5, 320, 88.25]],
    ])('refuses a rectangle with a %s', (_label, rect) => {
      expect(readHarnessMessage({ type: 'rects', rects: [HEADER_RECT, rect] }, KNOWN_IDS)).toBeNull();
    });

    // covers: FCT-0032
    it('refuses rectangles that are not a dense array', () => {
      expect(readHarnessMessage({ type: 'rects', rects: sparseRects() }, KNOWN_IDS)).toBeNull();
      expect(readHarnessMessage({ type: 'rects', rects: { 0: HEADER_RECT, length: 1 } }, KNOWN_IDS)).toBeNull();
      expect(readHarnessMessage({ type: 'rects' }, KNOWN_IDS)).toBeNull();
    });

    // covers: FCT-0032
    it('refuses a rects message with an extra key', () => {
      expect(readHarnessMessage({ type: 'rects', rects: [HEADER_RECT], frame: 1 }, KNOWN_IDS)).toBeNull();
    });
  });

  describe('hit', () => {
    // covers: FCT-0032
    it('accepts a hit on a known node, including a conditional branch node', () => {
      expect(readHarnessMessage({ type: 'hit', id: '0.1' }, KNOWN_IDS)).toEqual({ type: 'hit', id: '0.1' });
      expect(readHarnessMessage({ type: 'hit', id: '2.1.t.0.0' }, KNOWN_IDS)).toEqual({
        type: 'hit',
        id: '2.1.t.0.0',
      });
    });

    // covers: FCT-0032
    it('accepts a hit on no node', () => {
      expect(readHarnessMessage({ type: 'hit', id: null }, KNOWN_IDS)).toEqual({ type: 'hit', id: null });
    });

    // covers: FCT-0032
    it.each([
      ['an unknown id', { type: 'hit', id: '7' }],
      ['an empty id', { type: 'hit', id: '' }],
      ['a missing id', { type: 'hit' }],
      ['an undefined id', { type: 'hit', id: undefined }],
      ['a numeric id', { type: 'hit', id: 1 }],
      ['an extra key', { type: 'hit', id: '0', x: 3 }],
    ])('refuses a hit with %s', (_label, message) => {
      expect(readHarnessMessage(message, KNOWN_IDS)).toBeNull();
    });
  });

  describe('ready', () => {
    // covers: FCT-0032
    it('accepts the ready message', () => {
      expect(readHarnessMessage({ type: 'ready' }, KNOWN_IDS)).toEqual({ type: 'ready' });
    });

    // covers: FCT-0032
    it('accepts the ready message even before any node is known', () => {
      expect(readHarnessMessage({ type: 'ready' }, new Set<string>())).toEqual({ type: 'ready' });
    });

    // covers: FCT-0032
    it('refuses a ready message with an extra key', () => {
      expect(readHarnessMessage({ type: 'ready', version: 'abc' }, KNOWN_IDS)).toBeNull();
    });
  });

  // covers: FCT-0032
  it.each([
    ['null', null],
    ['undefined', undefined],
    ['a string', 'ready'],
    ['a number', 42],
    ['an array', [{ type: 'ready' }]],
    ['an unknown type', { type: 'navigate', url: 'https://example.com' }],
    ['the hit-test request sent to the harness', { type: 'hit-test', x: 10, y: 20 }],
    ['a type in another case', { type: 'READY' }],
    ['a missing type', { id: '0' }],
    ['an inherited type', Object.create({ type: 'ready' }) as unknown],
  ])('refuses %s', (_label, data) => {
    expect(readHarnessMessage(data, KNOWN_IDS)).toBeNull();
  });
});
