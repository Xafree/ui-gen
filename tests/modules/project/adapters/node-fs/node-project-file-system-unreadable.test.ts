import { execFileSync } from 'node:child_process';
import { mkdir, mkdtemp, readdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir, userInfo } from 'node:os';
import { join } from 'node:path';
import { inspect } from 'node:util';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { NodeProjectFileSystem } from '../../../../../src/modules/project/adapters/node-fs/node-project-file-system';
import type { ProjectFileSystem } from '../../../../../src/modules/project/ports/project-file-system';

vi.mock('node:fs/promises', async (importOriginal) => {
  const actual = await importOriginal<typeof import('node:fs/promises')>();
  return { ...actual, readdir: vi.fn(actual.readdir) };
});

const actualFs = await vi.importActual<typeof import('node:fs/promises')>('node:fs/promises');

const SECRET_MARKER = 'kh-internal-token-7f3a91';

const LIBRARY: Readonly<Record<string, string>> = {
  'src/index.ts': `export * from './components/kh-button.js';\n// ${SECRET_MARKER}\n`,
  'src/components/kh-button.ts': `import { LitElement, html } from 'lit';

// ${SECRET_MARKER}
export class KhButton extends LitElement {
  render() {
    return html\`<button part="control"><slot></slot></button>\`;
  }
}
customElements.define('kh-button', KhButton);
`,
  'src/components/kh-card.ts': `import { LitElement, html } from 'lit';

// ${SECRET_MARKER}
export class KhCard extends LitElement {
  render() {
    return html\`<article part="card"><slot></slot></article>\`;
  }
}
customElements.define('kh-card', KhCard);
`,
  'src/legacy/zz-badge.ts': `import { LitElement, html } from 'lit';

// ${SECRET_MARKER}
export class ZzBadge extends LitElement {
  render() {
    return html\`<span part="badge"><slot></slot></span>\`;
  }
}
customElements.define('zz-badge', ZzBadge);
`,
};

const READABLE_WITHOUT_LEGACY: readonly string[] = ['src/components/kh-button.ts', 'src/components/kh-card.ts', 'src/index.ts'];

let root: string;
let fileSystem: ProjectFileSystem;
let logged: unknown[][];

async function writeTree(base: string, files: Readonly<Record<string, string>>): Promise<void> {
  for (const [relativePath, content] of Object.entries(files)) {
    const path = join(base, ...relativePath.split('/'));
    await mkdir(join(path, '..'), { recursive: true });
    await writeFile(path, content, 'utf8');
  }
}

async function snapshot(base: string, prefix = ''): Promise<Record<string, string>> {
  const files: Record<string, string> = {};
  for (const entry of await actualFs.readdir(join(base, prefix), { withFileTypes: true })) {
    const relativePath = prefix === '' ? entry.name : `${prefix}/${entry.name}`;
    if (entry.isDirectory()) Object.assign(files, await snapshot(base, relativePath));
    else files[relativePath] = await readFile(join(base, ...relativePath.split('/')), 'utf8');
  }
  return files;
}

function deleteAfterListing(listedFolder: string, victim: string): void {
  vi.mocked(readdir).mockImplementation((async (path: Parameters<typeof actualFs.readdir>[0], options?: unknown) => {
    const entries: unknown = await Reflect.apply(actualFs.readdir, undefined, [path, options]);
    if (String(path) === listedFolder) await rm(victim, { recursive: true, force: true });
    return entries;
  }) as typeof readdir);
}

function refuseListing(refusedFolder: string, code: string): void {
  vi.mocked(readdir).mockImplementation((async (path: Parameters<typeof actualFs.readdir>[0], options?: unknown) => {
    if (String(path) === refusedFolder) {
      throw Object.assign(new Error(`${code}: simulated by the test, scandir '${refusedFolder}'`), {
        code,
        syscall: 'scandir',
        path: refusedFolder,
      });
    }
    const entries: unknown = await Reflect.apply(actualFs.readdir, undefined, [path, options]);
    return entries;
  }) as typeof readdir);
}

function denyRead(path: string, right: '(R)' | '(RD)'): () => void {
  const user = userInfo().username;
  execFileSync('icacls', [path, '/deny', `${user}:${right}`]);
  return () => {
    execFileSync('icacls', [path, '/remove:d', user]);
  };
}

function logText(): string {
  return logged.map((call) => call.map((value) => (typeof value === 'string' ? value : inspect(value))).join(' ')).join('\n');
}

function relativePaths(sources: readonly { readonly relativePath: string }[]): string[] {
  return sources.map((source) => source.relativePath);
}

beforeEach(async () => {
  root = await mkdtemp(join(tmpdir(), 'uigen-node-fs-unreadable-'));
  await writeTree(root, LIBRARY);
  fileSystem = new NodeProjectFileSystem();
  logged = [];
  const record = (...values: unknown[]): void => {
    logged.push(values);
  };
  vi.spyOn(console, 'error').mockImplementation(record);
  vi.spyOn(console, 'warn').mockImplementation(record);
  vi.spyOn(console, 'info').mockImplementation(record);
  vi.spyOn(console, 'log').mockImplementation(record);
  vi.spyOn(console, 'debug').mockImplementation(record);
});

afterEach(async () => {
  vi.restoreAllMocks();
  vi.mocked(readdir).mockImplementation(actualFs.readdir);
  await rm(root, { recursive: true, force: true });
});

describe('NodeProjectFileSystem.listSourceFiles with an entry deleted while reading', () => {
  // covers: FCT-0024
  it('skips a source file deleted between the listing of its folder and its reading', async () => {
    const victim = join(root, 'src', 'legacy', 'zz-badge.ts');
    deleteAfterListing(join(root, 'src', 'legacy'), victim);

    const sources = await fileSystem.listSourceFiles(root);

    expect(relativePaths(sources)).toEqual(READABLE_WITHOUT_LEGACY);
    expect(sources.find((source) => source.relativePath === 'src/index.ts')?.content).toBe(LIBRARY['src/index.ts']);
  });

  // covers: FCT-0024
  it('skips a sub-folder deleted between the listing of its parent and its own listing', async () => {
    deleteAfterListing(join(root, 'src'), join(root, 'src', 'legacy'));

    const sources = await fileSystem.listSourceFiles(root);

    expect(relativePaths(sources)).toEqual(READABLE_WITHOUT_LEGACY);
  });

  // covers: FCT-0024
  it('logs the skipped entry without any source content', async () => {
    const victim = join(root, 'src', 'legacy', 'zz-badge.ts');
    deleteAfterListing(join(root, 'src', 'legacy'), victim);

    await fileSystem.listSourceFiles(root);

    expect(logText()).toContain('zz-badge.ts');
    expect(logText()).not.toContain(SECRET_MARKER);
  });
});

describe('NodeProjectFileSystem.listSourceFiles with an entry it may not read', () => {
  // covers: FCT-0024
  it('skips a source file whose reading is denied', async () => {
    const before = await snapshot(root);
    const restore = denyRead(join(root, 'src', 'legacy', 'zz-badge.ts'), '(R)');
    let sources: readonly { readonly relativePath: string }[];
    try {
      sources = await fileSystem.listSourceFiles(root);
    } finally {
      restore();
    }

    expect(relativePaths(sources)).toEqual(READABLE_WITHOUT_LEGACY);
    expect(logText()).toContain('zz-badge.ts');
    expect(logText()).not.toContain(SECRET_MARKER);
    expect(await snapshot(root)).toEqual(before);
  });

  // covers: FCT-0024
  it('skips a sub-folder whose listing is denied', async () => {
    const before = await snapshot(root);
    const restore = denyRead(join(root, 'src', 'legacy'), '(RD)');
    let sources: readonly { readonly relativePath: string }[];
    try {
      sources = await fileSystem.listSourceFiles(root);
    } finally {
      restore();
    }

    expect(relativePaths(sources)).toEqual(READABLE_WITHOUT_LEGACY);
    expect(logText()).toContain('legacy');
    expect(logText()).not.toContain(SECRET_MARKER);
    expect(await snapshot(root)).toEqual(before);
  });

  // covers: FCT-0024
  it.each(['EACCES', 'EPERM', 'ENOENT'])('skips a sub-folder whose listing fails with %s', async (code) => {
    refuseListing(join(root, 'src', 'legacy'), code);

    const sources = await fileSystem.listSourceFiles(root);

    expect(relativePaths(sources)).toEqual(READABLE_WITHOUT_LEGACY);
    expect(logText()).toContain(code);
    expect(logText()).not.toContain(SECRET_MARKER);
  });

  it('still fails when the library folder itself cannot be listed', async () => {
    refuseListing(root, 'EACCES');

    await expect(fileSystem.listSourceFiles(root)).rejects.toMatchObject({ code: 'EACCES' });
  });
});
