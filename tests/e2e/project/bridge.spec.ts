import { mkdir, readdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import {
  answerNextFolderDialog,
  closeStudio,
  launchStudio,
  makeTempFolder,
  removeFolder,
  writeFiles,
  writeRecents,
  type BridgeHost,
  type Studio,
} from './studio-app';

const ATLAS_FILES: Readonly<Record<string, string>> = {
  'package.json': `{
  "name": "@atlas-design/components",
  "version": "2.4.1",
  "description": "Atlas design system web components",
  "type": "module",
  "dependencies": {
    "lit": "^3.2.0",
    "@lit/context": "^1.1.3"
  },
  "devDependencies": {
    "typescript": "~5.6.3"
  }
}
`,
  'src/lib/button/at-button.ts': `import { LitElement, html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

/**
 * Primary action button.
 */
@customElement('at-button')
export class AtButton extends LitElement {
  @property({ reflect: true }) variant: 'primary' | 'ghost' = 'primary';

  render() {
    return html\`<button class=\${this.variant}><slot></slot></button>\`;
  }
}
`,
  'src/lib/card/at-card.ts': `import { LitElement, html } from 'lit';

export class AtCard extends LitElement {
  render() {
    return html\`<article><slot name="header"></slot><slot></slot></article>\`;
  }
}

customElements.define('at-card', AtCard);
`,
};

const ATLAS_SETTINGS = '{\n  "formatVersion": 1,\n  "tagPrefix": "at-",\n  "componentsDir": "src/lib"\n}\n';

let root: string;
let userData: string;

test.beforeEach(async () => {
  root = await makeTempFolder('projects');
  userData = await makeTempFolder('user-data');
});

test.afterEach(async () => {
  await removeFolder(root);
  await removeFolder(userData);
});

async function withStudio(run: (studio: Studio) => Promise<void>): Promise<void> {
  const studio = await launchStudio(userData);
  try {
    await run(studio);
  } finally {
    await closeStudio(studio.app);
  }
}

test('exposes only the project API, opens a 1440 x 900 window and keeps the system theme', async () => {
  await withStudio(async ({ app, window }) => {
    expect(
      await window.evaluate(() => {
        const api = (globalThis as unknown as BridgeHost).uiGen;
        return {
          top: Object.keys(api).sort(),
          recents: Object.keys(api.recents).sort(),
          project: Object.keys(api.project).sort(),
          ipcRenderer: 'ipcRenderer' in globalThis,
          require: typeof Reflect.get(globalThis, 'require'),
        };
      }),
    ).toEqual({
      top: ['platform', 'project', 'recents'],
      recents: ['clear', 'list', 'open', 'remove', 'restore'],
      project: ['adopt', 'chooseLocation', 'close', 'create', 'defaultLocation', 'openFolder'],
      ipcRenderer: false,
      require: 'undefined',
    });
    expect(await app.evaluate(({ app: electronApp }) => electronApp.getPath('userData'))).toBe(userData);
    expect(await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows().map((owner) => owner.getContentSize()))).toEqual([
      [1440, 900],
    ]);
    expect(await app.evaluate(({ nativeTheme }) => nativeTheme.themeSource)).toBe('system');
  });
});

// covers: FCT-0010
test('lists the ten most recent projects of the user data folder, newest first', async () => {
  await mkdir(join(root, 'p12'));
  await mkdir(join(root, 'p09'));
  const entry = (slug: string, day: string): { path: string; name: string; openedAt: string } => ({
    path: join(root, slug),
    name: `Projet ${slug}`,
    openedAt: `2026-10-${day}T08:00:00.000Z`,
  });
  await writeRecents(userData, [
    entry('p04', '04'),
    entry('p11', '11'),
    entry('p01', '01'),
    entry('p07', '07'),
    entry('p12', '12'),
    entry('p02', '02'),
    entry('p09', '09'),
    entry('p05', '05'),
    entry('p10', '10'),
    entry('p03', '03'),
    entry('p08', '08'),
    entry('p06', '06'),
  ]);

  await withStudio(async ({ window }) => {
    expect(await window.evaluate(() => (globalThis as unknown as BridgeHost).uiGen.recents.list())).toEqual({
      unreadable: false,
      projects: [
        { ...entry('p12', '12'), available: true },
        { ...entry('p11', '11'), available: false },
        { ...entry('p10', '10'), available: false },
        { ...entry('p09', '09'), available: true },
        { ...entry('p08', '08'), available: false },
        { ...entry('p07', '07'), available: false },
        { ...entry('p06', '06'), available: false },
        { ...entry('p05', '05'), available: false },
        { ...entry('p04', '04'), available: false },
        { ...entry('p03', '03'), available: false },
      ],
    });
  });
});

// covers: FCT-0010
test('reports an unreadable recent projects file as an empty unreadable list', async () => {
  await writeFile(join(userData, 'recent-projects.json'), '{ "formatVersion": 1, "projects": [ { "path": ', 'utf8');

  await withStudio(async ({ window }) => {
    expect(await window.evaluate(() => (globalThis as unknown as BridgeHost).uiGen.recents.list())).toEqual({
      projects: [],
      unreadable: true,
    });
  });
});

test('removes a recent project from the list', async () => {
  const kept = { path: join(root, 'kept'), name: 'Kept', openedAt: '2026-10-10T08:00:00.000Z' };
  const removed = { path: join(root, 'removed'), name: 'Removed', openedAt: '2026-10-09T08:00:00.000Z' };
  await mkdir(kept.path);
  await writeRecents(userData, [kept, removed]);

  await withStudio(async ({ window }) => {
    expect(
      await window.evaluate((path) => (globalThis as unknown as BridgeHost).uiGen.recents.remove(path), removed.path),
    ).toEqual([{ ...kept, available: true }]);
    expect(await window.evaluate(() => (globalThis as unknown as BridgeHost).uiGen.recents.list())).toEqual({
      unreadable: false,
      projects: [{ ...kept, available: true }],
    });
  });
});

test('proposes the Documents folder when there is no recent project', async () => {
  await withStudio(async ({ app, window }) => {
    const documents = await app.evaluate(({ app: electronApp }) => electronApp.getPath('documents'));

    expect(await window.evaluate(() => (globalThis as unknown as BridgeHost).uiGen.project.defaultLocation())).toBe(documents);
  });
});

test('proposes the parent folder of the latest available recent project', async () => {
  const atlas = join(root, 'atlas-design-system');
  await writeFiles(atlas, { ...ATLAS_FILES, 'uigen.json': ATLAS_SETTINGS });
  await writeRecents(userData, [{ path: atlas, name: 'Atlas maison', openedAt: '2026-10-09T16:05:12.000Z' }]);

  await withStudio(async ({ window }) => {
    expect(await window.evaluate(() => (globalThis as unknown as BridgeHost).uiGen.project.defaultLocation())).toBe(root);
  });
});

// covers: FCT-0006
test('opens a folder that holds the UI-GEN settings file and remembers it', async () => {
  const atlas = join(root, 'atlas-design-system');
  await writeFiles(atlas, { ...ATLAS_FILES, 'uigen.json': ATLAS_SETTINGS });

  await withStudio(async ({ app, window }) => {
    await answerNextFolderDialog(app, atlas);

    expect(await window.evaluate(() => (globalThis as unknown as BridgeHost).uiGen.project.openFolder())).toMatchObject({
      kind: 'opened',
      project: {
        path: atlas,
        packageName: '@atlas-design/components',
        version: '2.4.1',
        settings: { formatVersion: 1, tagPrefix: 'at-', componentsDir: 'src/lib' },
      },
    });
    expect(await window.evaluate(() => (globalThis as unknown as BridgeHost).uiGen.recents.list())).toMatchObject({
      unreadable: false,
      projects: [{ path: atlas, available: true }],
    });
    expect(
      await window.evaluate(() =>
        (globalThis as unknown as BridgeHost).uiGen.project.close().then((value: unknown) => value === undefined),
      ),
    ).toBe(true);
  });
});

// covers: FCT-0004
test('refuses to create a project over a folder that is not empty and writes nothing', async () => {
  const location = join(root, 'kits');
  const existing = join(location, 'kit-horizon');
  await writeFiles(existing, { 'notes.md': '# Kit Horizon\n\nIdées de composants.\n' });

  await withStudio(async ({ app, window }) => {
    await answerNextFolderDialog(app, location);
    expect(await window.evaluate(() => (globalThis as unknown as BridgeHost).uiGen.project.chooseLocation())).toEqual({
      canceled: false,
      path: location,
    });

    expect(
      await window.evaluate(
        (request) => (globalThis as unknown as BridgeHost).uiGen.project.create(request),
        { name: 'Kit Horizon', location, packageName: '@kit-horizon/ui', tagPrefix: 'kh-' },
      ),
    ).toEqual({ kind: 'folder-not-empty', path: existing, entryCount: 1 });
  });

  expect(await readdir(location)).toEqual(['kit-horizon']);
  expect(await readdir(existing)).toEqual(['notes.md']);
  expect(await readFile(join(existing, 'notes.md'), 'utf8')).toBe('# Kit Horizon\n\nIdées de composants.\n');
});

test('refuses to create a project in a location the studio never proposed', async () => {
  const location = join(root, 'elsewhere');
  await mkdir(location);

  await withStudio(async ({ window }) => {
    expect(
      await window.evaluate(
        (request) => (globalThis as unknown as BridgeHost).uiGen.project.create(request),
        { name: 'Kit Horizon', location, packageName: '@kit-horizon/ui', tagPrefix: 'kh-' },
      ),
    ).toEqual({ kind: 'invalid-request', errors: [{ field: 'location', code: 'not-granted' }] });
  });

  expect(await readdir(location)).toEqual([]);
});

test('adopts only the library that the last opened folder proposed', async () => {
  const atlas = join(root, 'atlas-design-system');
  const other = join(root, 'other-library');
  await writeFiles(atlas, ATLAS_FILES);
  await writeFiles(other, ATLAS_FILES);

  await withStudio(async ({ app, window }) => {
    await answerNextFolderDialog(app, atlas);
    expect(await window.evaluate(() => (globalThis as unknown as BridgeHost).uiGen.project.openFolder())).toMatchObject({
      kind: 'needs-adoption',
      proposal: { path: atlas },
    });

    expect(
      await window.evaluate(
        (request) => (globalThis as unknown as BridgeHost).uiGen.project.adopt(request),
        { path: other, tagPrefix: 'at-', componentsDir: 'src/lib' },
      ),
    ).toEqual({ kind: 'invalid-request', errors: [{ field: 'path', code: 'not-pending' }] });

    expect(
      await window.evaluate(
        (request) => (globalThis as unknown as BridgeHost).uiGen.project.adopt(request),
        { path: atlas, tagPrefix: 'at-', componentsDir: 'src/lib' },
      ),
    ).toMatchObject({ kind: 'opened', project: { path: atlas, settings: { tagPrefix: 'at-', componentsDir: 'src/lib' } } });
  });

  expect(await readFile(join(atlas, 'uigen.json'), 'utf8')).toBe(ATLAS_SETTINGS);
  expect(await readdir(other)).not.toContain('uigen.json');
});

test('never opens a folder that is not in the recent projects', async () => {
  const atlas = join(root, 'atlas-design-system');
  await writeFiles(atlas, { ...ATLAS_FILES, 'uigen.json': ATLAS_SETTINGS });

  await withStudio(async ({ window }) => {
    const result = await window.evaluate(
      (path) =>
        (globalThis as unknown as BridgeHost).uiGen.recents.open(path).catch(() => ({ kind: 'rejected' as const })),
      atlas,
    );

    expect(result).not.toMatchObject({ kind: 'opened' });
    expect(await window.evaluate(() => (globalThis as unknown as BridgeHost).uiGen.recents.list())).toEqual({
      unreadable: false,
      projects: [],
    });
  });
});
