import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import {
  closeStudio,
  launchStudio,
  makeTempFolder,
  removeFolder,
  type BridgeHost,
  type Studio,
} from '../../project/studio-app';
import { snapshotFolder } from '../editor-project';
import {
  OUTSIDE_SECRET_FILE,
  PROBE_CARD_FILE,
  canvasContent,
  canvasFrame,
  canvasIframe,
  fileBytes,
  openRecentProject,
  rememberOpenedComponent,
  writeProbeCardProject,
} from './canvas-project';

let root: string;
let userData: string;
let project: string;

test.beforeEach(async () => {
  root = await makeTempFolder('canvas-isolation');
  userData = await makeTempFolder('canvas-isolation-user-data');
  project = join(root, 'kit-horizon');
  await writeProbeCardProject(project);
  await rememberOpenedComponent(userData, project, 'hz-probe-card');
});

test.afterEach(async () => {
  await removeFolder(root);
  await removeFolder(userData);
});

async function withEditor(run: (studio: Studio) => Promise<void>): Promise<void> {
  const studio = await launchStudio(userData);
  try {
    await openRecentProject(studio.window);
    await run(studio);
  } finally {
    await closeStudio(studio.app);
  }
}

async function probeResults(studio: Studio): Promise<unknown> {
  const probe = canvasContent(studio.window).locator('hz-probe-card');
  await expect(probe).toHaveCount(1);
  await expect(probe).toHaveAttribute('data-probe', /^\{.+\}$/);
  const raw = await probe.getAttribute('data-probe');
  const results: unknown = JSON.parse(raw ?? '{}');
  return results;
}

// covers: FCT-0032
test('runs the component in a sandboxed iframe served by the uigen-canvas scheme', async () => {
  await withEditor(async ({ window }) => {
    const iframe = canvasIframe(window);

    await expect(iframe).toHaveAttribute('sandbox', 'allow-scripts');
    await expect(iframe).toHaveAttribute('src', /^uigen-canvas:\/\/.+\/harness\.html(\?.*)?$/);
    await expect(canvasContent(window).locator('hz-probe-card p.probe')).toHaveText('Sonde');
  });
});

// covers: FCT-0032
test('denies the component the studio API, Node, the network and the disk outside the project', async () => {
  await withEditor(async (studio) => {
    const results = await probeResults(studio);

    expect(results).toEqual({
      bridge: 'undefined',
      process: 'undefined',
      require: 'undefined',
      network: 'failed',
      ownFile: 'failed',
      outsideFetch: 'failed',
      outsideImport: 'failed',
      outsideBackslashImport: 'failed',
      parentDocument: 'failed',
      popup: 'blocked',
    });
    const frame = await canvasFrame(studio.window);
    expect(await frame.evaluate('typeof globalThis.uiGen')).toBe('undefined');
    expect(await frame.evaluate('typeof globalThis.leakedSecret')).toBe('undefined');
  });
});

// covers: FCT-0032
test('refuses the requests a component makes from the canvas frame itself', async () => {
  await withEditor(async (studio) => {
    await probeResults(studio);
    const frame = await canvasFrame(studio.window);
    const pageUrl = frame.url();

    const outcomes: unknown = await frame.evaluate(`(async () => {
      const attempt = (run) => run().then(() => 'succeeded', () => 'failed');
      return {
        network: await attempt(() => fetch('https://example.com/')),
        outside: await attempt(() => fetch(new URL('../../../${OUTSIDE_SECRET_FILE}', location.href).href)),
        encodedOutside: await attempt(() => import(new URL('..%2f..%2f..%2f${OUTSIDE_SECRET_FILE}', location.href).href)),
        popup: window.open('https://example.com/') === null ? 'blocked' : 'opened',
      };
    })()`);

    expect(pageUrl.startsWith('uigen-canvas://')).toBe(true);
    expect(outcomes).toEqual({ network: 'failed', outside: 'failed', encodedOutside: 'failed', popup: 'blocked' });
  });
});

// covers: FCT-0032
test('keeps the studio usable, in a single window, after the component tried to escape', async () => {
  await withEditor(async (studio) => {
    const { app, window } = studio;
    const studioUrl = window.url();
    const before = await fileBytes(project, PROBE_CARD_FILE);
    const folderBefore = await snapshotFolder(project);

    await probeResults(studio);

    expect(app.windows()).toHaveLength(1);
    expect(await app.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows().length)).toBe(1);
    expect(window.url()).toBe(studioUrl);
    const tags = await window.evaluate(async () =>
      (await (globalThis as unknown as BridgeHost).uiGen.components.list()).map(({ tag }) => tag),
    );
    expect(tags).toEqual(['hz-probe-card']);
    const nav = window.getByRole('navigation', { name: 'Écrans du studio' });
    await nav.getByRole('link', { name: 'Playground', exact: true }).click();
    await expect(nav.getByRole('link', { name: 'Playground', exact: true })).toHaveAttribute('aria-current', 'page');
    expect((await fileBytes(project, PROBE_CARD_FILE)).equals(before)).toBe(true);
    expect(await snapshotFolder(project)).toEqual(folderBefore);
  });
});
