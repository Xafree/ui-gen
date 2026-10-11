import { mkdir, mkdtemp, readFile, readdir, rename, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout as pause } from 'node:timers/promises';
import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from 'vitest';
import { NodeProjectFileSystem } from '../../../../../src/modules/project/adapters/node-fs/node-project-file-system';
import {
  ProjectWriteError,
  type ProjectEntry,
  type ProjectFileSystem,
} from '../../../../../src/modules/project/ports/project-file-system';

vi.mock('node:fs/promises', async (importOriginal) => {
  const actual = await importOriginal<typeof import('node:fs/promises')>();
  return { ...actual, rename: vi.fn(actual.rename) };
});

vi.mock('node:timers/promises', () => ({ setTimeout: vi.fn(() => Promise.resolve()) }));

let root: string;
let fileSystem: ProjectFileSystem;
let errorSpy: MockInstance<typeof console.error>;

async function rejectionOf(promise: Promise<unknown>): Promise<ProjectWriteError> {
  try {
    await promise;
  } catch (error: unknown) {
    if (error instanceof ProjectWriteError) return error;
    throw new Error('expected a ProjectWriteError', { cause: error });
  }
  throw new Error('expected the promise to reject');
}

function systemError(code: string): Error {
  return Object.assign(new Error(`${code}: simulated by the test`), { code });
}

const projectEntries: readonly ProjectEntry[] = [
  { kind: 'file', relativePath: 'package.json', content: '{ "name": "@kit-horizon/ui" }\n' },
  { kind: 'folder', relativePath: 'src/components' },
];

beforeEach(async () => {
  root = await mkdtemp(join(tmpdir(), 'uigen-node-fs-safety-'));
  fileSystem = new NodeProjectFileSystem();
  errorSpy = vi.spyOn(console, 'error').mockImplementation(() => undefined);
});

afterEach(async () => {
  vi.restoreAllMocks();
  vi.resetAllMocks();
  await rm(root, { recursive: true, force: true });
});

describe('NodeProjectFileSystem.listSourceFiles links', () => {
  it('does not follow a link to a folder outside the project', async () => {
    const project = join(root, 'project');
    const outside = join(root, 'outside');
    await mkdir(join(project, 'src'), { recursive: true });
    await writeFile(join(project, 'src', 'index.ts'), 'export {};\n', 'utf8');
    await mkdir(outside);
    await writeFile(join(outside, 'secret.ts'), 'export const secret = 1;\n', 'utf8');
    await symlink(outside, join(project, 'linked'), 'junction');

    const sources = await fileSystem.listSourceFiles(project);

    expect(sources.map((source) => source.relativePath)).toEqual(['src/index.ts']);
  });
});

describe('NodeProjectFileSystem.listSourceFiles failure', () => {
  it('wraps an unexpected error with its cause and logs that cause', async () => {
    const folder = join(root, 'missing');

    const error = await rejectionOf(fileSystem.listSourceFiles(folder));

    expect(error.path).toBe(folder);
    expect(error.code).toBe('ENOENT');
    expect(error.cause).toMatchObject({ code: 'ENOENT' });
    expect(errorSpy.mock.calls.flat()).toContain(error.cause);
  });
});

describe('NodeProjectFileSystem.listSourceFiles order', () => {
  it('sorts the sources alphabetically whatever order the file system lists them in', async () => {
    const project = join(root, 'project');
    await mkdir(join(project, 'src'), { recursive: true });
    for (const name of ['eclair.ts', 'f.ts', 'zebra.ts', 'éclair.ts']) {
      await writeFile(join(project, 'src', name), 'export {};\n', 'utf8');
    }

    const sources = await fileSystem.listSourceFiles(project);

    expect(sources.map((source) => source.relativePath)).toEqual([
      'src/eclair.ts',
      'src/éclair.ts',
      'src/f.ts',
      'src/zebra.ts',
    ]);
  });

  it('falls back to the code unit order for paths that the collation cannot tell apart', async () => {
    vi.spyOn(String.prototype, 'localeCompare').mockReturnValue(0);
    const project = join(root, 'project');
    await mkdir(join(project, 'src'), { recursive: true });
    for (const name of ['a.ts', 'B.ts']) {
      await writeFile(join(project, 'src', name), 'export {};\n', 'utf8');
    }

    const sources = await fileSystem.listSourceFiles(project);

    expect(sources.map((source) => source.relativePath)).toEqual(['src/B.ts', 'src/a.ts']);
  });
});

describe('NodeProjectFileSystem paths that leave the folder', () => {
  it.each(['../escape.json', 'src/../../escape.json', '.', ''])(
    'refuses to write %j, logs it and writes nothing',
    async (relativePath) => {
      const project = join(root, 'project');
      await mkdir(project);

      const error = await rejectionOf(fileSystem.writeFileAtomically(project, relativePath, '{}\n'));

      expect(error.code).toBe('EINVAL');
      expect(errorSpy).toHaveBeenCalledWith(expect.stringContaining(JSON.stringify(relativePath)));
      expect(await readdir(root)).toEqual(['project']);
      expect(await readdir(project)).toEqual([]);
    },
  );

  it('refuses to read a file outside the folder, even when it exists, and logs it', async () => {
    const project = join(root, 'project');
    await mkdir(project);
    await writeFile(join(root, 'outside.json'), '{ "secret": true }\n', 'utf8');

    const error = await rejectionOf(fileSystem.readJsonFile(project, '../outside.json'));

    expect(error.code).toBe('EINVAL');
    expect(errorSpy).toHaveBeenCalledWith(expect.stringContaining('"../outside.json"'));
  });

  // covers: FCT-0013
  it('leaves nothing behind and logs it when an entry would be written outside the target folder', async () => {
    const entries: readonly ProjectEntry[] = [
      ...projectEntries,
      { kind: 'file', relativePath: '../escape.json', content: '{}\n' },
    ];

    const error = await rejectionOf(fileSystem.createFolderAtomically(join(root, 'kit-horizon'), entries));

    expect(error.code).toBe('EINVAL');
    expect(errorSpy).toHaveBeenCalledWith(expect.stringContaining('"../escape.json"'));
    expect(await readdir(root)).toEqual([]);
  });
});

describe('NodeProjectFileSystem.createFolderAtomically over an existing target', () => {
  // covers: FCT-0004
  it('refuses a target folder that is not empty and keeps everything in it', async () => {
    const target = join(root, 'kit-horizon');
    await mkdir(target);
    await writeFile(join(target, 'keep.txt'), 'keep me', 'utf8');

    const error = await rejectionOf(fileSystem.createFolderAtomically(target, projectEntries));

    expect(error.code).toBe('ENOTEMPTY');
    expect(await readdir(root)).toEqual(['kit-horizon']);
    expect(await readdir(target)).toEqual(['keep.txt']);
    expect(await readFile(join(target, 'keep.txt'), 'utf8')).toBe('keep me');
  });

  // covers: FCT-0013
  it('refuses to replace a file that stands at the target path', async () => {
    const target = join(root, 'kit-horizon');
    await writeFile(target, 'a file of the user', 'utf8');

    const error = await rejectionOf(fileSystem.createFolderAtomically(target, projectEntries));

    expect(error.code).toBe('EEXIST');
    expect(await readdir(root)).toEqual(['kit-horizon']);
    expect(await readFile(target, 'utf8')).toBe('a file of the user');
  });
});

describe('NodeProjectFileSystem.createFolderAtomically final rename', () => {
  // covers: FCT-0003
  it.each(['EPERM', 'EBUSY', 'EACCES'])('retries the final rename once after %s and creates the folder', async (code) => {
    const target = join(root, 'kit-horizon');
    vi.mocked(rename).mockRejectedValueOnce(systemError(code));

    await fileSystem.createFolderAtomically(target, projectEntries);

    expect(rename).toHaveBeenCalledTimes(2);
    expect(vi.mocked(pause).mock.calls).toEqual([[100]]);
    expect(await readdir(root)).toEqual(['kit-horizon']);
    const entries = await readdir(target);
    expect(entries).toHaveLength(2);
    expect(entries).toEqual(expect.arrayContaining(['package.json', 'src']));
  });

  // covers: FCT-0013
  it('gives up after three retries 100 ms apart, logs the cause and leaves nothing behind', async () => {
    vi.mocked(rename).mockRejectedValue(systemError('EBUSY'));

    const error = await rejectionOf(fileSystem.createFolderAtomically(join(root, 'kit-horizon'), projectEntries));

    expect(error.code).toBe('EBUSY');
    expect(error.cause).toMatchObject({ code: 'EBUSY' });
    expect(errorSpy.mock.calls.flat()).toContain(error.cause);
    expect(rename).toHaveBeenCalledTimes(4);
    expect(vi.mocked(pause).mock.calls).toEqual([[100], [100], [100]]);
    expect(await readdir(root)).toEqual([]);
  });

  it('does not retry an error that is not transient', async () => {
    vi.mocked(rename).mockRejectedValueOnce(systemError('ENOSPC'));

    const error = await rejectionOf(fileSystem.createFolderAtomically(join(root, 'kit-horizon'), projectEntries));

    expect(error.code).toBe('ENOSPC');
    expect(rename).toHaveBeenCalledOnce();
    expect(pause).not.toHaveBeenCalled();
    expect(await readdir(root)).toEqual([]);
  });

  // covers: FCT-0013
  it('puts back the empty folder it replaced when the rename keeps failing', async () => {
    const target = join(root, 'kit-horizon');
    await mkdir(target);
    vi.mocked(rename).mockRejectedValue(systemError('EBUSY'));

    const error = await rejectionOf(fileSystem.createFolderAtomically(target, projectEntries));

    expect(error.code).toBe('EBUSY');
    expect(await readdir(root)).toEqual(['kit-horizon']);
    expect(await readdir(target)).toEqual([]);
  });
});

describe('NodeProjectFileSystem.readJsonFile unreadable entry', () => {
  it('logs the cause of an entry it cannot read', async () => {
    await mkdir(join(root, 'package.json'));

    await fileSystem.readJsonFile(root, 'package.json');

    expect(errorSpy).toHaveBeenCalledWith(
      expect.stringContaining('package.json'),
      expect.objectContaining({ code: 'EISDIR' }),
    );
  });
});
