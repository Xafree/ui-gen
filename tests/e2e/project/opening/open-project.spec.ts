import { readdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  answerNextFolderDialog,
  closeStudio,
  launchStudio,
  makeTempFolder,
  removeFolder,
  writeFiles,
  writeRecents,
  type Studio,
} from '../studio-app';

type FolderContent = Readonly<Record<string, string>>;

const KIT_HORIZON_FILES: FolderContent = {
  'package.json': `{
  "name": "@kit-horizon/ui",
  "version": "0.4.0",
  "type": "module",
  "dependencies": {
    "lit": "^3.3.0"
  }
}
`,
  'uigen.json': '{\n  "formatVersion": 1,\n  "tagPrefix": "hz-",\n  "componentsDir": "src/components"\n}\n',
  'src/components/hz-button.ts': `import { LitElement, html } from 'lit';
import { customElement } from 'lit/decorators.js';

@customElement('hz-button')
export class HzButton extends LitElement {
  render() {
    return html\`<button><slot></slot></button>\`;
  }
}
`,
};

const ATLAS_FILES: FolderContent = {
  'package.json': `{
  "name": "@atlas/components",
  "version": "2.3.1",
  "description": "Atlas design system",
  "type": "module",
  "main": "dist/index.js",
  "scripts": {
    "build": "tsc"
  },
  "dependencies": {
    "lit": "^3.2.1"
  },
  "devDependencies": {
    "typescript": "^5.6.0"
  }
}
`,
  'README.md': '# Atlas\n\nComposants web du design system Atlas.\n',
  'src/index.ts': `export * from './lib/atlas-button.js';
export * from './lib/atlas-card.js';
export * from './lib/forms/atlas-input.js';
`,
  'src/lib/atlas-button.ts': `import { LitElement, css, html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

// Primary action of the Atlas design system.
@customElement('atlas-button')
export class AtlasButton extends LitElement {
  static styles = css\`
    :host { display: inline-flex; }
  \`;

  @property({ reflect: true }) variant: 'primary' | 'ghost' = 'primary';

  render() {
    return html\`<button part="control"><slot></slot></button>\`;
  }
}
`,
  'src/lib/atlas-card.ts': `import { LitElement, html } from 'lit';

export class AtlasCard extends LitElement {
  render() {
    return html\`<article><slot name="title"></slot><slot></slot></article>\`;
  }
}

customElements.define('atlas-card', AtlasCard);
`,
  'src/lib/forms/atlas-input.ts': `import { LitElement, html } from 'lit';
import { customElement } from 'lit/decorators.js';

@customElement("atlas-input")
export class AtlasInput extends LitElement {
  render() {
    return html\`<input part="field" />\`;
  }
}
`,
  'src/lib/legacy/old-badge.ts': `import { LitElement, html } from 'lit';
import { customElement } from 'lit/decorators.js';

/* Kept for the 1.x consumers. */
@customElement('old-badge')
export class OldBadge extends LitElement {
  render() {
    return html\`<span><slot></slot></span>\`;
  }
}
`,
};

const SITE_VITRINE_FILES: FolderContent = {
  'index.html': '<!doctype html>\n<html lang="fr">\n<head><title>Site vitrine</title></head>\n<body><h1>Bienvenue</h1></body>\n</html>\n',
  'css/styles.css': 'body { font-family: system-ui; }\n',
};

const REACT_APP_FILES: FolderContent = {
  'package.json': `{
  "name": "vitrine-react",
  "version": "1.0.0",
  "dependencies": {
    "react": "^19.0.0"
  }
}
`,
  'src/App.tsx': 'export function App() {\n  return <h1>Vitrine</h1>;\n}\n',
};

const NOVA_FILES: FolderContent = {
  'package.json': `{
  "name": "@nova/ui",
  "version": "3.0.0",
  "dependencies": {
    "lit": "^3.3.0"
  }
}
`,
  'uigen.json': '{\n  "formatVersion": 2,\n  "tagPrefix": "nv-",\n  "componentsDir": "src/components"\n}\n',
  'src/components/nv-chip.ts': "import { LitElement } from 'lit';\n\ncustomElements.define('nv-chip', class extends LitElement {});\n",
};

const ORBITE_FILES: FolderContent = {
  'package.json': `{
  "name": "@orbite/ui",
  "version": "0.2.0",
  "dependencies": {
    "lit": "^3.3.0"
  }
}
`,
  'uigen.json': '{\n  "formatVersion": 1,\n  "tagprefix": "or-",\n  "componentsDir": "src/components"\n}\n',
};

const NOT_A_LIT_LIBRARY_TITLE = "Ce dossier n'est pas une bibliothèque Lit";
const NOT_A_LIT_LIBRARY_MESSAGE =
  "Aucun package.json ne déclare la dépendance lit. Rien n'a été modifié. Choisissez le dossier racine de la bibliothèque, ou créez un projet.";
const CANNOT_OPEN_TITLE = "Ce projet ne s'ouvre pas";

let root: string;
let userData: string;

test.beforeEach(async () => {
  root = await makeTempFolder('open-projects');
  userData = await makeTempFolder('open-user-data');
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

async function prepare(name: string, files: FolderContent): Promise<string> {
  const folder = join(root, name);
  await writeFiles(folder, files);
  return folder;
}

async function snapshot(folder: string): Promise<Record<string, string>> {
  const entries = await readdir(folder, { recursive: true, withFileTypes: true });
  const content: Record<string, string> = {};
  for (const entry of entries) {
    if (!entry.isFile()) continue;
    const path = join(entry.parentPath, entry.name);
    content[path.slice(folder.length + 1).replaceAll('\\', '/')] = await readFile(path, 'utf8');
  }
  return content;
}

async function readJson(path: string): Promise<unknown> {
  return JSON.parse(await readFile(path, 'utf8'));
}

function openButton(window: Page): Locator {
  return window.locator('#btn-open');
}

async function openFolder(studio: Studio, folder: string): Promise<void> {
  await answerNextFolderDialog(studio.app, folder);
  await openButton(studio.window).click();
}

function adoptDialog(window: Page, name = 'Atlas'): Locator {
  return window.getByRole('dialog', { name: `Ouvrir ${name} avec UI-GEN ?` });
}

function adoptField(window: Page, label: string): Locator {
  return adoptDialog(window).getByRole('textbox', { name: label, exact: true });
}

function adoptButton(window: Page, label: string): Locator {
  return adoptDialog(window).getByRole('button', { name: label, exact: true });
}

function toast(window: Page, title: string): Locator {
  return window.locator('.ui-toast').filter({ has: window.locator('.ui-toast__title', { hasText: title }) });
}

async function expectToast(window: Page, title: string, message: string): Promise<void> {
  const shown = toast(window, title).last();
  await expect(shown).toBeVisible();
  await expect(shown.locator('.ui-toast__title')).toHaveText(title);
  await expect(shown.locator('.ui-toast__text')).toHaveText(message);
}

async function expectEditorOn(window: Page, name: string): Promise<void> {
  await expect(window.locator('ui-shell')).toHaveCount(1);
  await expect(window.locator('ui-home')).toHaveCount(0);
  await expect(window).toHaveTitle('Éditeur, UI-GEN');
  await expect(window.getByRole('button', { name: /^Projet :/ })).toContainText(name);
}

async function expectStillHome(window: Page): Promise<void> {
  await expect(window.locator('ui-home')).toHaveCount(1);
  await expect(window.locator('ui-shell')).toHaveCount(0);
  await expect(window).toHaveTitle('Accueil, UI-GEN');
}

// covers: FCT-0006
test('opens a folder that holds uigen.json straight in the editor, without any question nor file change', async () => {
  const kitHorizon = await prepare('kit-horizon', KIT_HORIZON_FILES);
  const before = await snapshot(kitHorizon);

  await withStudio(async (studio) => {
    await openFolder(studio, kitHorizon);

    await expectEditorOn(studio.window, 'Kit Horizon');
    await expect(studio.window.getByRole('dialog')).toHaveCount(0);
  });

  expect(await snapshot(kitHorizon)).toEqual(before);
});

// covers: FCT-0007
// covers: FCT-0021
test('proposes the majority prefix and the components folder of a Lit library, and writes only uigen.json once confirmed', async () => {
  const atlas = await prepare('atlas-design-system', ATLAS_FILES);
  const before = await snapshot(atlas);

  await withStudio(async (studio) => {
    const { window } = studio;
    await openFolder(studio, atlas);

    await expect(adoptDialog(window)).toBeVisible();
    await expect(adoptDialog(window)).toContainText(atlas);
    await expect(adoptDialog(window)).toContainText('@atlas/components 2.3.1');
    await expect(adoptDialog(window)).toContainText('4 trouvés dans src/lib');
    await expect(adoptField(window, 'Préfixe des balises')).toHaveValue('atlas-');
    await expect(adoptField(window, 'Préfixe des balises')).toBeFocused();
    await expect(adoptField(window, 'Dossier des composants')).toHaveValue('src/lib');
    await expect(window.locator('#ad-json-value')).toHaveText('"atlas-"');
    await expect(window.locator('#ad-json-dir-value')).toHaveText('"src/lib"');
    expect(await snapshot(atlas)).toEqual(before);

    await adoptField(window, 'Préfixe des balises').fill('at-');
    await adoptField(window, 'Dossier des composants').fill('src');
    await expect(window.locator('#ad-json-value')).toHaveText('"at-"');
    await expect(window.locator('#ad-json-dir-value')).toHaveText('"src"');
    expect(await snapshot(atlas)).toEqual(before);

    await adoptButton(window, 'Ajouter uigen.json et ouvrir').click();

    await expectEditorOn(window, 'Atlas');
    await expectToast(window, 'Atlas ouvert avec UI-GEN', `uigen.json ajouté à ${atlas} avec le préfixe at-.`);
  });

  const after = await snapshot(atlas);
  expect(Object.keys(after).sort()).toEqual([...Object.keys(before), 'uigen.json'].sort());
  expect({ ...after, 'uigen.json': '' }).toEqual({ ...before, 'uigen.json': '' });
  expect(await readJson(join(atlas, 'uigen.json'))).toEqual({ formatVersion: 1, tagPrefix: 'at-', componentsDir: 'src' });

  await withStudio(async ({ window }) => {
    const recent = window.getByRole('region', { name: 'Projets récents' }).getByRole('link').filter({ hasText: 'Atlas' });
    await expect(recent).toContainText(atlas);

    await recent.click();

    await expectEditorOn(window, 'Atlas');
    await expect(window.getByRole('dialog')).toHaveCount(0);
  });
});

// covers: FCT-0007
// covers: FCT-0021
test('writes the proposed prefix and folder when they are accepted as they are, then opens the library directly next time', async () => {
  const atlas = await prepare('atlas-design-system', ATLAS_FILES);

  await withStudio(async (studio) => {
    await openFolder(studio, atlas);
    await expect(adoptField(studio.window, 'Préfixe des balises')).toHaveValue('atlas-');

    await adoptButton(studio.window, 'Ajouter uigen.json et ouvrir').click();

    await expectEditorOn(studio.window, 'Atlas');
  });

  expect(await readJson(join(atlas, 'uigen.json'))).toEqual({
    formatVersion: 1,
    tagPrefix: 'atlas-',
    componentsDir: 'src/lib',
  });
  const adopted = await snapshot(atlas);

  await withStudio(async (studio) => {
    await openFolder(studio, atlas);

    await expectEditorOn(studio.window, 'Atlas');
    await expect(studio.window.getByRole('dialog')).toHaveCount(0);
  });

  expect(await snapshot(atlas)).toEqual(adopted);
});

// covers: FCT-0008
test('cancelling the adoption leaves every file of the library as it was and stays on the home screen', async () => {
  const atlas = await prepare('atlas-design-system', ATLAS_FILES);
  const before = await snapshot(atlas);

  await withStudio(async (studio) => {
    const { window } = studio;
    await openFolder(studio, atlas);
    await expect(adoptDialog(window)).toBeVisible();
    await adoptField(window, 'Préfixe des balises').fill('at-');

    await adoptButton(window, 'Annuler').click();

    await expect(adoptDialog(window)).toHaveCount(0);
    await expectStillHome(window);
    await expect(openButton(window)).toBeFocused();
    await expect(openButton(window)).toHaveText('Ouvrir un projet');
    expect(await snapshot(atlas)).toEqual(before);

    await openFolder(studio, atlas);
    await expect(adoptDialog(window)).toBeVisible();
    await window.keyboard.press('Escape');

    await expect(adoptDialog(window)).toHaveCount(0);
    await expectStillHome(window);
  });

  expect(await snapshot(atlas)).toEqual(before);
});

// covers: FCT-0009
test('explains that a folder without package.json, or without the lit dependency, is not a Lit library, and changes nothing', async () => {
  const vitrine = await prepare('site-vitrine', SITE_VITRINE_FILES);
  const reactApp = await prepare('vitrine-react', REACT_APP_FILES);
  const vitrineBefore = await snapshot(vitrine);
  const reactBefore = await snapshot(reactApp);

  await withStudio(async (studio) => {
    const { window } = studio;
    await openFolder(studio, vitrine);

    await expectToast(window, NOT_A_LIT_LIBRARY_TITLE, NOT_A_LIT_LIBRARY_MESSAGE);
    await expectStillHome(window);
    await expect(window.getByRole('dialog')).toHaveCount(0);
    await expect(openButton(window)).toBeFocused();
    await expect(openButton(window)).toHaveText('Ouvrir un projet');

    await openFolder(studio, reactApp);

    await expect(toast(window, NOT_A_LIT_LIBRARY_TITLE)).toHaveCount(2);
    await expectToast(window, NOT_A_LIT_LIBRARY_TITLE, NOT_A_LIT_LIBRARY_MESSAGE);
    await expectStillHome(window);
  });

  expect(await snapshot(vitrine)).toEqual(vitrineBefore);
  expect(await snapshot(reactApp)).toEqual(reactBefore);
});

test('refuses a uigen.json written by a newer UI-GEN, or with a misspelt field, and changes nothing', async () => {
  const nova = await prepare('nova-ui', NOVA_FILES);
  const orbite = await prepare('orbite-ui', ORBITE_FILES);
  const novaBefore = await snapshot(nova);
  const orbiteBefore = await snapshot(orbite);

  await withStudio(async (studio) => {
    const { window } = studio;
    await openFolder(studio, nova);

    await expectToast(
      window,
      CANNOT_OPEN_TITLE,
      "uigen.json a été créé par une version plus récente d'UI-GEN. Rien n'a été modifié. Mettez à jour UI-GEN pour ouvrir ce projet.",
    );
    await expectStillHome(window);
    await expect(openButton(window)).toBeFocused();

    await openFolder(studio, orbite);

    await expectToast(
      window,
      CANNOT_OPEN_TITLE,
      "Champ inconnu « tagprefix » dans uigen.json. Champ obligatoire « tagPrefix » absent de uigen.json. Rien n'a été modifié. Corrigez uigen.json, puis ouvrez de nouveau le projet.",
    );
    await expectStillHome(window);
  });

  expect(await snapshot(nova)).toEqual(novaBefore);
  expect(await snapshot(orbite)).toEqual(orbiteBefore);
});

// covers: FCT-0007
test('asks to adopt a recent library whose uigen.json is gone, instead of opening nothing', async () => {
  const atlas = await prepare('atlas-design-system', ATLAS_FILES);
  const before = await snapshot(atlas);
  await writeRecents(userData, [{ path: atlas, name: 'Atlas', openedAt: '2026-10-10T08:00:00.000Z' }]);

  await withStudio(async ({ window }) => {
    await window.getByRole('region', { name: 'Projets récents' }).getByRole('link').filter({ hasText: 'Atlas' }).click();

    await expect(adoptDialog(window)).toBeVisible();
    await expect(adoptField(window, 'Préfixe des balises')).toHaveValue('atlas-');
    await expect(adoptField(window, 'Dossier des composants')).toHaveValue('src/lib');

    await adoptButton(window, 'Annuler').click();

    await expectStillHome(window);
  });

  expect(await snapshot(atlas)).toEqual(before);
});

// covers: FCT-0011
test('warns that a recent project was moved since the list was shown, and marks it « Introuvable »', async () => {
  const kitHorizon = await prepare('kit-horizon', KIT_HORIZON_FILES);
  await writeRecents(userData, [{ path: kitHorizon, name: 'Kit Horizon', openedAt: '2026-10-10T08:00:00.000Z' }]);

  await withStudio(async ({ window }) => {
    const section = window.getByRole('region', { name: 'Projets récents' });
    await expect(section.getByRole('link')).toContainText('Kit Horizon');
    await removeFolder(kitHorizon);

    await section.getByRole('link').click();

    await expectToast(window, 'Projet introuvable', `Le dossier ${kitHorizon} a été déplacé ou supprimé.`);
    await expect(section.getByText('Introuvable', { exact: true })).toBeVisible();
    await expect(section.getByRole('link')).toHaveCount(0);
    await expectStillHome(window);
  });
});
