import { after, afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { readFile, utimes, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { build } from 'esbuild';
import { closeBrowser } from '../browser.mjs';
import { bundleEntry } from '../bundle.mjs';
import { render } from '../render.mjs';
import { ServiceError } from '../protocol.mjs';
import { makeSourcesProject } from './helpers/review-project.mjs';

const MODULE_COUNT = 400;
const CONFINEMENT_COST_FACTOR = 5;
const COST_SLACK_MS = 500;

const collect = (node) => [node, ...(node.children ?? []).flatMap(collect)];
const textsOf = (snapshot) => collect(snapshot.root).filter((node) => node.type === 'text').map((node) => node.text);
const elementWithClass = (snapshot, name) =>
  collect(snapshot.root).find((node) => node.type === 'element' && (node.className ?? '').split(' ').includes(name));

const params = (dir, name, component, overrides = {}) => ({ dir, package: name, component, props: {}, state: 'default', theme: 'light', ...overrides });

const LABELLED = `import { createElement } from 'react';
export function Label(props) { return createElement('p', null, 'version-one:' + JSON.stringify(props)); }
export function Other() { return createElement('p', null, 'other-component'); }
`;

const STATEFUL = `import { createElement, useEffect, useState } from 'react';
let mounts = 0;
const STYLE = \`
.spy { background: #111111; color: #ffffff; width: 120px; height: 40px; }
.spy:hover { background: #222222; }
.spy:focus { background: #444444; }
.spy:active { background: #333333; }
.dark .spy { color: #aaaaaa; }
\`;
export function Spy({ tone }) {
  mounts += 1;
  window.__uigenSpyVisits = (window.__uigenSpyVisits ?? 0) + 1;
  const root = document.documentElement;
  const [held, setHeld] = useState(false);
  useEffect(() => {
    const press = () => setHeld(true);
    window.addEventListener('pointerdown', press);
    return () => window.removeEventListener('pointerdown', press);
  }, []);
  const line = ['mounts', mounts, 'visits', window.__uigenSpyVisits, 'html', root.className, 'tone', tone ?? 'none', 'held', held].join(':');
  return createElement('button', { className: 'spy' }, createElement('style', null, STYLE), line);
}
`;

const ELEMENT_SOURCE = `class XTag extends HTMLElement {
  connectedCallback() { const p = document.createElement('p'); p.textContent = 'tone:' + this.getAttribute('tone'); this.append(p); }
}
customElements.define('x-tag', XTag);
`;

const pngOf = (dir, result) => readFile(path.join(dir, ...result.png.split('/')));

describe('render speed', () => {
  const cleanups = [];

  afterEach(async () => {
    await Promise.all(cleanups.splice(0).map((cleanup) => cleanup()));
  });

  after(closeBrowser);

  const project = async (packages) => {
    const made = await makeSourcesProject(packages);
    cleanups.push(made.remove);
    return made;
  };

  describe('bundling cost of the confinement', () => {
    const manyModules = () => {
      const files = { 'index.js': `${Array.from({ length: MODULE_COUNT }, (_, index) => `export * from './m${index}.js';`).join('\n')}\n` };
      for (let index = 0; index < MODULE_COUNT; index += 1) {
        files[`m${index}.js`] = `export const value${index} = ${index};\n`;
      }
      return files;
    };

    it('keeps a confined bundle of 400 modules within a few times the cost of a plain one', async () => {
      const { sources } = await project({ 'many-modules': manyModules() });
      const contents = `import * as lib from 'many-modules';\nwindow.lib = lib;`;
      const plainStart = performance.now();
      await build({ stdin: { contents, resolveDir: sources, loader: 'js' }, bundle: true, write: false, format: 'iife', logLevel: 'silent' });
      const plainMs = performance.now() - plainStart;

      const confinedStart = performance.now();
      const code = await bundleEntry({ contents, resolveDir: sources });
      const confinedMs = performance.now() - confinedStart;

      assert.ok(code.length > 0);
      assert.ok(confinedMs < plainMs * CONFINEMENT_COST_FACTOR + COST_SLACK_MS, `plain ${Math.round(plainMs)} ms, confined ${Math.round(confinedMs)} ms`);
    });
  });

  describe('one bundle per package', () => {
    it('does not rebuild between renders of other props, states, themes, texts and components', async () => {
      const { dir, modules } = await project({ 'cached-ui': { 'index.js': LABELLED } });
      const first = await render(params(dir, 'cached-ui', 'Label'));
      assert.ok(textsOf(first.snapshot).includes('version-one:{}'), JSON.stringify(textsOf(first.snapshot)));

      await writeFile(path.join(modules, 'cached-ui', 'index.js'), LABELLED.replaceAll('version-one', 'version-two'));

      const other = await render(params(dir, 'cached-ui', 'Label', { props: { tone: 'a' }, state: 'hover', theme: 'dark', sampleText: 'Hi' }));
      assert.ok(textsOf(other.snapshot).some((text) => text.startsWith('version-one:')), JSON.stringify(textsOf(other.snapshot)));
      const sibling = await render(params(dir, 'cached-ui', 'Other'));
      assert.deepEqual(textsOf(sibling.snapshot), ['other-component']);
    });

    for (const [label, change] of [
      ['the manifest of the package', async (modules) => writeFile(path.join(modules, 'stale-ui', 'package.json'), `${JSON.stringify({ name: 'stale-ui', version: '2.0.0', type: 'module', main: 'index.js', peerDependencies: { react: '>=18' } })}\n`)],
      ['the hidden lockfile of the sources', async (modules) => writeFile(path.join(modules, '.package-lock.json'), '{"changed":true}\n')],
      ['the lockfile of the sources', async (modules) => writeFile(path.join(modules, '..', 'package-lock.json'), '{"changed":true}\n')],
    ]) {
      it(`rebuilds after a change of ${label}`, async () => {
        const { dir, modules } = await project({ 'stale-ui': { 'index.js': LABELLED } });
        await render(params(dir, 'stale-ui', 'Label'));
        await writeFile(path.join(modules, 'stale-ui', 'index.js'), LABELLED.replaceAll('version-one', 'version-two'));

        await change(modules);

        const after = await render(params(dir, 'stale-ui', 'Label'));
        assert.ok(textsOf(after.snapshot).some((text) => text.startsWith('version-two:')), JSON.stringify(textsOf(after.snapshot)));
      });
    }

    it('rebuilds after a manifest rewrite that keeps its size', async () => {
      const { dir, modules } = await project({ 'touched-ui': { 'index.js': LABELLED } });
      await render(params(dir, 'touched-ui', 'Label'));
      await writeFile(path.join(modules, 'touched-ui', 'index.js'), LABELLED.replaceAll('version-one', 'version-two'));

      const later = new Date(Date.now() + 60_000);
      await utimes(path.join(modules, 'touched-ui', 'package.json'), later, later);

      const after = await render(params(dir, 'touched-ui', 'Label'));
      assert.ok(textsOf(after.snapshot).some((text) => text.startsWith('version-two:')), JSON.stringify(textsOf(after.snapshot)));
    });

    it('keeps two projects apart even with the same package name and version', async () => {
      const one = await project({ 'twin-ui': { 'index.js': LABELLED } });
      const two = await project({ 'twin-ui': { 'index.js': LABELLED.replaceAll('version-one', 'version-two') } });

      const fromOne = await render(params(one.dir, 'twin-ui', 'Label'));
      const fromTwo = await render(params(two.dir, 'twin-ui', 'Label'));

      assert.ok(textsOf(fromOne.snapshot).some((text) => text.startsWith('version-one:')));
      assert.ok(textsOf(fromTwo.snapshot).some((text) => text.startsWith('version-two:')));
    });

    it('answers componentNotFound for a missing component of an already bundled package', async () => {
      const { dir } = await project({ 'known-ui': { 'index.js': LABELLED } });
      await render(params(dir, 'known-ui', 'Label'));

      await assert.rejects(
        render(params(dir, 'known-ui', 'Absent')),
        (error) => error instanceof ServiceError && error.kind === 'componentNotFound' && error.message.includes('Absent'),
      );
    });

    it('does not keep a failed bundle', async () => {
      const { dir, modules } = await project({ 'late-ui': { 'index.js': `export * from './absent.js';\n` } });
      await assert.rejects(render(params(dir, 'late-ui', 'Label')), (error) => error instanceof ServiceError && error.kind === 'render');

      await writeFile(path.join(modules, 'late-ui', 'absent.js'), LABELLED);

      const fixed = await render(params(dir, 'late-ui', 'Label'));
      assert.equal(fixed.snapshot.version, 1);
    });
  });

  describe('props and text passed at runtime', () => {
    it('hands over special characters in props and sample text exactly', async () => {
      const { dir } = await project({ 'echo-ui': { 'index.js': LABELLED.replace("'version-one:' + JSON.stringify(props)", "'echo:' + JSON.stringify(props)") } });
      const tricky = { quote: `"'\``, close: '</script>', line: 'a b', slash: '\\n' };

      const result = await render(params(dir, 'echo-ui', 'Label', { props: tricky, sampleText: '</script>"' }));

      assert.deepEqual(textsOf(result.snapshot), [`echo:${JSON.stringify({ ...tricky, children: '</script>"' })}`.replaceAll(/\s+/g, ' ')]);
    });

    it('sets the props of a custom element at runtime, per render', async () => {
      const { dir, modules } = await project({ 'tag-ui': { 'x-tag.js': ELEMENT_SOURCE, 'index.js': '' } });
      const manifest = { name: 'tag-ui', version: '1.0.0', type: 'module', main: 'index.js', customElements: 'custom-elements.json' };
      await writeFile(path.join(modules, 'tag-ui', 'package.json'), `${JSON.stringify(manifest)}\n`);
      await writeFile(
        path.join(modules, 'tag-ui', 'custom-elements.json'),
        `${JSON.stringify({ modules: [{ path: 'x-tag.js', declarations: [{ tagName: 'x-tag' }] }] })}\n`,
      );

      const one = await render(params(dir, 'tag-ui', 'x-tag', { props: { tone: 'one' } }));
      const two = await render(params(dir, 'tag-ui', 'x-tag', { props: { tone: 'two' }, sampleText: 'inner' }));

      assert.ok(textsOf(one.snapshot).includes('tone:one'), JSON.stringify(textsOf(one.snapshot)));
      assert.ok(textsOf(two.snapshot).includes('tone:two'), JSON.stringify(textsOf(two.snapshot)));
      assert.ok(textsOf(two.snapshot).includes('inner'), JSON.stringify(textsOf(two.snapshot)));
      assert.ok(!textsOf(one.snapshot).includes('inner'));
    });
  });

  describe('no state between renders', () => {
    const CLEAN_LINE = 'mounts:1:visits:1:html::tone:none:held:false';

    it('renders default in light byte for byte the same after pressed, focus and dark renders', async () => {
      const { dir } = await project({ 'spy-ui': { 'index.js': STATEFUL } });
      const clean = () => params(dir, 'spy-ui', 'Spy');

      const first = await render(clean());
      const firstPng = await pngOf(dir, first);
      assert.deepEqual(textsOf(first.snapshot), [CLEAN_LINE]);

      const pressed = await render({ ...clean(), state: 'pressed', theme: 'dark', props: { tone: 'x' } });
      assert.equal(elementWithClass(pressed.snapshot, 'spy').style.backgroundColor, 'rgb(51, 51, 51)');
      assert.deepEqual(textsOf(pressed.snapshot), ['mounts:2:visits:2:html:dark:tone:x:held:true']);
      const focused = await render({ ...clean(), state: 'focus', theme: 'dark' });
      assert.equal(elementWithClass(focused.snapshot, 'spy').style.backgroundColor, 'rgb(68, 68, 68)');
      const hovered = await render({ ...clean(), state: 'hover' });
      assert.equal(elementWithClass(hovered.snapshot, 'spy').style.backgroundColor, 'rgb(34, 34, 34)');

      const last = await render(clean());

      assert.deepEqual(last.snapshot, first.snapshot);
      assert.deepEqual(await pngOf(dir, last), firstPng);
      assert.equal(elementWithClass(last.snapshot, 'spy').style.backgroundColor, 'rgb(17, 17, 17)');
    });

    it('still renders after a page that fails', async () => {
      const broken = `export function Broken() { throw new Error('synthetic failure'); }\n`;
      const { dir } = await project({ 'mixed-ui': { 'index.js': `${LABELLED}${broken}` } });

      await assert.rejects(render(params(dir, 'mixed-ui', 'Broken')), (error) => error instanceof ServiceError && error.kind === 'render' && error.message.includes('synthetic failure'));

      const result = await render(params(dir, 'mixed-ui', 'Other'));
      assert.deepEqual(textsOf(result.snapshot), ['other-component']);
    });
  });
});
