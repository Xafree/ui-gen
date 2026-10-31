import { after, afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { closeBrowser } from '../browser.mjs';
import { render } from '../render.mjs';
import { makeSourcesProject } from './helpers/review-project.mjs';

const LIBRARY = `import { createElement } from 'react';
const Root = ({ children }) => createElement('div', { style: { width: '10px', height: '10px' } }, children);
const Thumb = () => createElement('span');
const Track = () => createElement('span');
export const Toggle = Object.assign(Root, { Root, Thumb, Track, displayName: 'Toggle', variants: {} });
export const Plain = () => createElement('div', null, 'plain');
`;

describe('a component made of parts', () => {
  const cleanups = [];
  afterEach(async () => {
    await Promise.all(cleanups.splice(0).map((cleanup) => cleanup()));
  });
  after(closeBrowser);

  const rendered = async (component) => {
    const { dir, remove } = await makeSourcesProject({ 'parts-lib': { 'index.js': LIBRARY } });
    cleanups.push(remove);
    return render({ dir, package: 'parts-lib', component, props: {}, state: 'default', theme: 'light' });
  };

  it('says which parts it was rendered without', async () => {
    const { note } = await rendered('Toggle');
    assert.match(note, /rendered without its parts: Thumb, Track/);
  });

  it('says nothing for a component without parts', async () => {
    const { note } = await rendered('Plain');
    assert.equal(note, undefined);
  });
});
