import { mkdir, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { NodeComponentLibrary } from '../../../../../src/modules/sync/adapters/node-fs/node-component-library';

let root: string;
const library = new NodeComponentLibrary();

beforeEach(async () => {
  root = await mkdtemp(join(tmpdir(), 'ui-gen-library-stopped-'));
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(async () => {
  vi.restoreAllMocks();
  await rm(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
});

describe('a watch that cannot run', () => {
  it('reports that it stopped when the folder is missing', () => {
    const onStopped = vi.fn();

    library.watch(join(root, 'missing'), () => undefined, onStopped);

    expect(onStopped).toHaveBeenCalledOnce();
  });

  it('does not report a stop for a running watch, nor after it is stopped on purpose', async () => {
    await mkdir(join(root, 'components'));
    const onStopped = vi.fn();

    const stop = library.watch(join(root, 'components'), () => undefined, onStopped);
    stop();

    expect(onStopped).not.toHaveBeenCalled();
  });
});
