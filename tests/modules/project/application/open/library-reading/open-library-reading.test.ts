import { execFileSync } from 'node:child_process';
import { mkdir, mkdtemp, readdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir, userInfo } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { NodeProjectFileSystem } from '../../../../../../src/modules/project/adapters/node-fs/node-project-file-system';
import { openProjectFolder } from '../../../../../../src/modules/project/application/open/open-project-folder';
import type {
  AdoptionProposal,
  OpenProjectOutcome,
  ProjectDependencies,
} from '../../../../../../src/modules/project/application/open/project-outcomes';
import { InMemoryRecentProjectsStore } from '../../doubles/in-memory-recent-projects-store';

const NOW = new Date('2026-10-11T09:30:00.000Z');

const PACKAGE_JSON = `{
  "name": "@kit-horizon/ui",
  "version": "0.3.0",
  "description": "Kit Horizon — composants de l'équipe produit",
  "type": "module",
  "dependencies": {
    "lit": "^3.3.0"
  },
  "devDependencies": {
    "typescript": "~5.6.3"
  }
}
`;

const SETTINGS = `{
  "formatVersion": 1,
  "tagPrefix": "kh-",
  "componentsDir": "src/components"
}
`;

function componentSource(tag: string, className: string): string {
  return `import { LitElement, html } from 'lit';
import { customElement } from 'lit/decorators.js';

// Retouché à la main : garder la slot par défaut.
@customElement('${tag}')
export class ${className} extends LitElement {
  render() {
    return html\`<div part="${tag}"><slot></slot></div>\`;
  }
}
`;
}

const READABLE: Readonly<Record<string, string>> = {
  'package.json': PACKAGE_JSON,
  'src/index.ts': "export * from './components/kh-button.js';\nexport * from './components/kh-card.js';\n",
  'src/components/kh-button.ts': componentSource('kh-button', 'KhButton'),
  'src/components/kh-card.ts': componentSource('kh-card', 'KhCard'),
};

const LEGACY_FOLDER: Readonly<Record<string, string>> = {
  'src/legacy/zz-alert.ts': componentSource('zz-alert', 'ZzAlert'),
  'src/legacy/zz-badge.ts': componentSource('zz-badge', 'ZzBadge'),
  'src/legacy/zz-chip.ts': componentSource('zz-chip', 'ZzChip'),
};

let root: string;

async function writeTree(base: string, files: Readonly<Record<string, string>>): Promise<void> {
  const folders = new Set(Object.keys(files).map((relativePath) => join(base, ...relativePath.split('/'), '..')));
  for (const folder of folders) await mkdir(folder, { recursive: true });
  await Promise.all(
    Object.entries(files).map(([relativePath, content]) => writeFile(join(base, ...relativePath.split('/')), content, 'utf8')),
  );
}

async function snapshot(base: string, prefix = ''): Promise<Record<string, string>> {
  const files: Record<string, string> = {};
  for (const entry of await readdir(join(base, prefix), { withFileTypes: true })) {
    const relativePath = prefix === '' ? entry.name : `${prefix}/${entry.name}`;
    if (entry.isDirectory()) Object.assign(files, await snapshot(base, relativePath));
    else files[relativePath] = await readFile(join(base, ...relativePath.split('/')), 'utf8');
  }
  return files;
}

function dependencies(): ProjectDependencies {
  return { fileSystem: new NodeProjectFileSystem(), recentProjects: new InMemoryRecentProjectsStore(), clock: () => NOW };
}

async function openWhileDenied(path: string, denied: string, right: '(R)' | '(RD)'): Promise<OpenProjectOutcome> {
  const user = userInfo().username;
  execFileSync('icacls', [denied, '/deny', `${user}:${right}`]);
  try {
    return await openProjectFolder(dependencies(), path);
  } finally {
    execFileSync('icacls', [denied, '/remove:d', user]);
  }
}

function proposalOf(outcome: OpenProjectOutcome): AdoptionProposal {
  if (outcome.kind !== 'needs-adoption') throw new Error(`expected needs-adoption, got ${outcome.kind}`);
  return outcome.proposal;
}

function withoutPath(proposal: AdoptionProposal): AdoptionProposal {
  return { ...proposal, path: '' };
}

beforeEach(async () => {
  root = await mkdtemp(join(tmpdir(), 'uigen-open-reading-'));
  for (const method of ['error', 'warn', 'info', 'log', 'debug'] as const) {
    vi.spyOn(console, method).mockImplementation(() => undefined);
  }
});

afterEach(async () => {
  vi.restoreAllMocks();
  await rm(root, { recursive: true, force: true });
});

describe('openProjectFolder on a library with an unreadable entry', () => {
  // covers: FCT-0024
  it('proposes the adoption as if a denied sub-folder were absent and modifies nothing', async () => {
    const library = join(root, 'denied', 'kit-horizon');
    const reference = join(root, 'reference', 'kit-horizon');
    await writeTree(library, { ...READABLE, ...LEGACY_FOLDER });
    await writeTree(reference, READABLE);
    const before = await snapshot(library);

    const outcome = await openWhileDenied(library, join(library, 'src', 'legacy'), '(RD)');

    const proposal = proposalOf(outcome);
    expect(proposal.path).toBe(library);
    expect(withoutPath(proposal)).toEqual(withoutPath(proposalOf(await openProjectFolder(dependencies(), reference))));
    expect(proposal.tagPrefix).toBe('kh-');
    expect(proposal.componentCount).toBe(2);
    expect(proposal.componentsDir).toBe('src/components');
    expect(await snapshot(library)).toEqual(before);
  });

  // covers: FCT-0024
  it('proposes the adoption as if a denied source file were absent and modifies nothing', async () => {
    const library = join(root, 'denied', 'kit-horizon');
    const reference = join(root, 'reference', 'kit-horizon');
    await writeTree(library, { ...READABLE, 'src/components/zz-alert.ts': componentSource('zz-alert', 'ZzAlert') });
    await writeTree(reference, READABLE);
    const before = await snapshot(library);

    const outcome = await openWhileDenied(library, join(library, 'src', 'components', 'zz-alert.ts'), '(R)');

    const proposal = proposalOf(outcome);
    expect(proposal.path).toBe(library);
    expect(withoutPath(proposal)).toEqual(withoutPath(proposalOf(await openProjectFolder(dependencies(), reference))));
    expect(proposal.packageName).toBe('@kit-horizon/ui');
    expect(proposal.version).toBe('0.3.0');
    expect(proposal.componentCount).toBe(2);
    expect(proposal.tagPrefix).toBe('kh-');
    expect(await snapshot(library)).toEqual(before);
  });

  // covers: FCT-0024
  it('opens a library with settings despite a denied sub-folder and modifies nothing', async () => {
    const library = join(root, 'denied', 'kit-horizon');
    await writeTree(library, { ...READABLE, ...LEGACY_FOLDER, 'uigen.json': SETTINGS });
    const before = await snapshot(library);

    const outcome = await openWhileDenied(library, join(library, 'src', 'legacy'), '(RD)');

    expect(outcome).toMatchObject({
      kind: 'opened',
      project: {
        path: library,
        packageName: '@kit-horizon/ui',
        version: '0.3.0',
        settings: { formatVersion: 1, tagPrefix: 'kh-', componentsDir: 'src/components' },
      },
    });
    expect(await snapshot(library)).toEqual(before);
  });
});

describe('openProjectFolder on a library of more than 2 000 sources', () => {
  // covers: FCT-0025
  it('detects the prefix and the components folder from the first 2 000 sources only', async () => {
    const library = join(root, 'large', 'kit-horizon');
    const helpers: Record<string, string> = {};
    for (let index = 0; index < 1997; index += 1) {
      helpers[`src/helpers/helper-${String(index).padStart(4, '0')}.ts`] = `export const helper${String(index)} = ${String(index)};\n`;
    }
    await writeTree(library, {
      'package.json': PACKAGE_JSON,
      ...helpers,
      'src/components/kh-button.ts': componentSource('kh-button', 'KhButton'),
      'src/components/kh-card.ts': componentSource('kh-card', 'KhCard'),
      'src/components/kh-dialog.ts': componentSource('kh-dialog', 'KhDialog'),
      'src/zz/zz-alert.ts': componentSource('zz-alert', 'ZzAlert'),
      'src/zz/zz-badge.ts': componentSource('zz-badge', 'ZzBadge'),
      'src/zz/zz-chip.ts': componentSource('zz-chip', 'ZzChip'),
      'src/zz/zz-menu.ts': componentSource('zz-menu', 'ZzMenu'),
      'src/zz/zz-toast.ts': componentSource('zz-toast', 'ZzToast'),
    });

    const proposal = proposalOf(await openProjectFolder(dependencies(), library));

    expect(proposal.tagPrefix).toBe('kh-');
    expect(proposal.componentCount).toBe(3);
    expect(proposal.componentsDir).toBe('src/components');
    expect(proposal.packageName).toBe('@kit-horizon/ui');
  }, 60_000);
});
