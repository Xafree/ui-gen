import { mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from 'vitest';
import { NodeProjectFileSystem } from '../../../../../src/modules/project/adapters/node-fs/node-project-file-system';
import {
  ProjectWriteError,
  type ProjectEntry,
  type ProjectFileSystem,
} from '../../../../../src/modules/project/ports/project-file-system';

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
});

describe('NodeProjectFileSystem paths that leave the folder', () => {
  it.each(['../escape.json', 'src/../../escape.json', '.', ''])(
    'refuses to write %j and writes nothing',
    async (relativePath) => {
      const project = join(root, 'project');
      await mkdir(project);

      const error = await rejectionOf(fileSystem.writeFileAtomically(project, relativePath, '{}\n'));

      expect(error.code).toBe('EINVAL');
      expect(await readdir(root)).toEqual(['project']);
      expect(await readdir(project)).toEqual([]);
    },
  );

  it('refuses to read a file outside the folder, even when it exists', async () => {
    const project = join(root, 'project');
    await mkdir(project);
    await writeFile(join(root, 'outside.json'), '{ "secret": true }\n', 'utf8');

    const error = await rejectionOf(fileSystem.readJsonFile(project, '../outside.json'));

    expect(error.code).toBe('EINVAL');
  });

  // covers: FCT-0013
  it('leaves nothing behind when an entry would be written outside the target folder', async () => {
    const entries: readonly ProjectEntry[] = [
      ...projectEntries,
      { kind: 'file', relativePath: '../escape.json', content: '{}\n' },
    ];

    const error = await rejectionOf(fileSystem.createFolderAtomically(join(root, 'kit-horizon'), entries));

    expect(error.code).toBe('EINVAL');
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
