import { join } from 'node:path';
import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  closeStudio,
  launchStudio,
  makeTempFolder,
  removeFolder,
  writeFiles,
  writeRecents,
  type Studio,
} from '../project/studio-app';

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

// Bouton principal du kit, écrit à la main.
@customElement('hz-button')
export class HzButton extends LitElement {
  render() {
    return html\`<button part="control"><slot></slot></button>\`;
  }
}
`,
};

let root: string;
let userData: string;
let kitHorizon: string;

test.beforeEach(async () => {
  root = await makeTempFolder('shell-projects');
  userData = await makeTempFolder('shell-user-data');
  kitHorizon = join(root, 'kit-horizon');
  await writeFiles(kitHorizon, KIT_HORIZON_FILES);
  await writeRecents(userData, [{ path: kitHorizon, name: 'Kit Horizon', openedAt: '2026-10-10T08:00:00.000Z' }]);
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

function recentSection(window: Page): Locator {
  return window.getByRole('region', { name: 'Projets récents' });
}

function projectButton(window: Page): Locator {
  return window.getByRole('button', { name: /^Projet :/ });
}

function studioNav(window: Page): Locator {
  return window.getByRole('navigation', { name: 'Écrans du studio' });
}

function navLink(window: Page, label: string): Locator {
  return studioNav(window).getByRole('link', { name: label, exact: true });
}

function toasts(window: Page): Locator {
  return window.locator('.ui-toast');
}

async function openKitHorizon(window: Page): Promise<void> {
  const link = recentSection(window).getByRole('link');
  await expect(link).toContainText('Kit Horizon');
  await link.click();
  await expect(window.locator('ui-shell')).toHaveCount(1);
  await expect(projectButton(window)).toBeVisible();
}

// covers: FCT-0014
test('shows the top bar of the opened project above the Éditeur, without any arrival notice', async () => {
  await withStudio(async ({ window }) => {
    await openKitHorizon(window);

    await expect(projectButton(window)).toContainText('Kit Horizon');
    await expect(projectButton(window)).toContainText('@horizon/ui 0.4.0');
    await expect(studioNav(window).getByRole('link')).toHaveCount(5);
    for (const label of ['Éditeur', 'Playground', 'Import de composants', 'Import de tokens', 'Export']) {
      await expect(navLink(window, label)).toBeVisible();
    }
    await expect(navLink(window, 'Éditeur')).toHaveAttribute('aria-current', 'page');
    await expect(window.getByRole('button', { name: /^Chat IA/ })).toBeVisible();
    await expect(window.getByRole('link', { name: 'Exporter', exact: true })).toBeVisible();
    await expect(window.getByRole('button', { name: /^Passer au thème (sombre|clair)$/ })).toBeVisible();
    await expect(window.getByRole('img', { name: 'Compte local' })).toBeVisible();
    await expect(window).toHaveTitle('Éditeur, UI-GEN');
    await expect(window.locator('ui-editor')).toBeVisible();
    await expect(window.locator('#frame-name')).toHaveText('hz-button');
    await expect(window.getByText('Disponible dans un prochain lot', { exact: true })).toBeHidden();
    await expect(window.getByText("L'écran Éditeur arrive avec le lot 1.", { exact: true })).toBeHidden();
    await expect(toasts(window)).toHaveCount(0);
  });
});

// covers: FCT-0015
test('activates a screen not yet built from its tab and shows its pending empty state', async () => {
  await withStudio(async ({ window }) => {
    await openKitHorizon(window);

    await navLink(window, 'Import de tokens').click();

    await expect(navLink(window, 'Import de tokens')).toHaveAttribute('aria-current', 'page');
    await expect(navLink(window, 'Éditeur')).not.toHaveAttribute('aria-current', 'page');
    await expect(window.getByRole('heading', { level: 1, name: 'Import de tokens', exact: true })).toBeAttached();
    await expect(window.getByText("L'écran Import de tokens arrive avec le lot 2.", { exact: true })).toBeVisible();
    await expect(window).toHaveTitle('Import de tokens, UI-GEN');
    await expect(window.locator('ui-shell')).toHaveCount(1);

    await window.getByRole('link', { name: 'Exporter', exact: true }).click();

    await expect(navLink(window, 'Export')).toHaveAttribute('aria-current', 'page');
    await expect(window.getByText("L'écran Export arrive avec le lot 5.", { exact: true })).toBeVisible();
    await expect(window).toHaveTitle('Export, UI-GEN');
  });
});

// covers: FCT-0012
test('goes back home from « Fermer le projet » and keeps the project in the recent list', async () => {
  await withStudio(async ({ window }) => {
    await openKitHorizon(window);

    await projectButton(window).click();
    await window.getByRole('button', { name: 'Fermer le projet', exact: true }).click();

    await expect(window.locator('ui-shell')).toHaveCount(0);
    await expect(window.getByRole('heading', { level: 1, name: 'UI-GEN' })).toBeVisible();
    await expect(window).toHaveTitle('Accueil, UI-GEN');
    await expect(toasts(window).filter({ hasText: 'Kit Horizon fermé' })).toContainText('Il reste dans les projets récents.');
    await expect(recentSection(window).getByRole('link')).toContainText('Kit Horizon');
  });
});

// covers: FCT-0012
test('goes back home from « Changer de projet » without any notice, and can open the project again', async () => {
  await withStudio(async ({ window }) => {
    await openKitHorizon(window);

    await projectButton(window).click();
    await window.getByRole('button', { name: 'Changer de projet', exact: true }).click();

    await expect(window.locator('ui-shell')).toHaveCount(0);
    await expect(window.getByRole('heading', { level: 1, name: 'UI-GEN' })).toBeVisible();
    await expect(toasts(window)).toHaveCount(0);

    await openKitHorizon(window);
    await expect(window).toHaveTitle('Éditeur, UI-GEN');
  });
});

// covers: FCT-0012
test('starts on the home screen after a relaunch that follows a closed project', async () => {
  await withStudio(async ({ window }) => {
    await openKitHorizon(window);
    await projectButton(window).click();
    await window.getByRole('button', { name: 'Fermer le projet', exact: true }).click();
    await expect(window.locator('ui-shell')).toHaveCount(0);
  });

  await withStudio(async ({ window }) => {
    await expect(window.getByRole('heading', { level: 1, name: 'UI-GEN' })).toBeVisible();
    await expect(window.locator('ui-shell')).toHaveCount(0);
    await expect(recentSection(window).getByRole('link')).toContainText('Kit Horizon');
  });
});

// covers: FCT-0019
test('drives the project menu and the tabs with the keyboard only', async () => {
  await withStudio(async ({ window }) => {
    await openKitHorizon(window);
    const changeProject = window.getByRole('button', { name: 'Changer de projet', exact: true });
    const closeProject = window.getByRole('button', { name: 'Fermer le projet', exact: true });

    await projectButton(window).focus();
    await window.keyboard.press('Enter');
    await expect(projectButton(window)).toHaveAttribute('aria-expanded', 'true');
    await expect(changeProject).toBeFocused();
    await window.keyboard.press('ArrowDown');
    await expect(closeProject).toBeFocused();
    await window.keyboard.press('ArrowDown');
    await expect(changeProject).toBeFocused();
    await window.keyboard.press('Escape');
    await expect(projectButton(window)).toHaveAttribute('aria-expanded', 'false');
    await expect(projectButton(window)).toBeFocused();

    await window.keyboard.press('Tab');
    await expect(navLink(window, 'Éditeur')).toBeFocused();
    await window.keyboard.press('Tab');
    await expect(navLink(window, 'Playground')).toBeFocused();
    await window.keyboard.press('Space');
    await expect(navLink(window, 'Playground')).toHaveAttribute('aria-current', 'page');
    await expect(window).toHaveTitle('Playground, UI-GEN');
    await window.keyboard.press('Tab');
    await expect(navLink(window, 'Import de composants')).toBeFocused();
    await window.keyboard.press('Enter');
    await expect(navLink(window, 'Import de composants')).toHaveAttribute('aria-current', 'page');
    await expect(window.getByText("L'écran Import de composants arrive avec le lot 7.", { exact: true })).toBeVisible();
    await expect(window.locator('ui-shell')).toHaveCount(1);
  });
});

// covers: FCT-0019
test('says the Chat IA arrives with lot 6 on Ctrl+I', async () => {
  await withStudio(async ({ window }) => {
    await openKitHorizon(window);

    await window.keyboard.press('Control+i');

    const toast = toasts(window).filter({ hasText: 'Disponible dans un prochain lot' });
    await expect(toast).toHaveCount(1);
    await expect(toast).toContainText('Le Chat IA arrive avec le lot 6.');
  });
});
