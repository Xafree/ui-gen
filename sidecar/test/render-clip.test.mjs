import { after, afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { closeBrowser } from '../browser.mjs';
import { render } from '../render.mjs';
import { makeSourcesProject } from './helpers/review-project.mjs';

const sized = (style) => `import { createElement } from 'react';
export function Box() { return createElement('div', { style: { ${style}, background: '#123456' } }); }
`;

const pngSize = async (file) => {
  const bytes = await readFile(file);
  return { width: bytes.readUInt32BE(16), height: bytes.readUInt32BE(20) };
};

describe('the original image', () => {
  const cleanups = [];
  afterEach(async () => {
    await Promise.all(cleanups.splice(0).map((cleanup) => cleanup()));
  });
  after(closeBrowser);

  const imageOf = async (style) => {
    const { dir, remove } = await makeSourcesProject({ 'clip-lib': { 'index.js': sized(style) } });
    cleanups.push(remove);
    const result = await render({ dir, package: 'clip-lib', component: 'Box', props: {}, state: 'default', theme: 'light' });
    return pngSize(path.join(dir, result.png));
  };

  it('holds the pixels the browser snapped a fractional box to, not the one its last fraction touches', async () => {
    assert.deepEqual(await imageOf(`width: '75.2px', height: '36.5px'`), { width: 75, height: 37 });
  });

  it('still holds a component larger than the viewport', async () => {
    assert.deepEqual(await imageOf(`width: '900px', height: '40px'`), { width: 900, height: 40 });
  });
});
