import { expect, test } from '@playwright/test';
import { closeStudio, launchStudio, makeTempFolder, removeFolder, type BridgeHost, type Studio } from './studio-app';

const FOREIGN_PAGE = 'data:text/html,<title>foreign</title>';

let userData: string;

test.beforeEach(async () => {
  userData = await makeTempFolder('user-data');
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

test('refuses the calls of a page that is not the studio window', async () => {
  await withStudio(async ({ app }) => {
    const opened = app.waitForEvent('window');
    await app.evaluate(({ app: electronApp, BrowserWindow }, url) => {
      const foreign = new BrowserWindow({
        show: false,
        webPreferences: {
          preload: `${electronApp.getAppPath()}/out/preload/index.cjs`,
          contextIsolation: true,
          sandbox: true,
        },
      });
      return foreign.loadURL(url);
    }, FOREIGN_PAGE);
    const foreignPage = await opened;

    const failure = await foreignPage.evaluate(() =>
      (globalThis as unknown as BridgeHost).uiGen.recents.list().then(
        () => 'accepted',
        (error: unknown) => String(error),
      ),
    );

    expect(failure).toContain('IPC message rejected (untrusted-sender)');
  });
});

test('refuses a malformed message of the studio window and reveals nothing about it', async () => {
  await withStudio(async ({ window }) => {
    const failure = await window.evaluate(() =>
      (globalThis as unknown as BridgeHost).uiGen.recents.open(42 as unknown as string).then(
        () => 'accepted',
        (error: unknown) => String(error),
      ),
    );

    expect(failure).toContain('IPC message rejected (invalid-payload)');
    expect(failure).not.toContain('42');
  });
});
