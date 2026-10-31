import { after, afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { closeBrowser } from '../browser.mjs';
import { render } from '../render.mjs';
import { makeSourcesProject } from './helpers/review-project.mjs';

const LIBRARY = `import { createElement } from 'react';
const Root = () => createElement('div', { hidden: true });
const Trigger = () => createElement('button');
export const Empty = () => null;
export const Compound = Object.assign(Root, { Root, Trigger, displayName: 'Compound' });
`;

describe('a component that renders nothing visible', () => {
  const cleanups = [];
  afterEach(async () => {
    await Promise.all(cleanups.splice(0).map((cleanup) => cleanup()));
  });
  after(closeBrowser);

  const failure = async (component) => {
    const { dir, remove } = await makeSourcesProject({ 'empty-lib': { 'index.js': LIBRARY } });
    cleanups.push(remove);
    const started = Date.now();
    const error = await render({ dir, package: 'empty-lib', component, props: {}, state: 'default', theme: 'light' }).then(
      () => assert.fail('the render must be refused'),
      (reason) => reason,
    );
    return { error, ms: Date.now() - started };
  };

  it('fails at once and says so instead of waiting for the whole budget', async () => {
    const { error, ms } = await failure('Empty');
    assert.equal(error.kind, 'render');
    assert.match(error.message, /component "Empty" rendered nothing visible$/);
    assert.ok(ms < 10_000, `took ${ms} ms`);
  });

  it('names the parts it was rendered without', async () => {
    const { error } = await failure('Compound');
    assert.match(error.message, /component "Compound" rendered nothing visible without its parts: Trigger/);
  });
});
