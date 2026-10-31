import { after, afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { closeBrowser } from '../browser.mjs';
import { startBudget } from '../deadline.mjs';
import { checkExportNames, inspect } from '../inspect.mjs';
import { render, retryBudgetMs } from '../render.mjs';
import { ServiceError } from '../protocol.mjs';
import { makeSourcesProject } from './helpers/review-project.mjs';

const renderParams = (dir, name, component) => ({ dir, package: name, component, props: {}, state: 'default', theme: 'light' });

const SLOW_ANIMATION = `import { createElement } from 'react';
const STYLE = '@keyframes fade { to { opacity: 0.5; } } .slow { animation: fade 3600s linear 1; }';
export function Slow() {
  return createElement('div', { className: 'slow' }, createElement('style', null, STYLE), 'slow');
}
`;

const BUSY_LOOP = `import { createElement } from 'react';
export function Busy() {
  for (;;) {
    // never returns
  }
  return createElement('div');
}
`;

const GOOD_BUTTON = `import { createElement } from 'react';
export function Fine() { return createElement('button', null, 'fine'); }
`;

describe('exports reported by the page', () => {
  after(closeBrowser);

  it('accepts a list of component names', () => {
    assert.deepEqual(checkExportNames(['Button', 'Card'], 'ui'), ['Button', 'Card']);
  });

  for (const [label, value] of [
    ['undefined', undefined],
    ['null', null],
    ['an object', { length: 1 }],
    ['a list with a number', ['Button', 3]],
    ['a list with an empty name', ['']],
    ['a name longer than 128 characters', [`A${'b'.repeat(128)}`]],
    ['more than 10000 names', Array.from({ length: 10_001 }, (_, index) => `N${index}`)],
  ]) {
    it(`refuses ${label} as a render error`, () => {
      assert.throws(
        () => checkExportNames(value, 'ui'),
        (error) => error instanceof ServiceError && error.kind === 'render' && error.message.includes('ui'),
      );
    });
  }

  it('answers a render error when a package exports a component with an oversized name', async () => {
    const longName = `A${'b'.repeat(200)}`;
    const project = await makeSourcesProject({ 'long-names': { 'index.js': `export function ${longName}() { return null; }\n` } });
    try {
      await assert.rejects(inspect({ dir: project.dir, package: 'long-names' }), (error) => error instanceof ServiceError && error.kind === 'render');
    } finally {
      await project.remove();
    }
  });
});

describe('render budget', () => {
  const cleanups = [];

  afterEach(async () => {
    await Promise.all(cleanups.splice(0).map((cleanup) => cleanup()));
  });

  after(closeBrowser);

  it('expires a pending promise with a render error after the budget', async () => {
    const budget = startBudget(50, 'out of time');

    await assert.rejects(
      budget.bound(new Promise(() => undefined)),
      (error) => error instanceof ServiceError && error.kind === 'render' && error.message === 'out of time',
    );
    assert.equal(budget.remaining(), 0);
  });

  it('lets a promise that settles in time through', async () => {
    const budget = startBudget(5_000, 'out of time');

    assert.equal(await budget.bound(Promise.resolve('done')), 'done');
    assert.ok(budget.remaining() > 0);
  });

  it('renders a component whose finite animation lasts an hour without waiting for it', async () => {
    const project = await makeSourcesProject({ 'slow-ui': { 'index.js': SLOW_ANIMATION } });
    cleanups.push(project.remove);
    const started = Date.now();

    const result = await render(renderParams(project.dir, 'slow-ui', 'Slow'));

    assert.equal(result.snapshot.version, 1);
    assert.ok(Date.now() - started < 20_000, `took ${Date.now() - started} ms`);
  });

  it('answers a render error before 30 s when the page never returns, then renders again', async () => {
    const project = await makeSourcesProject({ 'busy-ui': { 'index.js': BUSY_LOOP }, 'fine-ui': { 'index.js': GOOD_BUTTON } });
    cleanups.push(project.remove);
    const started = Date.now();

    await assert.rejects(
      render(renderParams(project.dir, 'busy-ui', 'Busy')),
      (error) => error instanceof ServiceError && error.kind === 'render' && error.message.includes('25 s'),
    );
    assert.ok(Date.now() - started < 30_000, `took ${Date.now() - started} ms`);

    const result = await render(renderParams(project.dir, 'fine-ui', 'Fine'));
    assert.equal(result.snapshot.version, 1);
  });
});

describe('the retry of a refused sample text', () => {
  const CALLER_TIMEOUT_MS = 30_000;

  it('shares one cap with the first attempt, under the timeout of the caller', () => {
    for (const spent of [0, 1_000, 10_000, 24_999]) {
      const budget = retryBudgetMs(spent);
      assert.ok(budget > 0, `no time left after ${spent}`);
      assert.ok(spent + budget < CALLER_TIMEOUT_MS, `${spent} + ${budget}`);
    }
  });
});
