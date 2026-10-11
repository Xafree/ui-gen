import { mkdir } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test, type ElectronApplication, type Page } from '@playwright/test';
import {
  closeStudio,
  launchStudio,
  makeTempFolder,
  removeFolder,
  writeFiles,
  writeRecents,
  type RecentEntry,
  type Studio,
} from '../project/studio-app';
import {
  closeReferenceWindow,
  expectFontsLoaded,
  expectIdenticalCaptures,
  openReferenceWindow,
  settledCapture,
  showMockup,
  type Theme,
} from './mockup-reference';

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
const RECENT_PATHS = '.ac-row .ui-list-item__meta';
const THEMES: readonly Theme[] = ['light', 'dark'];

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
};

let root: string;
let userData: string;

test.beforeEach(async () => {
  root = await makeTempFolder('parity-projects');
  userData = await makeTempFolder('parity-user-data');
  await writeFiles(join(root, 'kit-horizon'), KIT_HORIZON_FILES);
  await mkdir(join(root, 'lumen-ui'));
  await mkdir(join(root, 'verdure', 'composants'), { recursive: true });
});

test.afterEach(async () => {
  await removeFolder(root);
  await removeFolder(userData);
});

async function seedMockupRecents(): Promise<void> {
  const now = Date.now();
  const entry = (name: string, path: string, age: number): RecentEntry => ({
    name,
    path,
    openedAt: new Date(now - age).toISOString(),
  });
  await writeRecents(userData, [
    entry('Kit Horizon', join(root, 'kit-horizon'), 6 * MINUTE),
    entry('Lumen UI', join(root, 'lumen-ui'), DAY + 3 * HOUR),
    entry('Composants Verdure', join(root, 'verdure', 'composants'), 4 * DAY),
    entry('Prototype Pivot', join(root, 'pivot-ui'), 23 * DAY),
  ]);
}

async function withStudio(run: (studio: Studio) => Promise<void>): Promise<void> {
  await seedMockupRecents();
  const studio = await launchStudio(userData);
  try {
    await run(studio);
  } finally {
    await closeStudio(studio.app);
  }
}

async function chooseStudioTheme(window: Page, theme: Theme): Promise<void> {
  await window.evaluate((chosen) => {
    localStorage.setItem('uigen-theme', chosen);
  }, theme);
}

async function expectHomeWithMockupRecents(window: Page): Promise<void> {
  await expect(window.locator('ui-home')).toHaveCount(1);
  await expect(window.getByRole('region', { name: 'Projets récents' }).getByRole('listitem')).toHaveCount(4);
  await expect(window.locator(RECENT_PATHS)).toHaveCount(4);
}

async function openKitHorizonShell(window: Page): Promise<void> {
  await window.getByRole('region', { name: 'Projets récents' }).getByRole('link').filter({ hasText: 'Kit Horizon' }).click();
  await expect(window.locator('ui-shell')).toHaveCount(1);
  await expect(window.getByText('Disponible dans un prochain lot', { exact: true })).toBeVisible();
  await window.mouse.move(1430, 890);
}

async function captureMockupHome(app: ElectronApplication, theme: Theme): Promise<Buffer> {
  const reference = await openReferenceWindow(app);
  try {
    await showMockup(app, reference, 'accueil.html', theme);
    await expect(reference.page.locator(RECENT_PATHS)).toHaveCount(4);
    await reference.page.evaluate('document.querySelector(".ac-footer").remove()');
    await expect(reference.page.locator('.ac-footer')).toHaveCount(0);
    await expectFontsLoaded(reference.page);
    return await settledCapture(reference.page, reference.page.locator(RECENT_PATHS));
  } finally {
    await closeReferenceWindow(app, reference);
  }
}

async function captureMockupShell(app: ElectronApplication, theme: Theme): Promise<Buffer> {
  const reference = await openReferenceWindow(app);
  try {
    await showMockup(app, reference, 'a-venir.html', theme);
    await expect(reference.page.locator('.ui-topbar__project')).toHaveText('Kit Horizon');
    await expect(reference.page.getByText('Disponible dans un prochain lot', { exact: true })).toBeVisible();
    await expectFontsLoaded(reference.page);
    return await settledCapture(reference.page, reference.page.locator(RECENT_PATHS));
  } finally {
    await closeReferenceWindow(app, reference);
  }
}

// covers: FCT-0018
test('shows the home screen pixel for pixel like accueil.html, in light then dark theme', async () => {
  await withStudio(async ({ window }) => {
    await chooseStudioTheme(window, THEMES[0] ?? 'light');
  });

  for (const [index, theme] of THEMES.entries()) {
    await withStudio(async ({ app, window }) => {
      await expect(window.locator('html')).toHaveAttribute('data-theme', theme);
      await expectHomeWithMockupRecents(window);
      await expectFontsLoaded(window);
      const studio = await settledCapture(window, window.locator(RECENT_PATHS));
      const mockup = await captureMockupHome(app, theme);
      await expectIdenticalCaptures(app, test.info(), `home-${theme}`, mockup, studio);
      const next = THEMES[index + 1];
      if (next !== undefined) await chooseStudioTheme(window, next);
    });
  }
});

// covers: FCT-0018
test('shows the shell of an opened project pixel for pixel like a-venir.html, in light then dark theme', async () => {
  await withStudio(async ({ window }) => {
    await chooseStudioTheme(window, THEMES[0] ?? 'light');
  });

  for (const [index, theme] of THEMES.entries()) {
    await withStudio(async ({ app, window }) => {
      await expect(window.locator('html')).toHaveAttribute('data-theme', theme);
      await expectHomeWithMockupRecents(window);
      await openKitHorizonShell(window);
      await expect(window.getByRole('button', { name: /^Projet :/ })).toContainText('Kit Horizon');
      await expect(window.getByRole('button', { name: /^Projet :/ })).toContainText('@horizon/ui 0.4.0');
      await expect(window.locator('.ui-toast')).toHaveCount(0);
      await expectFontsLoaded(window);
      const studio = await settledCapture(window, window.locator(RECENT_PATHS));
      const mockup = await captureMockupShell(app, theme);
      await expectIdenticalCaptures(app, test.info(), `shell-${theme}`, mockup, studio);
      const next = THEMES[index + 1];
      if (next !== undefined) await chooseStudioTheme(window, next);
    });
  }
});
