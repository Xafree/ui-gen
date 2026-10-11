import { expect, test, type ElectronApplication, type Page } from '@playwright/test';
import { closeStudio, launchStudio, makeTempFolder, removeFolder, type Studio } from '../project/studio-app';

type WindowsTheme = 'light' | 'dark';

const LIGHT_TOP_BAR = 'rgb(251, 251, 250)';
const DARK_TOP_BAR = 'rgb(24, 24, 23)';

let userData: string;

test.beforeEach(async () => {
  userData = await makeTempFolder('theme-user-data');
});

test.afterEach(async () => {
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

async function setWindowsTheme(app: ElectronApplication, theme: WindowsTheme): Promise<void> {
  await app.evaluate(({ nativeTheme }, source) => {
    nativeTheme.themeSource = source;
  }, theme);
}

function topBar(window: Page) {
  return window.locator('ui-home').locator('header').first();
}

async function expectStudioTheme(window: Page, theme: WindowsTheme): Promise<void> {
  await expect(topBar(window)).toHaveCSS('background-color', theme === 'dark' ? DARK_TOP_BAR : LIGHT_TOP_BAR);
  const toggleLabel = theme === 'dark' ? 'Passer au thème clair' : 'Passer au thème sombre';
  await expect(window.getByRole('button', { name: toggleLabel })).toBeVisible();
}

// covers: FCT-0016
test('switches the whole studio between light and dark from the theme button', async () => {
  await withStudio(async ({ app, window }) => {
    await setWindowsTheme(app, 'light');
    await expectStudioTheme(window, 'light');

    await window.getByRole('button', { name: 'Passer au thème sombre' }).click();

    await expectStudioTheme(window, 'dark');
    await expect(window.locator('html')).toHaveAttribute('data-theme', 'dark');

    await window.getByRole('button', { name: 'Passer au thème clair' }).click();

    await expectStudioTheme(window, 'light');
    await expect(window.locator('html')).toHaveAttribute('data-theme', 'light');
  });
});

// covers: FCT-0017
test('reopens in the dark theme chosen before the relaunch, even with a light Windows theme', async () => {
  await withStudio(async ({ app, window }) => {
    await setWindowsTheme(app, 'light');
    await expectStudioTheme(window, 'light');
    await window.getByRole('button', { name: 'Passer au thème sombre' }).click();
    await expectStudioTheme(window, 'dark');
    expect(await window.evaluate(() => localStorage.getItem('uigen-theme'))).toBe('dark');
  });

  await withStudio(async ({ app, window }) => {
    await setWindowsTheme(app, 'light');
    await expectStudioTheme(window, 'dark');
  });
});

// covers: FCT-0017
test('reopens in the light theme chosen before the relaunch, even with a dark Windows theme', async () => {
  await withStudio(async ({ app, window }) => {
    await setWindowsTheme(app, 'dark');
    await expectStudioTheme(window, 'dark');
    await window.getByRole('button', { name: 'Passer au thème clair' }).click();
    await expectStudioTheme(window, 'light');
  });

  await withStudio(async ({ app, window }) => {
    await setWindowsTheme(app, 'dark');
    await expectStudioTheme(window, 'light');
  });
});

// covers: FCT-0020
test('follows the Windows theme while no theme was chosen', async () => {
  await withStudio(async ({ app, window }) => {
    await setWindowsTheme(app, 'dark');
    await expectStudioTheme(window, 'dark');

    await setWindowsTheme(app, 'light');
    await expectStudioTheme(window, 'light');

    expect(await window.evaluate(() => localStorage.getItem('uigen-theme'))).toBeNull();
  });
});
