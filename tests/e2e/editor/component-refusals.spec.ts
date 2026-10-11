import { join } from 'node:path';
import { expect, test, type ElectronApplication } from '@playwright/test';
import { makeTempFolder, removeFolder, type BridgeHost } from '../project/studio-app';
import {
  OUTSIDE_COMPONENT_FILE,
  lastOpened,
  listedPath,
  mainProcessLog,
  openProject,
  refusalOf,
  withStudio,
  writeKitHorizon,
} from './editor-project';

const FOREIGN_PAGE = 'data:text/html,<title>foreign</title>';
const GENERIC_REFUSAL =
  /^Error: Error invoking remote method 'component:[a-z-]+': [A-Za-z]*Error: IPC message rejected \((?:untrusted-sender|invalid-payload)\)$/;

let root: string;
let userData: string;

test.beforeEach(async () => {
  root = await makeTempFolder('editor-refusals');
  userData = await makeTempFolder('user-data');
});

test.afterEach(async () => {
  await removeFolder(root);
  await removeFolder(userData);
});

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

test('refuses to open a path that is not a listed component, with a generic error that leaks no path', async () => {
  const kit = join(root, 'kit-horizon');
  await writeKitHorizon(kit);

  await withStudio(userData, async (studio) => {
    const log = mainProcessLog(studio.app);
    await openProject(studio, kit);
    const featureList = await listedPath(studio.window, 'hz-feature-list');
    const separator = featureList.includes('\\') ? '\\' : '/';
    const folder = featureList.slice(0, featureList.lastIndexOf(separator) + 1);
    const refused = [
      `${folder}index.ts`,
      `${folder}hz-missing.ts`,
      `${featureList}${separator}..${separator}..${separator}..${separator}package.json`,
      join(kit, 'uigen.json'),
      join(kit, OUTSIDE_COMPONENT_FILE),
      'C:\\Windows\\win.ini',
    ];

    for (const path of refused) {
      const failure = await refusalOf(studio.window, path);
      expect(failure).toMatch(GENERIC_REFUSAL);
      expect(failure).toContain('IPC message rejected (invalid-payload)');
      expect(failure).not.toContain(kit);
    }
    expect(await refusalOf(studio.window, 42)).toContain('IPC message rejected (invalid-payload)');
    expect(await lastOpened(studio.window)).toBeNull();
    await expect.poll(log).toContain('component:open');
    expect(log()).toContain('invalid-payload');
    expect(log()).not.toContain('hz-missing');
    expect(log()).not.toContain('win.ini');
  });
});

test('refuses the component calls of a page that is not the studio window', async () => {
  const kit = join(root, 'kit-horizon');
  await writeKitHorizon(kit);

  await withStudio(userData, async (studio) => {
    await openProject(studio, kit);
    const featureList = await listedPath(studio.window, 'hz-feature-list');
    const foreignPage = await openForeignPage(studio.app);

    const failures = await foreignPage.evaluate(async (path) => {
      const components = (globalThis as unknown as BridgeHost).uiGen.components;
      const outcome = (call: Promise<unknown>): Promise<string> =>
        call.then(
          () => 'accepted',
          (error: unknown) => String(error),
        );
      return [
        await outcome(components.list()),
        await outcome(components.open(path)),
        await outcome(components.lastOpened()),
      ];
    }, featureList);

    expect(failures).toHaveLength(3);
    for (const failure of failures) {
      expect(failure).toMatch(GENERIC_REFUSAL);
      expect(failure).toContain('IPC message rejected (untrusted-sender)');
    }
    expect(await lastOpened(studio.window)).toBeNull();
  });
});
