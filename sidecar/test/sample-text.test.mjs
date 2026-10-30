import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { makeProject, startService } from './helpers/service-client.mjs';

const NOTE = 'sampleText ignored: the component takes no content';
const HOSTILE = '"\'</script><img src=x onerror="window.__pwn=1">`${1+1}\\';

const collect = (node) => [node, ...(node.children ?? []).flatMap(collect)];
const texts = (snapshot) => collect(snapshot.root).filter((node) => node.type === 'text').map((node) => node.text);
const tags = (snapshot) => collect(snapshot.root).filter((node) => node.type === 'element').map((node) => node.tag);

describe('render sampleText', () => {
  let project;
  let service;

  before(async () => {
    project = await makeProject(['synthetic-sample']);
    service = await startService();
  });

  after(async () => {
    await service?.stop();
    await project?.remove();
  });

  const render = (component, overrides = {}) =>
    service.request('render', {
      dir: project.dir,
      package: 'synthetic-sample',
      component,
      props: {},
      state: 'default',
      theme: 'light',
      ...overrides,
    });

  const renderOk = async (component, overrides) => {
    const response = await render(component, overrides);
    assert.equal(response.ok, true, JSON.stringify(response));
    return response.result;
  };

  it('shows the sample text as the children of a React component', async () => {
    const result = await renderOk('Label', { sampleText: 'Button' });

    assert.deepEqual(texts(result.snapshot), ['Button']);
    assert.equal(result.note, undefined);
  });

  it('renders no text when no sample text is sent', async () => {
    const result = await renderOk('Label');

    assert.deepEqual(texts(result.snapshot), []);
    assert.equal(result.note, undefined);
  });

  it('renders a component that refuses children without the text and says so', async () => {
    const result = await renderOk('Field', { sampleText: 'Button' });

    assert.ok(tags(result.snapshot).includes('input'));
    assert.deepEqual(texts(result.snapshot), []);
    assert.equal(result.note, NOTE);
  });

  it('says nothing when a component simply does not display its children', async () => {
    const result = await renderOk('Silent', { sampleText: 'Button' });

    assert.deepEqual(texts(result.snapshot), ['fixed']);
    assert.equal(result.note, undefined);
  });

  it('puts the sample text in the light DOM of a web component, where its slot picks it up', async () => {
    const result = await renderOk('x-slotted', { sampleText: 'Button' });

    assert.deepEqual(texts(result.snapshot), ['Button']);
    assert.equal(result.note, undefined);
  });

  it('keeps a hostile sample text as plain text in React and in web components', async () => {
    for (const component of ['Label', 'x-slotted']) {
      const result = await renderOk(component, { sampleText: HOSTILE });

      assert.deepEqual(texts(result.snapshot), [HOSTILE], component);
      assert.ok(!tags(result.snapshot).includes('img'), component);
    }
  });

  it('refuses a sampleText that is not a string', async () => {
    const response = await render('Label', { sampleText: 42 });

    assert.equal(response.ok, false);
    assert.equal(response.error.kind, 'invalidRequest');
    assert.ok(response.error.message.includes('sampleText'), response.error.message);
  });
});
