import { mkdir, readdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test, type Page } from '@playwright/test';
import {
  closeStudio,
  launchStudio,
  makeTempFolder,
  removeFolder,
  writeRecents,
  type BridgeHost,
  type RecentEntry,
  type Studio,
} from '../studio-app';

let root: string;
let userData: string;

test.beforeEach(async () => {
  root = await makeTempFolder('recent-list-projects');
  userData = await makeTempFolder('recent-list-user-data');
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

function toastNamed(window: Page, title: string) {
  return window.locator('.ui-toast').filter({ hasText: title });
}

function readText(value: unknown, key: string): string {
  const field: unknown = typeof value === 'object' && value !== null ? Reflect.get(value, key) : null;
  return typeof field === 'string' ? field : '';
}

async function recentsOnDisk(): Promise<RecentEntry[]> {
  const value: unknown = JSON.parse(await readFile(join(userData, 'recent-projects.json'), 'utf8'));
  const projects: unknown = typeof value === 'object' && value !== null ? Reflect.get(value, 'projects') : null;
  if (!Array.isArray(projects)) return [];
  return projects.map((project: unknown) => ({
    path: readText(project, 'path'),
    name: readText(project, 'name'),
    openedAt: readText(project, 'openedAt'),
  }));
}

async function seedProjects(): Promise<{ horizon: RecentEntry; pivot: RecentEntry; lumen: RecentEntry }> {
  const horizon = { path: join(root, 'kit-horizon'), name: 'Kit Horizon', openedAt: '2026-10-10T08:00:00.000Z' };
  const pivot = { path: join(root, 'pivot-ui'), name: 'Prototype Pivot', openedAt: '2026-10-01T08:00:00.000Z' };
  const lumen = { path: join(root, 'lumen-ui'), name: 'Lumen UI', openedAt: '2026-09-20T08:00:00.000Z' };
  await mkdir(horizon.path);
  await mkdir(lumen.path);
  await writeFile(join(horizon.path, 'package.json'), '{ "name": "@horizon/ui" }\n', 'utf8');
  await writeRecents(userData, [lumen, horizon, pivot]);
  return { horizon, pivot, lumen };
}

// covers: FCT-0022
test('clears the recent list without touching any folder, and « Annuler » brings it back', async () => {
  const { horizon, pivot, lumen } = await seedProjects();
  const foldersBefore = (await readdir(root)).toSorted();

  await withStudio(async ({ window }) => {
    const section = recentSection(window);
    await expect(section.getByRole('link')).toHaveCount(2);

    await window.getByRole('button', { name: 'Effacer la liste' }).click();

    await expect(window.getByText('Aucun projet récent', { exact: true })).toBeVisible();
    await expect(window.getByRole('button', { name: 'Effacer la liste' })).toHaveCount(0);
    const toast = toastNamed(window, 'Liste des projets récents effacée');
    await expect(toast).toBeVisible();
    await expect(toast.getByText("Aucun dossier n'a été modifié.", { exact: true })).toBeVisible();
    await expect(window.locator('#empty-create')).toBeFocused();
    await expect.poll(recentsOnDisk).toEqual([]);
    expect((await readdir(root)).toSorted()).toEqual(foldersBefore);
    expect(await readFile(join(horizon.path, 'package.json'), 'utf8')).toBe('{ "name": "@horizon/ui" }\n');

    await toast.getByRole('button', { name: 'Annuler' }).click();

    await expect(section.getByRole('link')).toHaveCount(2);
    await expect(section.getByRole('link').nth(0)).toContainText('Kit Horizon');
    await expect(section.getByRole('link').nth(1)).toContainText('Lumen UI');
    await expect(window.getByRole('button', { name: 'Retirer de la liste : Prototype Pivot' })).toBeVisible();
    await expect(section.getByRole('link').nth(0)).toBeFocused();
    await expect.poll(recentsOnDisk).toEqual([horizon, pivot, lumen]);
  });
});

// covers: FCT-0023
test('puts a removed project back at its place with « Annuler »', async () => {
  const { horizon, pivot, lumen } = await seedProjects();

  await withStudio(async ({ window }) => {
    const removeButton = window.getByRole('button', { name: 'Retirer de la liste : Prototype Pivot' });
    await removeButton.click();
    await expect(removeButton).toHaveCount(0);
    await expect.poll(recentsOnDisk).toEqual([horizon, lumen]);

    await toastNamed(window, 'Prototype Pivot retiré de la liste').getByRole('button', { name: 'Annuler' }).click();

    await expect(removeButton).toBeFocused();
    await expect.poll(recentsOnDisk).toEqual([horizon, pivot, lumen]);
    const rows = recentSection(window).getByRole('listitem');
    await expect(rows).toHaveCount(3);
    await expect(rows.nth(1)).toContainText('Prototype Pivot');
  });
});

// covers: FCT-0023
test('refuses to restore through the bridge a project that was never removed', async () => {
  const { horizon, lumen } = await seedProjects();
  await writeRecents(userData, [horizon, lumen]);

  await withStudio(async ({ window }) => {
    await expect(recentSection(window).getByRole('link')).toHaveCount(2);
    const intruder = { path: 'C:\\Windows\\System32', name: 'System32', openedAt: '2026-10-11T08:00:00.000Z' };

    const failure = await window.evaluate(
      (project) =>
        (globalThis as unknown as BridgeHost).uiGen.recents.restore([project]).then(
          () => 'restored',
          (error: unknown) => String(error),
        ),
      intruder,
    );

    expect(failure).toContain('IPC message rejected (invalid-payload)');
    expect(await recentsOnDisk()).toEqual([horizon, lumen]);
  });
});
