import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import { makeTempFolder, removeFolder } from '../../project/studio-app';
import { canvasContent, openRecentProject, rememberOpenedComponent } from '../canvas/canvas-project';
import { FEATURE_LIST_FILE, PLAN_BADGE_FILE, endsWithFile, lastOpened, withStudio, writeKitHorizon } from '../editor-project';
import { componentOption, componentsTab, expectEditorOn, openedMark, writeTsconfig } from './editor-screen';

let root: string;
let userData: string;
let project: string;

test.beforeEach(async () => {
  root = await makeTempFolder('editor-components-tab');
  userData = await makeTempFolder('editor-components-tab-user-data');
  project = join(root, 'kit-horizon');
  await writeKitHorizon(project);
  await writeTsconfig(project);
  await rememberOpenedComponent(userData, project, 'hz-pricing-card');
});

test.afterEach(async () => {
  await removeFolder(root);
  await removeFolder(userData);
});

// covers: FCT-0028
test('opens a component clicked in the components tab and marks it « Ouvert »', async () => {
  await withStudio(userData, async ({ window }) => {
    await openRecentProject(window);
    await expectEditorOn(window, 'hz-pricing-card');
    await componentsTab(window).click();

    await componentOption(window, 'FeatureList').click();

    await expectEditorOn(window, 'hz-feature-list');
    await expect(componentOption(window, 'FeatureList')).toContainText('Ouvert');
    await expect(componentOption(window, 'PricingCard')).not.toContainText('Ouvert');
    await expect(openedMark(window)).toHaveCount(1);
    await expect(canvasContent(window).locator('hz-feature-list').first()).toBeAttached();
    await expect.poll(async () => endsWithFile(String(await lastOpened(window)), FEATURE_LIST_FILE)).toBe(true);
  });
});

// covers: FCT-0028
// covers: FCT-0026
test('opens a component with Enter in the components tab, and arrives on it at the next opening', async () => {
  await withStudio(userData, async ({ window }) => {
    await openRecentProject(window);
    await expectEditorOn(window, 'hz-pricing-card');
    await componentsTab(window).click();

    await componentOption(window, 'PlanBadge').focus();
    await window.keyboard.press('Enter');

    await expectEditorOn(window, 'hz-plan-badge');
    await expect(componentOption(window, 'PlanBadge')).toContainText('Ouvert');
    await expect(openedMark(window)).toHaveCount(1);
    await expect.poll(async () => endsWithFile(String(await lastOpened(window)), PLAN_BADGE_FILE)).toBe(true);
  });

  await withStudio(userData, async ({ window }) => {
    await openRecentProject(window);

    await expectEditorOn(window, 'hz-plan-badge');
  });
});
