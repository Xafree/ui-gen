import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
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

beforeEach(async () => {
  root = await mkdtemp(join(tmpdir(), 'ui-gen-canvas-nested-'));
  await mkdir(join(root, 'src', 'components'), { recursive: true });
  await writeFile(join(root, 'src', 'components', 'hz-pricing-card.ts'), readFixture(PRICING_CARD));
});

afterEach(async () => {
  await rm(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
});

async function get(path: string): Promise<string> {
  const handler = createCanvasHandler({
    projectRoot: () => root,
    compute,
    studio: { harnessScript: join(root, 'harness.js'), packagesRoot: join(root, 'node_modules') },
  });
  const response = await handler(new Request(`${ORIGIN}${path}`));
  expect(response.status).toBe(200);
  return response.text();
}

describe('the instrumentation of served components', () => {
  it('marks the opened component, requested with a version', async () => {
    expect(await get('/src/components/hz-pricing-card.ts?v=abc')).toContain('data-uigen-node="0"');
  });

  it('transpiles a component imported by the opened one without marking its nodes', async () => {
    const code = await get('/src/components/hz-pricing-card.ts');

    expect(code).not.toContain('data-uigen-node');
    expect(code).toContain('__decorate');
  });

  it('transpiles a .js import resolved to a .ts file without marking its nodes', async () => {
    const code = await get('/src/components/hz-pricing-card.js');

    expect(code).not.toContain('data-uigen-node');
  });
});
