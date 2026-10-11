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
  type Studio,
} from './studio-app';

let root: string;
let userData: string;

test.beforeEach(async () => {
  root = await makeTempFolder('create-projects');
  userData = await makeTempFolder('create-user-data');
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

function createDialog(window: Page): Locator {
  return window.getByRole('dialog', { name: 'Créer un projet' });
}

function field(window: Page, label: string): Locator {
  return createDialog(window).getByRole('textbox', { name: label, exact: true });
}

function button(window: Page, label: string): Locator {
  return createDialog(window).getByRole('button', { name: label, exact: true });
}

async function readJson(path: string): Promise<unknown> {
  return JSON.parse(await readFile(path, 'utf8'));
}

async function openCreateDialogIn(studio: Studio, location: string): Promise<void> {
  const { app, window } = studio;
  await window.getByRole('button', { name: 'Créer un projet', exact: true }).first().click();
  await expect(createDialog(window)).toBeVisible();
  await expect(field(window, 'Nom du projet')).toBeFocused();
  await answerNextFolderDialog(app, location);
  await button(window, 'Parcourir…').click();
  await expect(field(window, 'Emplacement')).toHaveValue(location);
  await expect(button(window, 'Parcourir…')).toBeFocused();
}

// covers: FCT-0002
// covers: FCT-0003
test('creates « Kit Horizon » in the chosen location with the proposed settings, then opens it in the editor', async () => {
  const project = join(root, 'kit-horizon');

  await withStudio(async (studio) => {
    const { window } = studio;
    await openCreateDialogIn(studio, root);

    await field(window, 'Nom du projet').fill('Kit Horizon');

    await expect(field(window, 'Nom du package npm')).toHaveValue('@kit-horizon/ui');
    await expect(field(window, 'Préfixe des balises')).toHaveValue('kh-');
    await expect(createDialog(window).getByText(project, { exact: true })).toBeVisible();
    await expect(createDialog(window).getByText('<kh-button>', { exact: true })).toBeVisible();

    await button(window, 'Créer le projet').click();

    await expect(window.locator('ui-shell')).toHaveCount(1);
    await expect(window.getByRole('button', { name: /^Projet :/ })).toContainText('Kit Horizon');
    await expect(window.getByRole('button', { name: /^Projet :/ })).toContainText('@kit-horizon/ui');
    await expect(window).toHaveTitle('Éditeur, UI-GEN');
    const notice = window.locator('.ui-toast').filter({ hasText: 'Projet créé' });
    await expect(notice).toContainText(`Kit Horizon est prêt dans ${project}.`);
  });

  expect(await readdir(root)).toEqual(['kit-horizon']);
  expect(await readJson(join(project, 'package.json'))).toMatchObject({
    name: '@kit-horizon/ui',
    dependencies: { lit: expect.any(String) as unknown },
  });
  expect(await readJson(join(project, 'uigen.json'))).toEqual({
    formatVersion: 1,
    tagPrefix: 'kh-',
    componentsDir: 'src/components',
  });
  expect(await readdir(join(project, 'src', 'components'))).toEqual([]);

  await withStudio(async ({ window }) => {
    const recents = window.getByRole('region', { name: 'Projets récents' }).getByRole('link');
    await expect(recents).toHaveCount(1);
    await expect(recents).toContainText('Kit Horizon');
    await expect(recents).toContainText(project);
  });
});

// covers: FCT-0002
test('creates the project with the package name and the prefix the user changed', async () => {
  const project = join(root, 'horizon');

  await withStudio(async (studio) => {
    const { window } = studio;
    await openCreateDialogIn(studio, root);
    await field(window, 'Nom du projet').fill('Kit Horizon');

    await field(window, 'Nom du package npm').fill('@horizon/ui');
    await field(window, 'Préfixe des balises').fill('hz-');

    await expect(createDialog(window).getByText(project, { exact: true })).toBeVisible();
    await expect(createDialog(window).getByText('<hz-button>', { exact: true })).toBeVisible();
    await button(window, 'Créer le projet').click();
    await expect(window.locator('ui-shell')).toHaveCount(1);
  });

  expect(await readJson(join(project, 'package.json'))).toMatchObject({ name: '@horizon/ui' });
  expect(await readJson(join(project, 'uigen.json'))).toMatchObject({ tagPrefix: 'hz-' });
});

// covers: FCT-0004
test('refuses to create the project over a folder that is not empty, names it and writes nothing', async () => {
  const existing = join(root, 'kit-horizon');
  await writeFiles(existing, {
    'notes.md': '# Kit Horizon\n\nIdées de composants.\n',
    'brouillon.txt': 'bouton, carte, onglets\n',
  });

  await withStudio(async (studio) => {
    const { window } = studio;
    await openCreateDialogIn(studio, root);
    await field(window, 'Nom du projet').fill('Kit Horizon');

    await button(window, 'Créer le projet').click();

    await expect(createDialog(window).locator('#cr-loc-err')).toHaveText(
      `Le dossier ${existing} existe déjà et contient 2 fichiers. Rien n'a été créé. Changez le nom du projet ou l'emplacement.`,
    );
    await expect(field(window, 'Nom du projet')).toBeFocused();
    await expect(window.locator('ui-shell')).toHaveCount(0);
    await expect(window.locator('.ui-toast').filter({ hasText: 'Projet créé' })).toHaveCount(0);
  });

  expect(await readdir(root)).toEqual(['kit-horizon']);
  expect((await readdir(existing)).toSorted()).toEqual(['brouillon.txt', 'notes.md']);
  expect(await readFile(join(existing, 'notes.md'), 'utf8')).toBe('# Kit Horizon\n\nIdées de composants.\n');
});

// covers: FCT-0005
test('explains each broken prefix rule in the window and keeps the creation impossible', async () => {
  const rules: readonly (readonly [string, string])[] = [
    ['KH-', "Le préfixe s'écrit en minuscules : kh- au lieu de KH-."],
    ['1kh-', 'Le préfixe commence par une lettre, par exemple kh-.'],
    ['k_h-', 'Utilisez seulement des lettres minuscules, des chiffres et des tirets.'],
    ['kh', 'Ajoutez un tiret à la fin : kh-.'],
    ['k--h-', 'Retirez le double tiret : un seul tiret sépare le préfixe du nom.'],
  ];

  await withStudio(async (studio) => {
    const { window } = studio;
    await openCreateDialogIn(studio, root);
    await field(window, 'Nom du projet').fill('Kit Horizon');

    for (const [prefix, message] of rules) {
      await field(window, 'Préfixe des balises').fill(prefix);
      await expect(createDialog(window).getByText(message, { exact: true })).toBeVisible();
      await expect(button(window, 'Créer le projet')).toBeDisabled();
      await expect(createDialog(window).getByText('Corrigez le préfixe', { exact: true })).toBeVisible();
    }

    await field(window, 'Préfixe des balises').press('Enter');
    await expect(createDialog(window)).toBeVisible();
    await expect(window.locator('ui-shell')).toHaveCount(0);
  });

  expect(await readdir(root)).toEqual([]);
});
