import { mkdir, mkdtemp, open, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { NodeProjectFileSystem } from '../../../../../src/modules/project/adapters/node-fs/node-project-file-system';
import type { ProjectFileSystem } from '../../../../../src/modules/project/ports/project-file-system';

vi.mock('node:fs/promises', async (importOriginal) => {
  const actual = await importOriginal<typeof import('node:fs/promises')>();
  return { ...actual, readFile: vi.fn(actual.readFile), open: vi.fn(actual.open) };
});

const HELPER_COUNT = 1997;

let root: string;
let fileSystem: ProjectFileSystem;

function helperPath(index: number): string {
  return `src/helpers/helper-${String(index).padStart(4, '0')}.ts`;
}

function helperSource(index: number): string {
  return `export const helper${String(index)} = (value: number): number => value + ${String(index)};\n`;
}

function componentSource(tag: string, className: string): string {
  return `import { LitElement, html } from 'lit';

export class ${className} extends LitElement {
  render() {
    return html\`<div part="${tag}"><slot></slot></div>\`;
  }
}
customElements.define('${tag}', ${className});
`;
}

const COMPONENTS: Readonly<Record<string, string>> = {
  'src/components/kh-button.ts': componentSource('kh-button', 'KhButton'),
  'src/components/kh-card.ts': componentSource('kh-card', 'KhCard'),
  'src/components/kh-dialog.ts': componentSource('kh-dialog', 'KhDialog'),
};

const BEYOND_LIMIT: Readonly<Record<string, string>> = {
  'src/zz/zz-alert.ts': componentSource('zz-alert', 'ZzAlert'),
  'src/zz/zz-badge.ts': componentSource('zz-badge', 'ZzBadge'),
  'src/zz/zz-chip.ts': componentSource('zz-chip', 'ZzChip'),
  'src/zz/zz-menu.ts': componentSource('zz-menu', 'ZzMenu'),
  'src/zz/zz-toast.ts': componentSource('zz-toast', 'ZzToast'),
};

async function writeTree(base: string, files: Readonly<Record<string, string>>): Promise<void> {
  const folders = new Set(Object.keys(files).map((relativePath) => join(base, ...relativePath.split('/'), '..')));
  for (const folder of folders) await mkdir(folder, { recursive: true });
  await Promise.all(
    Object.entries(files).map(([relativePath, content]) => writeFile(join(base, ...relativePath.split('/')), content, 'utf8')),
  );
}

function largeLibrary(): Record<string, string> {
  const files: Record<string, string> = {};
  for (let index = 0; index < HELPER_COUNT; index += 1) files[helperPath(index)] = helperSource(index);
  return { ...files, ...COMPONENTS, ...BEYOND_LIMIT };
}

function pathsGivenTo(calls: readonly (readonly unknown[])[]): string[] {
  return calls.map((call) => String(call[0]));
}

beforeEach(async () => {
  root = await mkdtemp(join(tmpdir(), 'uigen-node-fs-bounds-'));
  await writeTree(root, largeLibrary());
  fileSystem = new NodeProjectFileSystem();
  vi.mocked(readFile).mockClear();
  vi.mocked(open).mockClear();
});

afterEach(async () => {
  vi.restoreAllMocks();
  await rm(root, { recursive: true, force: true });
});

describe('NodeProjectFileSystem.listSourceFiles in a library of more than 2 000 sources', () => {
  // covers: FCT-0025
  it('returns only the first 2 000 sources in alphabetical order of their path', async () => {
    const sources = await fileSystem.listSourceFiles(root);

    expect(sources).toHaveLength(2000);
    expect(sources[0]?.relativePath).toBe('src/components/kh-button.ts');
    expect(sources[1]?.relativePath).toBe('src/components/kh-card.ts');
    expect(sources[2]?.relativePath).toBe('src/components/kh-dialog.ts');
    expect(sources[3]?.relativePath).toBe('src/helpers/helper-0000.ts');
    expect(sources[1999]?.relativePath).toBe('src/helpers/helper-1996.ts');
    expect(sources[1999]?.content).toBe(helperSource(1996));
    expect(sources.map((source) => source.relativePath).filter((path) => path.startsWith('src/zz/'))).toEqual([]);
  }, 60_000);

  // covers: FCT-0025
  it('never opens the sources beyond the first 2 000', async () => {
    await fileSystem.listSourceFiles(root);

    const touched = [...pathsGivenTo(vi.mocked(readFile).mock.calls), ...pathsGivenTo(vi.mocked(open).mock.calls)];
    expect(touched.filter((path) => path.includes(join('src', 'zz')))).toEqual([]);
  }, 60_000);

  // covers: FCT-0025
  it('reads every source of a library of exactly 2 000 sources', async () => {
    for (const relativePath of Object.keys(BEYOND_LIMIT)) await rm(join(root, ...relativePath.split('/')));

    const sources = await fileSystem.listSourceFiles(root);

    expect(sources).toHaveLength(2000);
    expect(sources[1999]?.relativePath).toBe('src/helpers/helper-1996.ts');
  }, 60_000);
});
