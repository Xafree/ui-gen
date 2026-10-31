import { after, afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { closeBrowser } from '../browser.mjs';
import { checkComposition } from '../composition.mjs';
import { render } from '../render.mjs';
import { makeSourcesProject } from './helpers/review-project.mjs';

const LIBRARY = `import { createElement as h } from 'react';
const Root = ({ children, tone, disabled }) => h('section', { 'data-tone': tone, 'data-off': String(Boolean(disabled)), style: { width: '40px', height: '20px' } }, children);
const Trigger = ({ children }) => h('button', null, children);
export const Pick = Object.assign(Root, { Root, Trigger });
export const Note = ({ children }) => h('p', null, children);
export const notComponent = 3;
export const Version = '1.0';
`;

const PICK = { type: 'Pick', props: { tone: 'a' }, children: ['head', { type: 'Pick.Trigger', children: [{ type: 'Note', children: ['inner'] }] }] };

const nested = (levels) => (levels === 0 ? { type: 'Note' } : { type: 'Note', children: [nested(levels - 1)] });

describe('checkComposition', () => {
  const refused = (value, pattern, component = 'Note') => assert.throws(() => checkComposition(value, component), (error) => error.kind === 'invalidRequest' && pattern.test(error.message));

  it('accepts a tree of export paths and texts', () => {
    assert.equal(checkComposition(PICK, 'Pick'), PICK);
  });

  it('refuses what is not a node', () => {
    refused('text', /a text must be a child/);
    refused(null, /every node must be/);
    refused({ type: 'lower.case' }, /every node must be/);
    refused({ type: 'Note.__proto__' }, /every node must be/);
    refused({ type: 'Note', extra: 1 }, /unknown key "extra"/);
    refused({ type: 'Note', props: [] }, /props of "Note"/);
    refused({ type: 'Note', children: 'x' }, /children of "Note"/);
  });

  it('refuses a root other than the component', () => {
    refused({ type: 'Other' }, /its root must be "Note"/);
  });

  it('refuses injected markup', () => {
    refused({ type: 'Note', props: { dangerouslySetInnerHTML: { __html: 'x' } } }, /"dangerouslySetInnerHTML"/);
  });

  it('bounds depth, nodes and size', () => {
    refused(nested(12), /deeper than 12/);
    refused({ type: 'Note', children: Array.from({ length: 300 }, () => ({ type: 'Note' })) }, /more than 256 nodes/);
    refused({ type: 'Note', children: Array.from({ length: 100 }, () => 'x'.repeat(1000)) }, /larger than/);
  });
});

describe('render with a composition', () => {
  const cleanups = [];
  afterEach(async () => {
    await Promise.all(cleanups.splice(0).map((cleanup) => cleanup()));
  });
  after(closeBrowser);

  const project = async () => {
    const { dir, remove } = await makeSourcesProject({ 'pick-lib': { 'index.js': LIBRARY } });
    cleanups.push(remove);
    return dir;
  };
  const params = (dir, extra) => ({ dir, package: 'pick-lib', component: 'Pick', props: {}, state: 'default', theme: 'light', ...extra });

  it('mounts the parts, with neither the sample text nor the parts note', async () => {
    const { snapshot, note, png } = await render(params(await project(), { composition: PICK, sampleText: 'ignored' }));
    const text = JSON.stringify(snapshot);
    assert.match(text, /inner/);
    assert.match(text, /head/);
    assert.doesNotMatch(text, /ignored/);
    assert.equal(note, undefined);
    assert.match(png, /Pick-/);
  });

  it('gives the props of the request to the root, over those of the composition', async () => {
    const dir = await project();
    const { snapshot } = await render(params(dir, { composition: PICK, props: { tone: 'b' } }));
    assert.deepEqual(snapshot.source.props, { tone: 'b' });
  });

  it('names the capture after the composition', async () => {
    const dir = await project();
    const bare = await render(params(dir));
    const first = await render(params(dir, { composition: PICK }));
    const other = await render(params(dir, { composition: { ...PICK, children: ['other'] } }));
    assert.equal(new Set([bare.png, first.png, other.png]).size, 3);
  });

  it('names the capture after the sample text', async () => {
    const dir = await project();
    const bare = await render(params(dir));
    const texted = await render(params(dir, { sampleText: 'Pick' }));
    assert.notEqual(bare.png, texted.png);
  });

  for (const type of ['Missing', 'Pick.Missing', 'Version', 'Pick.Root.Trigger.Nope']) {
    it(`refuses the type ${type}, which is no exported component`, async () => {
      const composition = { type: 'Pick', children: [{ type }] };
      await assert.rejects(render(params(await project(), { composition })), (error) => error.kind === 'invalidRequest' && error.message.includes(`uses "${type}"`));
    });
  }

  it('refuses an invalid tree before opening a page', async () => {
    await assert.rejects(render(params(await project(), { composition: { type: 'Note' } })), /its root must be "Pick"/);
  });
});
