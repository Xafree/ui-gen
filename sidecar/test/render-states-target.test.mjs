import { after, afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { closeBrowser } from '../browser.mjs';
import { render } from '../render.mjs';
import { makeSourcesProject } from './helpers/review-project.mjs';

const LIBRARY = `import { createElement as h, useState } from 'react';
const HIDDEN = { position: 'absolute', width: '1px', height: '1px', clip: 'rect(0 0 0 0)', overflow: 'hidden' };
export const Collection = ({ disabled }) => {
  const [shown, setShown] = useState('idle');
  return h('div', null,
    h('template', null),
    h('span', null, 'Label'),
    h('button', { disabled, onMouseEnter: () => setShown('hovered'), onMouseDown: () => setShown('down'), onFocus: () => setShown((now) => (now === 'down' ? now : 'focused')) }, disabled ? 'off' : shown));
};
export const Check = () => {
  const [shown, setShown] = useState('idle');
  return h('label', { onMouseEnter: () => setShown('hovered') },
    h('input', { type: 'checkbox', style: HIDDEN, onFocus: () => setShown('focused') }),
    h('span', null, shown));
};
`;

describe('the element a forced state reaches', () => {
  const cleanups = [];
  afterEach(async () => {
    await Promise.all(cleanups.splice(0).map((cleanup) => cleanup()));
  });
  after(closeBrowser);

  const shownIn = async (component, state, props = {}) => {
    const { dir, remove } = await makeSourcesProject({ 'states-lib': { 'index.js': LIBRARY } });
    cleanups.push(remove);
    const { snapshot } = await render({ dir, package: 'states-lib', component, props, state, theme: 'light' });
    return JSON.stringify(snapshot);
  };

  for (const [state, expected] of [['hover', 'hovered'], ['pressed', 'down'], ['focus', 'focused']]) {
    it(`${state} reaches the button of a composed root whose first child is a template`, async () => {
      assert.match(await shownIn('Collection', state), new RegExp(expected));
    });
  }

  it('disabled reaches the component through its props', async () => {
    assert.match(await shownIn('Collection', 'disabled'), /off/);
  });

  it('hover passes over a visually hidden input', async () => {
    assert.match(await shownIn('Check', 'hover'), /hovered/);
  });

  it('focus reaches a visually hidden input', async () => {
    assert.match(await shownIn('Check', 'focus'), /focused/);
  });
});
