import { mkdir, mkdtemp, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createCanvasHandler } from '../../../src/main/canvas/canvas-handler';
import { transpileProjectModule } from '../../../src/modules/canvas/adapters/typescript/typescript-transpiler';
import type { CanvasCompute } from '../../../src/modules/canvas/ports/canvas-compute';
import { createTypeScriptComponentParser } from '../../../src/modules/sync/adapters/typescript/typescript-component-parser';
import { readComponent } from '../../../src/modules/sync/application/read-component';
import { PRICING_CARD, readFixture } from '../../modules/sync/sync-fixtures';

const ORIGIN = 'uigen-canvas://canvas';
const parser = createTypeScriptComponentParser();

const compute: CanvasCompute = {
  readModel: (text) => Promise.resolve(readComponent(text, parser)),
  transpile: (source, tsconfig, fileName) => Promise.resolve(transpileProjectModule(source, tsconfig, fileName)),
};

let root: string;
let project: string;
let studioRoot: string;
let openProject: string | null;

beforeEach(async () => {
  root = await mkdtemp(join(tmpdir(), 'ui-gen-canvas-handler-'));
  project = join(root, 'kit');
  studioRoot = join(root, 'studio');
  openProject = project;
  await mkdir(join(project, 'src', 'components'), { recursive: true });
  await mkdir(join(project, 'tokens'), { recursive: true });
  await mkdir(join(project, '.git'), { recursive: true });
  await mkdir(join(studioRoot, 'node_modules', 'lit'), { recursive: true });
  await mkdir(join(studioRoot, 'node_modules', '@lit', 'reactive-element'), { recursive: true });
  await mkdir(join(studioRoot, 'node_modules', 'secret-package'), { recursive: true });
  await writeFile(join(project, 'src', 'components', 'hz-pricing-card.ts'), readFixture(PRICING_CARD));
  await writeFile(join(project, 'src', 'components', 'helper.ts'), 'export const helper: number = 1;\n');
  await writeFile(join(project, 'src', 'components', 'broken.ts'), 'export const = ;\n');
  await writeFile(join(project, 'tokens', 'tokens.css'), ':root { --hz-color-bg: red; }\n');
  await writeFile(join(project, 'logo.svg'), '<svg xmlns="http://www.w3.org/2000/svg"/>');
  await writeFile(join(project, 'notes.md'), 'notes');
  await writeFile(join(project, 'page.html'), '<script>alert(1)</script>');
  await writeFile(join(project, '.git', 'config'), 'secret');
  await writeFile(join(project, 'tsconfig.json'), '{ "compilerOptions": { "experimentalDecorators": true } }');
  await writeFile(join(root, 'outside.js'), 'export const leaked = true;\n');
  await writeFile(join(studioRoot, 'harness.js'), 'export {};\n');
  await writeFile(join(studioRoot, 'node_modules', 'lit', 'index.js'), 'export const lit = 1;\n');
  await writeFile(join(studioRoot, 'node_modules', 'lit', 'index.js.map'), '{}');
  await writeFile(join(studioRoot, 'node_modules', 'lit', 'README.md'), 'readme');
  await writeFile(join(studioRoot, 'node_modules', '@lit', 'reactive-element', 'reactive-element.js'), 'export const r = 1;\n');
  await writeFile(join(studioRoot, 'node_modules', 'secret-package', 'index.js'), 'export const secret = 1;\n');
});

afterEach(async () => {
  vi.restoreAllMocks();
  await rm(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
});

function handler() {
  return createCanvasHandler({
    projectRoot: () => openProject,
    compute,
    studio: { harnessScript: join(studioRoot, 'harness.js'), packagesRoot: join(studioRoot, 'node_modules') },
  });
}

async function get(path: string, init?: RequestInit): Promise<Response> {
  return handler()(new Request(`${ORIGIN}${path}`, init));
}

describe('the canvas harness page', () => {
  it('is served with a content security policy that closes the network', async () => {
    const response = await get('/__studio/harness.html?component=src%2Fa.ts');
    const policy = response.headers.get('Content-Security-Policy') ?? '';

    expect(response.status).toBe(200);
    expect(response.headers.get('Content-Type')).toBe('text/html; charset=utf-8');
    expect(policy).toContain("default-src uigen-canvas:");
    expect(policy).toContain("connect-src 'none'");
    expect(policy).not.toMatch(/script-src[^;]*unsafe-inline/);
    expect(await response.text()).toContain('<link rel="stylesheet" href="/tokens/tokens.css">');
  });

  it('leaves the tokens out when the project has none', async () => {
    await rm(join(project, 'tokens', 'tokens.css'));

    const page = await (await get('/__studio/harness.html')).text();

    expect(page).not.toContain('tokens.css');
  });

  it('serves its script', async () => {
    const response = await get('/__studio/harness.js');

    expect(response.status).toBe(200);
    expect(response.headers.get('Content-Type')).toBe('text/javascript; charset=utf-8');
    expect(await response.text()).toBe('export {};\n');
  });
});

describe('the lit packages of the studio', () => {
  it('serves the files of the allowed packages, scoped or not', async () => {
    const lit = await get('/__studio/packages/lit/index.js');
    const reactive = await get('/__studio/packages/@lit/reactive-element/reactive-element.js');

    expect(await lit.text()).toBe('export const lit = 1;\n');
    expect(await reactive.text()).toBe('export const r = 1;\n');
    expect(lit.headers.get('Access-Control-Allow-Origin')).toBe('*');
  });

  it.each([
    ['a package that is not served', '/__studio/packages/secret-package/index.js'],
    ['no file', '/__studio/packages/lit'],
    ['a file of unknown type', '/__studio/packages/lit/README.md'],
    ['a missing file', '/__studio/packages/lit/missing.js'],
    ['an unknown studio asset', '/__studio/other.js'],
    ['a package folder alone', '/__studio/packages'],
  ])('refuses %s', async (_label, path) => {
    expect((await get(path)).status).toBe(404);
  });

  it('serves the source maps of the packages as JSON', async () => {
    expect((await get('/__studio/packages/lit/index.js.map')).headers.get('Content-Type')).toBe(
      'application/json; charset=utf-8',
    );
  });
});

describe('the files of the opened project', () => {
  it('serves a component as an instrumented, transpiled module', async () => {
    const response = await get('/src/components/hz-pricing-card.ts?v=abc');
    const code = await response.text();

    expect(response.status).toBe(200);
    expect(response.headers.get('Content-Type')).toBe('text/javascript; charset=utf-8');
    expect(response.headers.get('Cache-Control')).toBe('no-store');
    expect(response.headers.get('X-Content-Type-Options')).toBe('nosniff');
    expect(code).toContain('data-uigen-node="0"');
    expect(code).toContain('__decorate');
    expect(code).not.toContain('@customElement(');
  });

  it('serves a plain module the same way', async () => {
    const code = await (await get('/src/components/helper.ts')).text();

    expect(code).toContain('export const helper = 1');
  });

  it('serves a .js request from the .ts file next to it', async () => {
    const response = await get('/src/components/helper.js');

    expect(response.status).toBe(200);
    expect(await response.text()).toContain('export const helper = 1');
  });

  it('serves styles and images untouched', async () => {
    const css = await get('/tokens/tokens.css');
    const svg = await get('/logo.svg');

    expect(css.headers.get('Content-Type')).toBe('text/css; charset=utf-8');
    expect(await css.text()).toBe(':root { --hz-color-bg: red; }\n');
    expect(svg.headers.get('Content-Type')).toBe('image/svg+xml');
  });

  it('answers 500 when the module cannot be prepared', async () => {
    const logged = vi.spyOn(console, 'error').mockImplementation(() => undefined);

    const response = await get('/src/components/broken.ts');

    expect(response.status).toBe(500);
    expect(logged).toHaveBeenCalledOnce();
  });

  it.each([
    ['a missing file', '/src/components/missing.ts', 404],
    ['a folder', '/src/components', 404],
    ['a type that is not served', '/notes.md', 415],
    ['an html page of the project', '/page.html', 415],
  ])('refuses %s', async (_label, path, status) => {
    expect((await get(path)).status).toBe(status);
  });
});

// covers: FCT-0032
describe('confinement to the opened project', () => {
  it.each([
    ['an encoded slash', '/src/components/..%2f..%2f..%2foutside.js'],
    ['an encoded backslash', '/src/components/..%5c..%5c..%5coutside.js'],
    ['a traversal in a package', '/__studio/packages/lit/..%2fsecret-package%2findex.js'],
    ['a hidden folder', '/.git/config'],
    ['an empty segment', '/src//components/helper.ts'],
    ['a drive letter', '/C%3A/Windows/win.ini'],
  ])('refuses %s', async (_label, path) => {
    vi.spyOn(console, 'warn').mockImplementation(() => undefined);

    expect((await get(path)).status).toBe(400);
  });

  it('cannot climb above the project with encoded dot segments', async () => {
    expect((await get('/src/%2e%2e/%2e%2e/outside.js')).status).toBe(404);
  });

  it('does not follow a link out of the project', async () => {
    await symlink(root, join(project, 'escape'), 'junction');

    expect((await get('/escape/outside.js')).status).toBe(404);
  });

  it('serves nothing while no project is open', async () => {
    openProject = null;

    expect((await get('/tokens/tokens.css')).status).toBe(404);
    expect((await get('/__studio/harness.html')).status).toBe(404);
  });

  it.each(['POST', 'PUT', 'DELETE', 'PATCH'])('refuses %s', async (method) => {
    vi.spyOn(console, 'warn').mockImplementation(() => undefined);

    const response = await get('/tokens/tokens.css', { method, body: 'x' });

    expect(response.status).toBe(405);
  });

  it('refuses another scheme or another host', async () => {
    const other = await handler()(new Request('https://canvas/tokens/tokens.css'));
    const elsewhere = await handler()(new Request('uigen-canvas://elsewhere/tokens/tokens.css'));

    expect(other.status).toBe(404);
    expect(elsewhere.status).toBe(404);
  });
});
