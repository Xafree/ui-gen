import { after, afterEach, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, mkdtemp, rm, symlink, writeFile } from 'node:fs/promises';
import { readFileSync, realpathSync, statSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { chromium } from 'playwright';
import { readInstalledPackage } from '../installed-package.mjs';
import { confinedReader, readProps, startMeter } from '../props.mjs';
import { captureSnapshot } from '../snapshot.mjs';
import { makeProject } from './helpers/service-client.mjs';

const MEBIBYTE = 1024 * 1024;
const LIMITS = { budgetMs: 20_000, maxFiles: 4_000, maxBytes: 64 * MEBIBYTE };
const LOOP_WORKER = new URL('./helpers/loop-worker.mjs', import.meta.url);
const MEMORY_WORKER = new URL('./helpers/memory-worker.mjs', import.meta.url);

const posix = (file) => file.split(path.sep).join('/');
const real = { realpath: realpathSync.native, stat: statSync, readFile: readFileSync };

describe('the readers of inspect', () => {
  let scratch;

  before(async () => {
    scratch = await mkdtemp(path.join(os.tmpdir(), 'uigen-review-'));
  });

  after(async () => {
    await rm(scratch, { recursive: true, force: true });
  });

  it('refuses a declaration file over the byte bound without reading it', async () => {
    const modules = path.join(scratch, 'sized');
    await mkdir(modules, { recursive: true });
    const file = posix(path.join(modules, 'big.d.ts'));
    await writeFile(file, `export interface Big { a: string }\n/*${'x'.repeat(3 * MEBIBYTE)}*/\n`);
    const reads = [];
    const spy = {
      ...real,
      readFile: (...args) => {
        reads.push(args[0]);
        return real.readFile(...args);
      },
    };
    const reader = confinedReader([modules], startMeter({ ...LIMITS, maxBytes: MEBIBYTE }), spy);

    assert.throws(() => reader.read(file), /MiB|bytes/);
    assert.deepEqual(reads, [], 'the file was read before its size was checked');
  });

  it('treats a link that leaves the package as a file or directory that does not exist', async () => {
    const modules = path.join(scratch, 'modules');
    const outside = path.join(scratch, 'outside');
    await mkdir(path.join(modules, 'pkg'), { recursive: true });
    await mkdir(outside, { recursive: true });
    await writeFile(path.join(outside, 'secret.d.ts'), 'export interface Secret { a: string }\n');
    await writeFile(path.join(modules, 'pkg', 'own.d.ts'), 'export interface Own { a: string }\n');
    await symlink(outside, path.join(modules, 'pkg', 'gen'), 'junction');
    const reader = confinedReader([modules], startMeter(LIMITS));
    const at = (...parts) => posix(path.join(modules, 'pkg', ...parts));

    assert.equal(reader.fileExists(at('own.d.ts')), true);
    assert.equal(reader.directoryExists(at()), true);
    assert.equal(reader.fileExists(at('gen', 'secret.d.ts')), false);
    assert.equal(reader.directoryExists(at('gen')), false);
    assert.equal(reader.read(at('gen', 'secret.d.ts')), undefined);
  });
});

describe('readProps in a worker', () => {
  const projects = [];

  afterEach(async () => {
    await Promise.all(projects.splice(0).map((project) => project.remove()));
  });

  const syntheticUi = async () => {
    const project = await makeProject(['synthetic-ui']);
    projects.push(project);
    return readInstalledPackage(project.dir, 'synthetic-ui');
  };

  it('interrupts a worker that never yields at the deadline and says so', async () => {
    const installed = await syntheticUi();
    const started = Date.now();

    const { components, notes } = await readProps(installed, { ...LIMITS, budgetMs: 300 }, { workerUrl: LOOP_WORKER });

    assert.ok(Date.now() - started < 15_000, 'the loop was not interrupted');
    assert.equal(components.size, 0);
    assert.match(notes.join('\n'), /took more than 0\.3 s/);
    assert.match(notes.join('\n'), /props not read/);
  });

  it('answers a note, not an error, when the worker runs out of memory', async () => {
    const installed = await syntheticUi();

    const { components, notes } = await readProps(installed, LIMITS, { workerUrl: MEMORY_WORKER, memoryMb: 16 });

    assert.equal(components.size, 0);
    assert.match(notes.join('\n'), /memory/);
    assert.match(notes.join('\n'), /props not read/);
  });

  it('still reads the props of a package in its default worker', async () => {
    const installed = await syntheticUi();

    const { components, notes } = await readProps(installed, LIMITS);

    assert.equal(components.get('Button')?.length, 3);
    assert.deepEqual(notes, []);
  });

  it('does not read a custom elements manifest that exceeds the byte bound', async () => {
    const installed = await syntheticUi();

    const { notes } = await readProps(installed, { ...LIMITS, maxBytes: 16 });

    assert.match(notes.join('\n'), /bytes|MiB/);
  });
});

describe('captureSnapshot animations of svg nodes', () => {
  let browser;
  let page;

  before(async () => {
    browser = await chromium.launch();
    page = await browser.newPage();
  });

  after(async () => {
    await page?.close();
    await browser?.close();
  });

  const svgOf = (node) => node.children.filter((child) => child.type === 'svg');

  it('records an infinite rotation on an svg, by transform or by the rotate property', async () => {
    await page.setContent(`<style>
      #root { width: 120px; height: 40px; background-color: white; }
      .spin { animation: spin 1s linear infinite; }
      .turn { animation: turn 1s linear infinite; }
      @keyframes spin { to { transform: rotate(360deg); } }
      @keyframes turn { to { rotate: 360deg; } }
    </style><div id="root">
      <svg class="spin" width="20" height="20"><circle cx="10" cy="10" r="8" fill="red"/></svg>
      <svg class="turn" width="20" height="20"><circle cx="10" cy="10" r="8" fill="blue"/></svg>
      <svg width="20" height="20"><circle cx="10" cy="10" r="8" fill="green"/></svg>
    </div>`);
    const { root } = await captureSnapshot(page, '#root', undefined);
    const [spin, turn, still] = svgOf(root);

    assert.equal(spin.animations.length, 1);
    assert.equal(spin.animations[0].name, 'spin');
    assert.equal(spin.animations[0].iterations, 'infinite');
    assert.deepEqual(spin.animations[0].properties, ['transform']);
    assert.deepEqual(turn.animations[0].properties, ['rotate']);
    assert.equal(still.animations, undefined);
  });

  it('keeps only the first 16 animations of an svg', async () => {
    const names = Array.from({ length: 20 }, (_, index) => `k${index}`);
    const keyframes = names.map((name, index) => `@keyframes ${name} { to { transform: rotate(${index + 1}deg); } }`).join('\n');
    await page.setContent(`<style>${keyframes}
      svg { animation-name: ${names.join(', ')}; animation-duration: 1s; animation-iteration-count: infinite; }
    </style><div id="root"><svg width="20" height="20"><circle cx="10" cy="10" r="8"/></svg></div>`);
    const { root } = await captureSnapshot(page, '#root', undefined);

    const [svg] = svgOf(root);
    assert.deepEqual(svg.animations.map((animation) => animation.name), names.slice(0, 16));
  });

  it('cuts the text fields of an animation to the bounds the Rust parser expects', async () => {
    const name = `n${'a'.repeat(299)}`;
    await page.setContent(`<style>
      @keyframes ${name} { to { transform: rotate(1deg); } }
      #root { animation: ${name} 1s linear infinite; width: 10px; height: 10px; background-color: red; }
    </style><div id="root"></div>`);
    const { root } = await captureSnapshot(page, '#root', undefined);

    assert.equal(root.animations[0].name.length, 256);
  });
});
