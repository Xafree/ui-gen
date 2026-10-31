import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import { PARTS_OF } from '../inspect.mjs';

const partsOf = (0, eval)(PARTS_OF);

describe('parts of compound exports', () => {
  it('finds statics that are exports named after the parent, the alias of its root included', () => {
    const root = () => null;
    const caption = () => null;
    const lone = () => null;
    const wider = () => null;
    Object.assign(root, { Root: root, Caption: caption, Lone: lone });
    Object.assign(wider, { Root: wider });
    const lib = { Panel: root, PanelRoot: root, PanelCaption: caption, Lone: lone, PanelWide: wider };
    const names = Object.keys(lib);
    assert.deepEqual(partsOf(lib, names), { PanelRoot: 'Panel', PanelCaption: 'Panel' });
  });

  it('survives statics it can not read', () => {
    const hostile = () => null;
    Object.defineProperty(hostile, 'Boom', {
      enumerable: true,
      get() {
        throw new Error('no');
      },
    });
    assert.deepEqual(partsOf({ Hostile: hostile, Nothing: null }, ['Hostile', 'Nothing']), {});
  });
});
