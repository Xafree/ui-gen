import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  ComponentFileTooLargeError,
  NodeComponentLibrary,
} from '../../../../../src/modules/sync/adapters/node-fs/node-component-library';

let root: string;
const library = new NodeComponentLibrary();

async function write(relativePath: string, text = 'export {};\n'): Promise<string> {
  const path = join(root, relativePath);
  await mkdir(join(path, '..'), { recursive: true });
  await writeFile(path, text, 'utf8');
  return path;
}

beforeEach(async () => {
  root = await mkdtemp(join(tmpdir(), 'ui-gen-library-'));
});

afterEach(async () => {
  vi.restoreAllMocks();
  await rm(root, { recursive: true, force: true, maxRetries: 3 });
});

// covers: FCT-0026
describe('listing the component files', () => {
  it('lists the TypeScript files of the folder and its subfolders, sorted', async () => {
    const card = await write('hz-card.ts');
    const badge = await write('badge/hz-badge.ts');
    await write('README.md');
    await write('styles.css');
    await write('hz-card.d.ts');

    expect(await library.listFiles(root)).toEqual([badge, card]);
  });

  it('skips hidden folders and build folders', async () => {
    const kept = await write('ui/hz-a.ts');
    await write('node_modules/lib/index.ts');
    await write('.cache/hz-b.ts');
    await write('dist/hz-c.ts');

    expect(await library.listFiles(root)).toEqual([kept]);
  });

  it('keeps a folder whose name merely contains a skipped word', async () => {
    const kept = await write('outer/hz-a.ts');

    expect(await library.listFiles(root)).toEqual([kept]);
  });

  it('lists nothing for a folder that does not exist or that is a file', async () => {
    const file = await write('hz-a.ts');

    expect(await library.listFiles(join(root, 'missing'))).toEqual([]);
    expect(await library.listFiles(file)).toEqual([]);
  });
});

describe('reading a component file', () => {
  it('reads the exact text', async () => {
    const path = await write('hz-a.ts', "class A {}\r\n// é\n");

    expect(await library.readText(path)).toBe("class A {}\r\n// é\n");
  });

  it('refuses a file over one megabyte', async () => {
    const path = await write('hz-big.ts', 'x'.repeat(1_000_001));

    await expect(library.readText(path)).rejects.toBeInstanceOf(ComponentFileTooLargeError);
  });

  it('accepts a file of exactly one megabyte', async () => {
    const path = await write('hz-edge.ts', 'x'.repeat(1_000_000));

    expect(await library.readText(path)).toHaveLength(1_000_000);
  });

  it('fails for a missing file', async () => {
    await expect(library.readText(join(root, 'missing.ts'))).rejects.toMatchObject({ code: 'ENOENT' });
  });
});

describe('watching the component files', () => {
  it('reports the path of a file written in a subfolder, until stopped', async () => {
    await mkdir(join(root, 'badge'));
    const changed: string[] = [];
    const stop = library.watch(root, (path) => {
      changed.push(path);
    });

    await writeFile(join(root, 'badge', 'hz-badge.ts'), 'class A {}\n', 'utf8');
    await vi.waitFor(() => {
      expect(changed).toContain(join(root, 'badge', 'hz-badge.ts'));
    });
    stop();
  });

  it('warns and returns a no-op when the folder cannot be watched', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);

    const stop = library.watch(join(root, 'missing'), () => undefined);

    expect(warn).toHaveBeenCalledOnce();
    expect(() => {
      stop();
    }).not.toThrow();
  });
});
