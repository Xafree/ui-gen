import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test, type Page } from '@playwright/test';
import {
  closeStudio,
  launchStudio,
  makeTempFolder,
  removeFolder,
  writeFiles,
  writeRecents,
  type RecentEntry,
  type Studio,
} from './studio-app';

const KIT_HORIZON_FILES: Readonly<Record<string, string>> = {
  'package.json': `{
  "name": "@horizon/ui",
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

let root: string;
let userData: string;

test.beforeEach(async () => {
  root = await makeTempFolder('home-projects');
  userData = await makeTempFolder('home-user-data');
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

function recentSection(window: Page) {
  return window.getByRole('region', { name: 'Projets récents' });
}

async function recentPathsOnDisk(): Promise<string[]> {
  const value: unknown = JSON.parse(await readFile(join(userData, 'recent-projects.json'), 'utf8'));
  const projects: unknown = typeof value === 'object' && value !== null ? Reflect.get(value, 'projects') : null;
  if (!Array.isArray(projects)) return [];
  return projects.map((project: unknown) => {
    const path: unknown = typeof project === 'object' && project !== null ? Reflect.get(project, 'path') : null;
    return typeof path === 'string' ? path : '';
  });
}

// covers: FCT-0001
test('opens on the home screen with both start actions and an empty recent list on first launch', async () => {
  await withStudio(async ({ window }) => {
    await expect(window.getByRole('heading', { level: 1, name: 'UI-GEN' })).toBeVisible();
    await expect(window).toHaveTitle('Accueil, UI-GEN');
    await expect(window.getByText('Aucun projet ouvert', { exact: true })).toBeVisible();
    await expect(window.getByRole('heading', { level: 2, name: 'Projets récents' })).toBeVisible();
    await expect(window.getByText('Aucun projet récent', { exact: true })).toBeVisible();
    await expect(window.getByRole('button', { name: 'Créer un projet', exact: true })).toHaveCount(2);
    await expect(window.getByRole('button', { name: 'Ouvrir un projet', exact: true })).toHaveCount(2);
    await expect(window.getByRole('button', { name: 'Effacer la liste' })).toHaveCount(0);
    await expect(window.locator('ui-shell')).toHaveCount(0);
  });
});

// covers: FCT-0010
test('lists the ten most recent projects of the user data folder, newest first', async () => {
  const entry = (rank: string): RecentEntry => ({
    path: join(root, `p${rank}`),
    name: `Projet p${rank}`,
    openedAt: `2026-10-${rank}T08:00:00.000Z`,
  });
  const ranks = ['04', '11', '01', '07', '12', '02', '09', '05', '10', '03', '08', '06'];
  for (const rank of ranks) {
    await mkdir(join(root, `p${rank}`));
  }
  await writeRecents(userData, ranks.map(entry));

  await withStudio(async ({ window }) => {
    const links = recentSection(window).getByRole('link');
    await expect(links).toHaveCount(10);
    const expectedOrder = ['12', '11', '10', '09', '08', '07', '06', '05', '04', '03'];
    for (const [index, rank] of expectedOrder.entries()) {
      await expect(links.nth(index)).toContainText(`Projet p${rank}`);
      await expect(links.nth(index)).toContainText(join(root, `p${rank}`));
    }
    await expect(recentSection(window).getByText('Projet p02')).toHaveCount(0);
    await expect(recentSection(window).getByText('Projet p01')).toHaveCount(0);
    await expect(window.getByRole('button', { name: 'Créer un projet', exact: true })).toHaveCount(1);
    await expect(window.getByRole('button', { name: 'Effacer la liste' })).toBeVisible();
  });
});

// covers: FCT-0011
test('marks a moved project as « Introuvable » and removes it from the saved list without touching any folder', async () => {
  const kitHorizon = join(root, 'kit-horizon');
  const pivot = join(root, 'pivot-ui');
  await mkdir(kitHorizon);
  await writeRecents(userData, [
    { path: kitHorizon, name: 'Kit Horizon', openedAt: '2026-10-10T08:00:00.000Z' },
    { path: pivot, name: 'Prototype Pivot', openedAt: '2026-09-18T08:00:00.000Z' },
  ]);

  await withStudio(async ({ window }) => {
    const section = recentSection(window);
    await expect(section.getByText('Introuvable', { exact: true })).toBeVisible();
    await expect(section.getByText(pivot, { exact: true })).toBeVisible();
    await expect(section.getByRole('link')).toHaveCount(1);

    await window.getByRole('button', { name: 'Retirer de la liste : Prototype Pivot' }).click();

    await expect(window.getByText('Prototype Pivot retiré de la liste', { exact: true })).toBeVisible();
    await expect(window.getByText(`Le dossier ${pivot} n'a pas été modifié.`, { exact: true })).toBeVisible();
    await expect(section.getByText('Prototype Pivot', { exact: true })).toHaveCount(0);
    await expect(section.getByText('Introuvable', { exact: true })).toHaveCount(0);
    await expect(section.getByRole('link')).toBeFocused();
    await expect.poll(recentPathsOnDisk).toEqual([kitHorizon]);
  });
});

test('warns on the home screen when the saved recent list cannot be read', async () => {
  await writeFile(join(userData, 'recent-projects.json'), '{ "formatVersion": 1, "projects": [', 'utf8');

  await withStudio(async ({ window }) => {
    await expect(window.getByText('Projets récents illisibles', { exact: true })).toBeVisible();
    await expect(
      window.getByText("La liste n'a pas pu être lue. Aucun dossier n'a été modifié.", { exact: true }),
    ).toBeVisible();
    await expect(window.getByText('Aucun projet récent', { exact: true })).toBeVisible();
  });
});

test('opens a recent UI-GEN project in the studio shell from its link', async () => {
  const kitHorizon = join(root, 'kit-horizon');
  await writeFiles(kitHorizon, KIT_HORIZON_FILES);
  await writeRecents(userData, [{ path: kitHorizon, name: 'Kit Horizon', openedAt: '2026-10-10T08:00:00.000Z' }]);

  await withStudio(async ({ window }) => {
    const link = recentSection(window).getByRole('link');
    await expect(link).toContainText('Kit Horizon');

    await link.click();

    await expect(window.locator('ui-shell')).toHaveCount(1);
    await expect(window.locator('ui-home')).toHaveCount(0);
  });
});
