import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import { makeTempFolder, removeFolder } from '../../project/studio-app';
import { canvasContent, openRecentProject, rememberOpenedComponent } from '../canvas/canvas-project';
import { lastOpened, withStudio, writeEmptyLibrary, writeKitHorizon } from '../editor-project';
import {
  componentOption,
  componentsTab,
  expectEditorOn,
  openedMark,
  rememberRecentProject,
  writeKitHorizonSupport,
  writeTsconfig,
} from './editor-screen';

let root: string;
let userData: string;
let project: string;

test.beforeEach(async () => {
  root = await makeTempFolder('editor-arrival');
  userData = await makeTempFolder('editor-arrival-user-data');
  project = join(root, 'kit-horizon');
});

test.afterEach(async () => {
  await removeFolder(root);
  await removeFolder(userData);
});

// covers: FCT-0026
test('arrives on the component last opened in the project', async () => {
  await writeKitHorizon(project);
  await writeTsconfig(project);
  await writeKitHorizonSupport(project);
  await rememberOpenedComponent(userData, project, 'hz-pricing-card');

  await withStudio(userData, async ({ window }) => {
    await openRecentProject(window);

    await expectEditorOn(window, 'hz-pricing-card');
    await expect(canvasContent(window).locator('hz-pricing-card').first()).toBeAttached();
    await expect(window.getByText('Disponible dans un prochain lot', { exact: true })).toBeHidden();
    await componentsTab(window).click();
    await expect(openedMark(window)).toHaveCount(1);
    await expect(componentOption(window, 'PricingCard')).toContainText('Ouvert');
  });
});

// covers: FCT-0026
test('arrives on the first component by tag order when none was opened in the project', async () => {
  await writeKitHorizon(project);
  await writeTsconfig(project);
  await writeKitHorizonSupport(project);
  await rememberRecentProject(userData, project);

  await withStudio(userData, async ({ window }) => {
    await openRecentProject(window);

    await expectEditorOn(window, 'hz-feature-list');
    await componentsTab(window).click();
    await expect(componentOption(window, 'FeatureList')).toContainText('Ouvert');
    await expect(openedMark(window)).toHaveCount(1);
  });
});

// covers: FCT-0027
test('shows the empty library state with « Importer un composant » when the components folder has no component', async () => {
  await writeEmptyLibrary(project);
  await rememberRecentProject(userData, project);

  await withStudio(userData, async ({ window }) => {
    await openRecentProject(window);

    await expect(window.locator('ui-editor')).toBeVisible();
    await expect(window.locator('#layers-empty')).toBeVisible();
    await expect(window.locator('#layers-empty')).toContainText('Aucun composant dans cette bibliothèque');
    await expect(window.locator('#canvas-empty')).toBeVisible();
    await expect(window.locator('#canvas-empty')).toContainText('Aucun composant dans cette bibliothèque');
    await expect(window.locator('#canvas-empty').getByRole('link', { name: 'Importer un composant' })).toBeVisible();
    await expect(window.locator('ui-canvas-frame iframe')).toHaveCount(0);
    await expect(window.getByText('Disponible dans un prochain lot', { exact: true })).toBeHidden();
    expect(await lastOpened(window)).toBeNull();

    await componentsTab(window).click();

    await expect(window.locator('#comp-count')).toHaveText('0');
    await expect(window.locator('#comp-empty-title')).toHaveText('Aucun composant dans cette bibliothèque');
    await expect(window.getByRole('button', { name: 'Effacer la recherche' })).toBeHidden();
  });
});
