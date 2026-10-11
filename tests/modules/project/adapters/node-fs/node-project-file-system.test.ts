import { execFileSync } from 'node:child_process';
import { mkdir, mkdtemp, readFile, readdir, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir, userInfo } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from 'vitest';
import { NodeProjectFileSystem } from '../../../../../src/modules/project/adapters/node-fs/node-project-file-system';
import {
  ProjectWriteError,
  type ProjectEntry,
  type ProjectFileSystem,
} from '../../../../../src/modules/project/ports/project-file-system';

const PACKAGE_JSON = `{
  "name": "@kit-horizon/ui",
  "version": "0.1.0",
  "description": "Bibliothèque de composants — thème « Horizon »",
  "dependencies": {
    "lit": "^3.3.0"
  }
}
`;

const HZ_BUTTON = `import { LitElement, html, css } from 'lit';

// Bouton principal du kit, retouché à la main
export class HzButton extends LitElement {
  static styles = css\`:host { display: inline-flex; }\`;

  render() {
    return html\`<button part="control"><slot></slot></button>\`;
  }
}
customElements.define('hz-button', HzButton);
`;

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

function loggedValues(): unknown[] {
  return errorSpy.mock.calls.flat();
}

async function writeTree(base: string, files: Readonly<Record<string, string>>): Promise<void> {
  for (const [relativePath, content] of Object.entries(files)) {
    const path = join(base, ...relativePath.split('/'));
    await mkdir(join(path, '..'), { recursive: true });
    await writeFile(path, content, 'utf8');
  }
}

beforeEach(async () => {
  root = await mkdtemp(join(tmpdir(), 'uigen-node-fs-'));
  fileSystem = new NodeProjectFileSystem();
  errorSpy = vi.spyOn(console, 'error').mockImplementation(() => undefined);
});

afterEach(async () => {
  vi.restoreAllMocks();
  await rm(root, { recursive: true, force: true });
});

describe('NodeProjectFileSystem.childPath', () => {
  it('joins a parent folder and a child name with the platform separator', () => {
    expect(fileSystem.childPath(root, 'kit-horizon')).toBe(join(root, 'kit-horizon'));
  });
});

describe('NodeProjectFileSystem.readFolderState', () => {
  it('reports a missing folder', async () => {
    await expect(fileSystem.readFolderState(join(root, 'kit-horizon'))).resolves.toEqual({ kind: 'missing' });
  });

  it('reports a file where a folder is expected', async () => {
    await writeFile(join(root, 'kit-horizon'), 'not a folder', 'utf8');

    await expect(fileSystem.readFolderState(join(root, 'kit-horizon'))).resolves.toEqual({ kind: 'not-a-folder' });
  });

  it('reports an existing empty folder', async () => {
    await mkdir(join(root, 'kit-horizon'));

    await expect(fileSystem.readFolderState(join(root, 'kit-horizon'))).resolves.toEqual({ kind: 'empty' });
  });

  // covers: FCT-0004
  it('counts only the direct entries of a folder that is not empty', async () => {
    const project = join(root, 'kit-horizon');
    await writeTree(project, {
      'package.json': PACKAGE_JSON,
      'src/components/hz-button.ts': HZ_BUTTON,
      'src/index.ts': "export * from './components/hz-button.js';\n",
      '.uigen/settings.json': '{}\n',
    });

    await expect(fileSystem.readFolderState(project)).resolves.toEqual({ kind: 'not-empty', entryCount: 3 });
  });

  it('wraps an unexpected system error with its cause and logs that cause', async () => {
    const locked = join(root, 'locked');
    await mkdir(locked);
    const user = userInfo().username;
    execFileSync('icacls', [locked, '/deny', `${user}:(RD)`]);
    try {
      const error = await rejectionOf(fileSystem.readFolderState(locked));

      expect(error.code).toBe('EPERM');
      expect(error.cause).toMatchObject({ code: 'EPERM' });
      expect(loggedValues()).toContain(error.cause);
    } finally {
      execFileSync('icacls', [locked, '/remove:d', user]);
    }
  });
});

describe('NodeProjectFileSystem.readJsonFile', () => {
  it('reads a JSON file at a slash-separated path relative to the folder', async () => {
    await writeTree(root, { 'tokens/horizon.tokens.json': '{ "color": { "brand": { "$value": "#3b5bdb" } } }\n' });

    await expect(fileSystem.readJsonFile(root, 'tokens/horizon.tokens.json')).resolves.toEqual({
      kind: 'ok',
      value: { color: { brand: { $value: '#3b5bdb' } } },
    });
  });

  it('strips a leading UTF-8 byte order mark before parsing', async () => {
    await writeFile(join(root, 'package.json'), `\uFEFF${PACKAGE_JSON}`, 'utf8');

    await expect(fileSystem.readJsonFile(root, 'package.json')).resolves.toEqual({
      kind: 'ok',
      value: {
        name: '@kit-horizon/ui',
        version: '0.1.0',
        description: 'Bibliothèque de composants — thème « Horizon »',
        dependencies: { lit: '^3.3.0' },
      },
    });
  });

  it('reports a missing file', async () => {
    await expect(fileSystem.readJsonFile(root, 'package.json')).resolves.toEqual({ kind: 'missing' });
  });

  it('reports a file whose content is not valid JSON', async () => {
    await writeFile(join(root, 'package.json'), '{\n  "name": "@kit-horizon/ui",\n', 'utf8');

    await expect(fileSystem.readJsonFile(root, 'package.json')).resolves.toEqual({ kind: 'invalid-json' });
  });

  it('reports an unreadable entry as invalid JSON, not as missing', async () => {
    await mkdir(join(root, 'package.json'));

    await expect(fileSystem.readJsonFile(root, 'package.json')).resolves.toEqual({ kind: 'invalid-json' });
  });
});

describe('NodeProjectFileSystem.listSourceFiles', () => {
  it('lists TypeScript and JavaScript sources recursively, sorted, with slash-separated paths', async () => {
    await writeTree(root, {
      'src/index.ts': "export * from './components/hz-button.js';\n",
      'src/components/hz-button.ts': HZ_BUTTON,
      'src/components/hz-card.mts': 'export const card = 1;\n',
      'scripts/release.mjs': 'console.log("release");\n',
      'legacy/hz-badge.js': 'export const badge = 1;\n',
      'distribution/notes.ts': 'export const notes = 1;\n',
      'README.md': '# Kit Horizon\n',
      'src/components/hz-button.css': ':host { color: red; }\n',
      'src/types.d.ts': 'declare const horizon: string;\n',
      'package.json': PACKAGE_JSON,
    });

    const sources = await fileSystem.listSourceFiles(root);

    expect(sources.map((source) => source.relativePath)).toEqual([
      'distribution/notes.ts',
      'legacy/hz-badge.js',
      'scripts/release.mjs',
      'src/components/hz-button.ts',
      'src/components/hz-card.mts',
      'src/index.ts',
    ]);
    expect(sources.find((source) => source.relativePath === 'src/components/hz-button.ts')?.content).toBe(HZ_BUTTON);
  });

  it('skips dependency, build output and dot folders at any depth', async () => {
    await writeTree(root, {
      'src/index.ts': 'export {};\n',
      'node_modules/lit/index.js': 'export {};\n',
      'dist/index.js': 'export {};\n',
      'out/main/index.js': 'export {};\n',
      'build/bundle.mjs': 'export {};\n',
      'coverage/lcov-report/sorter.js': 'export {};\n',
      '.git/hooks/pre-commit.js': 'export {};\n',
      '.storybook/main.ts': 'export {};\n',
      'packages/core/node_modules/tslib/tslib.js': 'export {};\n',
      'packages/core/.cache/chunk.ts': 'export {};\n',
      'packages/core/src/core.ts': 'export {};\n',
    });

    const sources = await fileSystem.listSourceFiles(root);

    expect(sources.map((source) => source.relativePath)).toEqual(['packages/core/src/core.ts', 'src/index.ts']);
  });

  it('keeps a source of exactly 1 000 000 bytes and skips a larger one, counted in bytes', async () => {
    await writeTree(root, {
      'src/at-limit.ts': 'a'.repeat(1_000_000),
      'src/over-limit.ts': 'a'.repeat(1_000_001),
      'src/accented.ts': 'é'.repeat(500_001),
    });

    const sources = await fileSystem.listSourceFiles(root);

    expect(sources.map((source) => source.relativePath)).toEqual(['src/at-limit.ts']);
    expect(sources[0]?.content.length).toBe(1_000_000);
  });

  it('returns an empty list for a folder without sources', async () => {
    await writeTree(root, { 'README.md': '# Kit Horizon\n' });

    await expect(fileSystem.listSourceFiles(root)).resolves.toEqual([]);
  });
});

describe('NodeProjectFileSystem.createFolderAtomically', () => {
  const projectEntries: readonly ProjectEntry[] = [
    { kind: 'file', relativePath: 'package.json', content: PACKAGE_JSON },
    { kind: 'folder', relativePath: 'src' },
    { kind: 'folder', relativePath: 'src/components' },
    { kind: 'folder', relativePath: '.uigen' },
    { kind: 'file', relativePath: '.uigen/settings.json', content: '{\n  "componentsDir": "src/components"\n}\n' },
  ];

  // covers: FCT-0003
  it('creates the target folder with every folder and file entry', async () => {
    const target = join(root, 'kit-horizon');

    await fileSystem.createFolderAtomically(target, projectEntries);

    expect((await readdir(target)).sort()).toEqual(['.uigen', 'package.json', 'src']);
    expect(await readFile(join(target, 'package.json'), 'utf8')).toBe(PACKAGE_JSON);
    expect(await readdir(join(target, 'src', 'components'))).toEqual([]);
    expect(await readFile(join(target, '.uigen', 'settings.json'), 'utf8')).toBe(
      '{\n  "componentsDir": "src/components"\n}\n',
    );
    expect(await readdir(root)).toEqual(['kit-horizon']);
  });

  it('accepts an existing empty target folder', async () => {
    const target = join(root, 'kit-horizon');
    await mkdir(target);

    await fileSystem.createFolderAtomically(target, projectEntries);

    expect((await readdir(target)).sort()).toEqual(['.uigen', 'package.json', 'src']);
    expect(await readdir(root)).toEqual(['kit-horizon']);
  });

  // covers: FCT-0013
  it('refuses with the system code when the parent folder does not exist, writing nothing', async () => {
    const target = join(root, 'missing-parent', 'kit-horizon');

    const error = await rejectionOf(fileSystem.createFolderAtomically(target, projectEntries));

    expect(error.code).toBe('ENOENT');
    expect(error.cause).toMatchObject({ code: 'ENOENT' });
    expect(loggedValues()).toContain(error.cause);
    expect(await readdir(root)).toEqual([]);
  });

  // covers: FCT-0013
  it('leaves no folder or file behind when an entry cannot be written', async () => {
    const target = join(root, 'kit-horizon');
    const entries: readonly ProjectEntry[] = [
      ...projectEntries,
      { kind: 'file', relativePath: 'a', content: 'first' },
      { kind: 'file', relativePath: 'a/b', content: 'second' },
    ];

    const error = await rejectionOf(fileSystem.createFolderAtomically(target, entries));

    expect(error.code).toMatch(/^E[A-Z]+$/);
    expect(error.cause).toMatchObject({ code: error.code });
    expect(loggedValues()).toContain(error.cause);
    expect(await readdir(root)).toEqual([]);
  });
});

describe('NodeProjectFileSystem.writeFileAtomically', () => {
  it('writes a file at a slash-separated path and leaves no temporary file', async () => {
    await mkdir(join(root, '.uigen'));

    await fileSystem.writeFileAtomically(root, '.uigen/settings.json', '{\n  "componentsDir": "src/components"\n}\n');

    expect(await readFile(join(root, '.uigen', 'settings.json'), 'utf8')).toBe(
      '{\n  "componentsDir": "src/components"\n}\n',
    );
    expect(await readdir(join(root, '.uigen'))).toEqual(['settings.json']);
  });

  it('replaces the content of an existing file', async () => {
    await writeFile(join(root, 'package.json'), '{ "name": "old" }\n', 'utf8');

    await fileSystem.writeFileAtomically(root, 'package.json', PACKAGE_JSON);

    expect(await readFile(join(root, 'package.json'), 'utf8')).toBe(PACKAGE_JSON);
    expect(await readdir(root)).toEqual(['package.json']);
  });

  it('rejects with its cause and leaves no temporary file when a folder holds the target name', async () => {
    await mkdir(join(root, 'package.json'));

    const error = await rejectionOf(fileSystem.writeFileAtomically(root, 'package.json', PACKAGE_JSON));

    expect(error.code).toMatch(/^E[A-Z]+$/);
    expect(error.cause).toMatchObject({ code: error.code });
    expect(loggedValues()).toContain(error.cause);
    expect(await readdir(root)).toEqual(['package.json']);
    expect((await stat(join(root, 'package.json'))).isDirectory()).toBe(true);
    expect(await readdir(join(root, 'package.json'))).toEqual([]);
  });

  it('rejects with its cause and leaves the disk unchanged when a file stands where a folder is expected', async () => {
    await writeFile(join(root, 'notes'), 'keep me', 'utf8');

    const error = await rejectionOf(fileSystem.writeFileAtomically(root, 'notes/settings.json', '{}\n'));

    expect(error.cause).toMatchObject({ code: error.code });
    expect(loggedValues()).toContain(error.cause);
    expect(await readdir(root)).toEqual(['notes']);
    expect(await readFile(join(root, 'notes'), 'utf8')).toBe('keep me');
  });
});
