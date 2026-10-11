import { mkdir, readdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test, type ElectronApplication } from '@playwright/test';
import {
  answerNextFolderDialog,
  closeStudio,
  launchStudio,
  makeTempFolder,
  removeFolder,
  writeRecents,
  type BridgeHost,
  type Studio,
} from './studio-app';

const FOREIGN_PAGE = 'data:text/html,<title>foreign</title>';
const GENERIC_REFUSAL = /^Error: Error invoking remote method '[a-z:-]+': [A-Za-z]*Error: IPC message rejected \((?:untrusted-sender|invalid-payload)\)$/;

let userData: string;
let root: string;

test.beforeEach(async () => {
  userData = await makeTempFolder('user-data');
  root = await makeTempFolder('bridge-security');
});

test.afterEach(async () => {
  await removeFolder(userData);
  await removeFolder(root);
});

async function withStudio(run: (studio: Studio) => Promise<void>): Promise<void> {
  const studio = await launchStudio(userData);
  try {
    await run(studio);
  } finally {
    await closeStudio(studio.app);
  }
}

function mainProcessLog(app: ElectronApplication): () => string {
  const chunks: string[] = [];
  const record = (chunk: Buffer): void => {
    chunks.push(chunk.toString('utf8'));
  };
  app.process().stdout?.on('data', record);
  app.process().stderr?.on('data', record);
  return () => chunks.join('');
}

async function recentsFile(): Promise<string> {
  return readFile(join(userData, 'recent-projects.json'), 'utf8');
}

async function openForeignPage(app: ElectronApplication) {
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
  return opened;
}

test('refuses the calls of a page that is not the studio window before any effect', async () => {
  const kept = join(root, 'kit-horizon');
  await mkdir(kept);
  await writeRecents(userData, [{ path: kept, name: 'Kit Horizon', openedAt: '2026-10-10T08:00:00.000Z' }]);

  await withStudio(async ({ app }) => {
    const log = mainProcessLog(app);
    const foreignPage = await openForeignPage(app);
    const recentsBefore = await recentsFile();

    const failures = await foreignPage.evaluate(async () => {
      const bridge = (globalThis as unknown as BridgeHost).uiGen;
      const outcome = (call: Promise<unknown>): Promise<string> =>
        call.then(
          () => 'accepted',
          (error: unknown) => String(error),
        );
      return [await outcome(bridge.recents.list()), await outcome(bridge.recents.clear())];
    });

    for (const failure of failures) {
      expect(failure).toMatch(GENERIC_REFUSAL);
      expect(failure).toContain('IPC message rejected (untrusted-sender)');
    }
    expect(await recentsFile()).toBe(recentsBefore);
    await expect.poll(log).toContain('recents:clear');
    expect(log()).toContain('untrusted-sender');
  });
});

test('refuses a malformed message of the studio window with a generic error', async () => {
  await withStudio(async ({ window }) => {
    const failure = await window.evaluate(() =>
      (globalThis as unknown as BridgeHost).uiGen.recents.open(42 as unknown as string).then(
        () => 'accepted',
        (error: unknown) => String(error),
      ),
    );

    expect(failure).toMatch(GENERIC_REFUSAL);
    expect(failure).toContain('IPC message rejected (invalid-payload)');
  });
});

test('refuses a request with an extra key before any effect and logs neither its path nor its content', async () => {
  const location = join(root, 'granted-location');
  const injected = join(root, 'injected-target');
  await mkdir(location);

  await withStudio(async ({ app, window }) => {
    const log = mainProcessLog(app);
    await answerNextFolderDialog(app, location);
    expect(await window.evaluate(() => (globalThis as unknown as BridgeHost).uiGen.project.chooseLocation())).toEqual({
      canceled: false,
      path: location,
    });

    const failure = await window.evaluate(
      (request) =>
        (globalThis as unknown as BridgeHost).uiGen.project
          .create(request as unknown as Parameters<BridgeHost['uiGen']['project']['create']>[0])
          .then(
            () => 'accepted',
            (error: unknown) => String(error),
          ),
      { name: 'Kit Horizon', location, packageName: '@kit-horizon/ui', tagPrefix: 'kh-', template: injected },
    );

    expect(failure).toMatch(GENERIC_REFUSAL);
    expect(failure).toContain('IPC message rejected (invalid-payload)');
    expect(failure).not.toContain(location);
    expect(failure).not.toContain(injected);
    await expect.poll(log).toContain('project:create');
    expect(log()).toContain('invalid-payload');
    expect(log()).not.toContain(location);
    expect(log()).not.toContain(injected);
    expect(log()).not.toContain('Kit Horizon');
  });

  expect(await readdir(location)).toEqual([]);
  expect(await readdir(root)).toEqual(['granted-location']);
});
