import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createCanvasHandler } from '../../../src/main/canvas/canvas-handler';
import type { CanvasCompute } from '../../../src/modules/canvas/ports/canvas-compute';
import {
  MAX_COMPUTE_TEXT_LENGTH,
  MAX_TSCONFIG_LENGTH,
} from '../../../src/modules/sync/adapters/utility-process/compute-protocol';

const ORIGIN = 'uigen-canvas://canvas';

let root: string;
let transpile: ReturnType<typeof vi.fn<CanvasCompute['transpile']>>;

beforeEach(async () => {
  root = await mkdtemp(join(tmpdir(), 'ui-gen-canvas-bounds-'));
  await mkdir(join(root, 'src'), { recursive: true });
  await writeFile(join(root, 'src', 'small.ts'), 'export const small = 1;\n');
  transpile = vi.fn<CanvasCompute['transpile']>(() => Promise.resolve('export const small = 1;\n'));
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(async () => {
  vi.restoreAllMocks();
  await rm(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
});

async function get(path: string): Promise<Response> {
  const handle = createCanvasHandler({
    projectRoot: () => root,
    compute: { readModel: () => Promise.reject(new Error('unused')), transpile },
    studio: { harnessScript: join(root, 'harness.js'), packagesRoot: join(root, 'node_modules') },
  });
  return handle(new Request(`${ORIGIN}${path}`));
}

describe('a project file larger than the analysis process accepts', () => {
  it.each(['/src/big.ts', '/src/big.ts?v=1'])('answers 413 at once for %s, without asking the process', async (path) => {
    await writeFile(join(root, 'src', 'big.ts'), 'x'.repeat(MAX_COMPUTE_TEXT_LENGTH + 1));

    const response = await get(path);

    expect(response.status).toBe(413);
    expect(transpile).not.toHaveBeenCalled();
  });

  it('answers 413 when the tsconfig is too large', async () => {
    await writeFile(join(root, 'tsconfig.json'), ' '.repeat(MAX_TSCONFIG_LENGTH + 1));

    const response = await get('/src/small.ts');

    expect(response.status).toBe(413);
    expect(transpile).not.toHaveBeenCalled();
  });

  it('serves a file of exactly the maximum size', async () => {
    await writeFile(join(root, 'src', 'edge.ts'), 'x'.repeat(MAX_COMPUTE_TEXT_LENGTH));
    await writeFile(join(root, 'tsconfig.json'), ' '.repeat(MAX_TSCONFIG_LENGTH));

    const response = await get('/src/edge.ts');

    expect(response.status).toBe(200);
    expect(transpile).toHaveBeenCalledOnce();
  });
});
