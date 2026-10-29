import { after, afterEach, before, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, mkdtemp, rm, stat, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { readInstalledPackage } from '../installed-package.mjs';
import { readProps } from '../props.mjs';
import { makeProject, startService } from './helpers/service-client.mjs';

const LIMITS = { budgetMs: 20_000, maxFiles: 4_000, maxBytes: 64 * 1024 * 1024 };
const MEBIBYTE = 1024 * 1024;
const BOUND_TEST_TIMEOUT_MS = 120_000;
const RESPONSE_BUDGET_MS = 30_000;

const BUTTON_PROPS = [
  { name: 'variant', type: 'enum', values: ['solid', 'outline'] },
  { name: 'size', type: 'enum', values: ['sm', 'md'] },
  { name: 'disabled', type: 'boolean', values: null },
];

const byName = (props) => [...props].sort((a, b) => (a.name < b.name ? -1 : Number(a.name > b.name)));

const propsOf = (response, name) => {
  assert.equal(response.ok, true, JSON.stringify(response));
  const component = response.result.components.find((entry) => entry.name === name);
  assert.ok(component !== undefined, `component ${name} is missing from ${JSON.stringify(response.result.components)}`);
  return component.props;
};

const exists = async (file) => {
  try {
    await stat(file);
    return true;
  } catch {
    return false;
  }
};

const sourcesModules = (project) => path.join(project.dir, '.uigen', 'sources', 'node_modules');

async function writeGeneratedPackage(project, name, files) {
  const root = path.join(sourcesModules(project), name);
  await mkdir(path.join(root, 'gen'), { recursive: true });
  const manifest = {
    name,
    version: '1.0.0',
    type: 'module',
    main: 'index.js',
    types: 'index.d.ts',
    peerDependencies: { react: '>=18' },
  };
  await writeFile(path.join(root, 'package.json'), JSON.stringify(manifest));
  await writeFile(path.join(root, 'index.js'), 'export function Button() {\n  return null;\n}\n');
  const reExports = files.map(({ file }) => `export * from './gen/${file}';`);
  await writeFile(
    path.join(root, 'index.d.ts'),
    [
      "export interface ButtonProps { variant: 'solid' | 'outline'; }",
      'export declare function Button(props: ButtonProps): unknown;',
      ...reExports,
      '',
    ].join('\n'),
  );
  const batch = 200;
  for (let start = 0; start < files.length; start += batch) {
    await Promise.all(
      files.slice(start, start + batch).map(({ file, content }) => writeFile(path.join(root, 'gen', `${file}.d.ts`), content)),
    );
  }
}

describe('readProps', () => {
  const projects = [];

  const projectWith = async (packages) => {
    const project = await makeProject(packages);
    projects.push(project);
    return project;
  };

  afterEach(async () => {
    await Promise.all(projects.splice(0).map((project) => project.remove()));
  });

  // covers: E3-1.1
  it('returns the Button props of synthetic-ui with their kinds and enum values in declaration order', async () => {
    const project = await projectWith(['synthetic-ui']);
    const installed = await readInstalledPackage(project.dir, 'synthetic-ui');

    const { components, notes } = await readProps(installed, LIMITS);

    assert.ok(components instanceof Map);
    assert.deepEqual(byName(components.get('Button')), byName(BUTTON_PROPS));
    assert.deepEqual(notes, []);
  });

  // covers: E3-1.2
  it('keeps only the props declared by the package, for function and constructor signatures', async () => {
    const project = await projectWith(['synthetic-typed', 'synthetic-base-types']);
    const installed = await readInstalledPackage(project.dir, 'synthetic-typed');

    const { components } = await readProps(installed, LIMITS);

    assert.deepEqual(byName(components.get('Card')), [
      { name: 'count', type: 'number', values: null },
      { name: 'dense', type: 'boolean', values: null },
      { name: 'heading', type: 'string', values: null },
      { name: 'meta', type: 'other', values: null },
      { name: 'tone', type: 'enum', values: ['quiet', 'loud', 'neutral'] },
    ]);
    assert.deepEqual(byName(components.get('Panel')), [
      { name: 'open', type: 'boolean', values: null },
      { name: 'side', type: 'enum', values: ['left', 'right'] },
    ]);
  });

  // covers: E3-1.3
  it('answers no props and a note for a package without type declarations', async () => {
    const project = await projectWith(['synthetic-untyped']);
    const installed = await readInstalledPackage(project.dir, 'synthetic-untyped');

    const { components, notes } = await readProps(installed, LIMITS);

    for (const props of components.values()) {
      assert.deepEqual(props, []);
    }
    assert.ok(notes.length > 0);
    assert.match(notes.join('\n'), /types?|declarations?/i);
  });

  // covers: E3-1.3
  it('stops at the file bound it is given and says so', async () => {
    const project = await projectWith(['synthetic-ui']);
    const installed = await readInstalledPackage(project.dir, 'synthetic-ui');

    const { components, notes } = await readProps(installed, { ...LIMITS, maxFiles: 1 });

    assert.deepEqual(components.get('Button') ?? [], []);
    assert.match(notes.join('\n'), /files?/i);
  });

  // covers: E3-1.3
  it('stops at the byte bound it is given and says so', async () => {
    const project = await projectWith(['synthetic-ui']);
    const installed = await readInstalledPackage(project.dir, 'synthetic-ui');

    const { components, notes } = await readProps(installed, { ...LIMITS, maxBytes: 64 });

    assert.deepEqual(components.get('Button') ?? [], []);
    assert.match(notes.join('\n'), /bytes?|MiB|Mio|MB/i);
  });
});

describe('inspect props', () => {
  let project;
  let service;

  before(async () => {
    project = await makeProject(['synthetic-ui', 'synthetic-typed', 'synthetic-base-types', 'synthetic-untyped']);
    service = await startService();
  });

  after(async () => {
    await service?.stop();
    await project?.remove();
  });

  // covers: E3-1.1
  it('rends the typed props of the React Button and of the x-chip web component of synthetic-ui', async () => {
    const response = await service.request('inspect', { dir: project.dir, package: 'synthetic-ui' });

    assert.deepEqual(byName(propsOf(response, 'Button')), byName(BUTTON_PROPS));
    assert.deepEqual(propsOf(response, 'x-chip'), [{ name: 'label', type: 'string', values: null }]);
    assert.deepEqual(response.result.notes, []);
  });

  // covers: E3-1.2
  it('drops inherited attributes, children, className, style, handlers and function props', async () => {
    const response = await service.request('inspect', { dir: project.dir, package: 'synthetic-typed' });

    assert.equal(response.ok, true, JSON.stringify(response));
    assert.deepEqual(response.result.components.map((component) => component.name).sort(), ['Card', 'Panel']);
    const card = propsOf(response, 'Card').map((prop) => prop.name).sort();
    assert.deepEqual(card, ['count', 'dense', 'heading', 'meta', 'tone']);
    const tone = propsOf(response, 'Card').find((prop) => prop.name === 'tone');
    assert.deepEqual(tone, { name: 'tone', type: 'enum', values: ['quiet', 'loud', 'neutral'] });
    const panel = propsOf(response, 'Panel').map((prop) => prop.name).sort();
    assert.deepEqual(panel, ['open', 'side']);
  });

  // covers: E3-1.3
  it('lists the components of a package without types with empty props and a note that says so', async () => {
    const response = await service.request('inspect', { dir: project.dir, package: 'synthetic-untyped' });

    assert.equal(response.ok, true, JSON.stringify(response));
    const names = response.result.components.map((component) => component.name).sort();
    assert.deepEqual(names, ['Gauge', 'Meter']);
    for (const component of response.result.components) {
      assert.deepEqual(component.props, []);
    }
    assert.ok(response.result.notes.length > 0);
    assert.match(response.result.notes.join('\n'), /types?|declarations?/i);
  });

  // covers: E3-1.3
  it('names the file bound, answers within 30 s and keeps serving when the types span more than 4000 files', { timeout: BOUND_TEST_TIMEOUT_MS }, async () => {
    const generated = await makeProject(['synthetic-ui']);
    try {
      const files = Array.from({ length: 4_200 }, (_, index) => ({
        file: `m${index}`,
        content: `export interface M${index} { a: string }\n`,
      }));
      await writeGeneratedPackage(generated, 'synthetic-many-files', files);

      const started = Date.now();
      const response = await service.request('inspect', { dir: generated.dir, package: 'synthetic-many-files' });
      const elapsed = Date.now() - started;

      assert.equal(response.ok, true, JSON.stringify(response));
      assert.ok(elapsed < RESPONSE_BUDGET_MS, `inspect took ${elapsed} ms`);
      assert.deepEqual(propsOf(response, 'Button'), []);
      assert.match(response.result.notes.join('\n'), /4[\s,._  ]?000|files?/i);

      const next = await service.request('inspect', { dir: generated.dir, package: 'synthetic-ui' });
      assert.deepEqual(byName(propsOf(next, 'Button')), byName(BUTTON_PROPS));
    } finally {
      await generated.remove();
    }
  });

  // covers: E3-1.3
  it('names the byte bound, answers within 30 s and keeps serving when the types exceed 64 MiB', { timeout: BOUND_TEST_TIMEOUT_MS }, async () => {
    const generated = await makeProject(['synthetic-ui']);
    try {
      const padding = ` ${'x'.repeat(MEBIBYTE - 64)} `;
      const files = Array.from({ length: 72 }, (_, index) => ({
        file: `b${index}`,
        content: `export interface B${index} { a: string }\n/*${padding}*/\n`,
      }));
      await writeGeneratedPackage(generated, 'synthetic-many-bytes', files);

      const started = Date.now();
      const response = await service.request('inspect', { dir: generated.dir, package: 'synthetic-many-bytes' });
      const elapsed = Date.now() - started;

      assert.equal(response.ok, true, JSON.stringify(response));
      assert.ok(elapsed < RESPONSE_BUDGET_MS, `inspect took ${elapsed} ms`);
      assert.deepEqual(propsOf(response, 'Button'), []);
      assert.match(response.result.notes.join('\n'), /64\s*(MiB|Mio|MB)|bytes?/i);

      const next = await service.request('inspect', { dir: generated.dir, package: 'synthetic-ui' });
      assert.deepEqual(byName(propsOf(next, 'Button')), byName(BUTTON_PROPS));
    } finally {
      await generated.remove();
    }
  });
});

describe('props never run package code', () => {
  let scratch;
  let project;
  let witness;

  before(async () => {
    scratch = await mkdtemp(path.join(os.tmpdir(), 'uigen-witness-'));
    witness = path.join(scratch, 'witness.txt');
    project = await makeProject(['synthetic-tripwire']);
  });

  after(async () => {
    await project?.remove();
    await rm(scratch, { recursive: true, force: true });
  });

  // covers: E3-1.4
  it('does not execute index.js in Node while inspecting or reading props', async () => {
    const indexFile = path.join(sourcesModules(project), 'synthetic-tripwire', 'index.js');
    process.env.UIGEN_WITNESS = witness;
    try {
      await import(pathToFileURL(indexFile).href);
      assert.equal(await exists(witness), true, 'the tripwire must fire when Node does load the package');
      await rm(witness);

      const installed = await readInstalledPackage(project.dir, 'synthetic-tripwire');
      const read = await readProps(installed, LIMITS);
      assert.deepEqual(read.components.get('Tripwire'), [{ name: 'mood', type: 'enum', values: ['calm', 'loud'] }]);
      assert.equal(await exists(witness), false, 'readProps executed the package in Node');

      const service = await startService();
      try {
        const response = await service.request('inspect', { dir: project.dir, package: 'synthetic-tripwire' });
        assert.equal(response.ok, true, JSON.stringify(response));
        assert.deepEqual(propsOf(response, 'Tripwire'), [{ name: 'mood', type: 'enum', values: ['calm', 'loud'] }]);
      } finally {
        await service.stop();
      }
      assert.equal(await exists(witness), false, 'inspect executed the package in Node');
    } finally {
      delete process.env.UIGEN_WITNESS;
    }
  });
});
