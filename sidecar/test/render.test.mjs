import { after, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { readFile, rm } from 'node:fs/promises';
import { readFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { makeProject, startService } from './helpers/service-client.mjs';

const PNG_SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
const BUTTON_PROPS = { variant: 'solid', size: 'md' };

const collect = (node) => [node, ...(node.children ?? []).flatMap(collect)];

const renderParams = (dir, overrides) => ({
  dir,
  package: 'synthetic-ui',
  component: 'Button',
  props: BUTTON_PROPS,
  state: 'default',
  theme: 'light',
  ...overrides,
});

const assertValidSnapshot = (snapshot) => {
  assert.equal(snapshot.version, 1);
  assert.ok(snapshot.viewport.width > 0);
  assert.ok(snapshot.viewport.height > 0);
  assert.ok(snapshot.viewport.deviceScaleFactor > 0);
  assert.equal(snapshot.root.type, 'element');
  assert.equal(typeof snapshot.root.tag, 'string');
  assert.ok(Array.isArray(snapshot.root.children));
  for (const node of collect(snapshot.root)) {
    assert.ok(['element', 'text', 'svg'].includes(node.type), `node type ${node.type}`);
    for (const key of ['x', 'y', 'width', 'height']) {
      assert.ok(Number.isFinite(node.box[key]), `box.${key} is finite`);
    }
  }
};

const buttonOf = (snapshot) => collect(snapshot.root).find((node) => node.type === 'element' && node.tag === 'button');

const readLaunchLog = (file) =>
  readFileSync(file, 'utf8')
    .split('\n')
    .filter((line) => line !== '')
    .map((line) => JSON.parse(line));

describe('render', () => {
  let project;
  let service;
  let launchLog;

  before(async () => {
    project = await makeProject(['synthetic-ui', 'synthetic-broken', 'synthetic-vue-lib']);
    launchLog = path.join(os.tmpdir(), `uigen-launch-${path.basename(project.dir)}.jsonl`);
    service = await startService({ launchLog });
  });

  after(async () => {
    await service?.stop();
    await project?.remove();
    await rm(launchLog, { force: true });
  });

  const renderOk = async (state) => {
    const response = await service.request('render', renderParams(project.dir, { state }));
    assert.equal(response.ok, true, JSON.stringify(response));
    return response.result;
  };

  const assertPngWritten = async (png) => {
    assert.ok(png.startsWith('.uigen/captures/'), png);
    assert.ok(png.endsWith('.png'), png);
    assert.ok(!png.includes('\\') && !png.includes('..'), png);
    const bytes = await readFile(path.join(project.dir, ...png.split('/')));
    assert.ok(bytes.length > PNG_SIGNATURE.length);
    assert.deepEqual(bytes.subarray(0, PNG_SIGNATURE.length), PNG_SIGNATURE);
  };

  // covers: E2-3.2
  it('renders the Button in default then hover with a different hover style and a PNG on disk', async () => {
    const idle = await renderOk('default');
    const hovered = await renderOk('hover');

    for (const result of [idle, hovered]) {
      assertValidSnapshot(result.snapshot);
      await assertPngWritten(result.png);
    }
    const idleButton = buttonOf(idle.snapshot);
    const hoveredButton = buttonOf(hovered.snapshot);
    assert.ok(idleButton, 'button element in the default snapshot');
    assert.ok(hoveredButton, 'button element in the hover snapshot');
    assert.equal(idleButton.style.backgroundColor, 'rgb(25, 118, 210)');
    assert.equal(hoveredButton.style.backgroundColor, 'rgb(21, 101, 192)');
    assert.ok(collect(idle.snapshot.root).some((node) => node.type === 'text' && node.text === 'Button'));
  });

  // covers: E2-3.3
  it('answers componentNotFound and names the component absent from the exports', async () => {
    const response = await service.request('render', renderParams(project.dir, { component: 'Missing' }));

    assert.equal(response.ok, false);
    assert.equal(response.error.kind, 'componentNotFound');
    assert.ok(response.error.message.includes('Missing'), response.error.message);
  });

  // covers: E2-3.3
  it('answers notInstalled when the package is not installed', async () => {
    const response = await service.request('render', renderParams(project.dir, { package: 'not-installed-pkg' }));

    assert.equal(response.ok, false);
    assert.equal(response.error.kind, 'notInstalled');
    assert.ok(response.error.message.includes('not-installed-pkg'), response.error.message);
  });

  // covers: E2-3.3
  it('answers frameworkUnsupported for a vue package', async () => {
    const response = await service.request('render', renderParams(project.dir, { package: 'synthetic-vue-lib', component: 'Card', props: {} }));

    assert.equal(response.ok, false);
    assert.equal(response.error.kind, 'frameworkUnsupported');
    assert.ok(response.error.message.includes('vue'), response.error.message);
  });

  // covers: E2-3.3
  it('answers render with the message of the exception thrown by the page', async () => {
    const response = await service.request('render', renderParams(project.dir, { package: 'synthetic-broken', component: 'Broken', props: {} }));

    assert.equal(response.ok, false);
    assert.equal(response.error.kind, 'render');
    assert.ok(response.error.message.includes('synthetic render failure'), response.error.message);
  });

  // covers: E2-3.4
  it('launches Chromium without a remote debugging port and closes it on shutdown', async () => {
    const beforeShutdown = readLaunchLog(launchLog);
    const launches = beforeShutdown.filter((entry) => entry.event === 'launch');
    assert.ok(launches.length >= 1, 'Chromium was launched by the renders above');
    for (const { options } of launches) {
      const args = options?.args ?? [];
      assert.ok(!args.some((arg) => /^--remote-debugging-(port|address)/.test(arg)), JSON.stringify(args));
      assert.notEqual(options?.devtools, true);
    }
    assert.equal(beforeShutdown.filter((entry) => entry.event === 'disconnected').length, 0, 'Chromium is still open before shutdown');

    const response = await service.request('shutdown', {});
    assert.equal(response.ok, true);
    const { code } = await service.exited;

    assert.equal(code, 0);
    const afterShutdown = readLaunchLog(launchLog);
    assert.equal(afterShutdown.filter((entry) => entry.event === 'disconnected').length, launches.length);
  });
});
