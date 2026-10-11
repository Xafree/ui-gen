import { rm } from 'node:fs/promises';
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
  DARK_BG,
  LIGHT_BG,
  OFFER_CARD_FILE,
  PROJECT_TOKENS_FILE,
  canvasContent,
  canvasFrame,
  fileBytes,
  openRecentProject,
  rememberOpenedComponent,
  rootCustomProperty,
  setDarkPreview,
  studioCustomProperties,
  writeOfferCardProject,
} from './canvas-project';

let root: string;
let userData: string;
let project: string;

test.beforeEach(async () => {
  root = await makeTempFolder('canvas-render');
  userData = await makeTempFolder('canvas-render-user-data');
  project = join(root, 'kit-horizon');
  await writeOfferCardProject(project);
  await rememberOpenedComponent(userData, project, 'hz-offer-card');
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

// covers: FCT-0030
test('renders the real component in the canvas, one instance per variant value, with the project tokens', async () => {
  await withEditor(async ({ window }) => {
    const content = canvasContent(window);
    const cards = content.locator('hz-offer-card');

    await expect(cards).toHaveCount(3);
    for (const variant of ['starter', 'pro', 'equipe']) {
      const card = content.locator(`hz-offer-card[variant="${variant}"]`);
      await expect(card).toHaveCount(1);
      await expect(card.locator('p.variant')).toHaveText(`Offre ${variant}`);
      await expect(card.locator('h3.plan')).toHaveText('Horizon');
    }
    await expect(cards.first().locator('article.card')).toHaveCSS('background-color', LIGHT_BG);
    await expect(cards.first().locator('article.card')).toHaveCSS('padding-top', '16px');

    const frame = await canvasFrame(window);
    expect(await rootCustomProperty(frame, '--hz-color-bg')).toBe(LIGHT_BG);
  });
});

// covers: FCT-0030
test('marks the rendered elements of the copy with their node ids', async () => {
  await withEditor(async ({ window }) => {
    const card = canvasContent(window).locator('hz-offer-card[variant="pro"]');

    await expect(card.locator('article.card')).toHaveAttribute('data-uigen-node', /^[0-9][0-9a-z.]*$/);
    await expect(card.locator('h3.plan')).toHaveAttribute('data-uigen-node', /^[0-9][0-9a-z.]*$/);
  });
});

// covers: FCT-0031
test('leaves the component file and the project byte for byte as they were once the canvas has rendered', async () => {
  const before = await fileBytes(project, OFFER_CARD_FILE);
  const folderBefore = await snapshotFolder(project);

  await withEditor(async ({ window }) => {
    await expect(canvasContent(window).locator('hz-offer-card')).toHaveCount(3);
    await expect(canvasContent(window).locator('hz-offer-card').first().locator('h3.plan')).toHaveText('Horizon');

    expect((await fileBytes(project, OFFER_CARD_FILE)).equals(before)).toBe(true);
    expect(await snapshotFolder(project)).toEqual(folderBefore);
  });

  expect((await fileBytes(project, OFFER_CARD_FILE)).equals(before)).toBe(true);
  expect(await snapshotFolder(project)).toEqual(folderBefore);
});

// covers: FCT-0030
test('keeps the studio tokens and styles out of the canvas document', async () => {
  await withEditor(async ({ window }) => {
    await expect(canvasContent(window).locator('hz-offer-card')).toHaveCount(3);

    const frame = await canvasFrame(window);

    expect(await studioCustomProperties(frame)).toEqual([]);
    expect(await rootCustomProperty(frame, '--hz-color-bg')).toBe(LIGHT_BG);
  });
});

// covers: FCT-0044
test('renders the dark theme of the project tokens in the canvas only, without changing the studio theme', async () => {
  await withEditor(async ({ window }) => {
    const studioRoot = window.locator('html');
    const studioTheme = await studioRoot.getAttribute('data-theme');
    await expect(canvasContent(window).locator('hz-offer-card')).toHaveCount(3);

    await setDarkPreview(window, true);

    const content = canvasContent(window);
    await expect(content.locator('[data-hz-theme="dark"]')).not.toHaveCount(0);
    await expect(content.locator('hz-offer-card')).toHaveCount(3);
    await expect(content.locator('hz-offer-card').first().locator('article.card')).toHaveCSS(
      'background-color',
      DARK_BG,
    );
    await expect(content.locator('hz-offer-card').last().locator('article.card')).toHaveCSS(
      'background-color',
      DARK_BG,
    );
    expect(await studioRoot.getAttribute('data-theme')).toBe(studioTheme);
    await expect(window.locator('html[data-hz-theme], body[data-hz-theme]')).toHaveCount(0);
    expect(await studioCustomProperties(await canvasFrame(window))).toEqual([]);

    await setDarkPreview(window, false);

    await expect(canvasContent(window).locator('[data-hz-theme="dark"]')).toHaveCount(0);
    await expect(canvasContent(window).locator('hz-offer-card').first().locator('article.card')).toHaveCSS(
      'background-color',
      LIGHT_BG,
    );
    expect(await studioRoot.getAttribute('data-theme')).toBe(studioTheme);
  });
});

// covers: FCT-0030
test('renders the component without tokens and without error when the project has no tokens file', async () => {
  await rm(join(project, PROJECT_TOKENS_FILE));

  await withEditor(async ({ window }) => {
    const cards = canvasContent(window).locator('hz-offer-card');

    await expect(cards).toHaveCount(3);
    await expect(canvasContent(window).locator('hz-offer-card[variant="equipe"] p.variant')).toHaveText('Offre equipe');
    await expect(cards.first().locator('article.card')).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)');
    expect(await rootCustomProperty(await canvasFrame(window), '--hz-color-bg')).toBe('');
    const tags = await window.evaluate(async () =>
      (await (globalThis as unknown as BridgeHost).uiGen.components.list()).map(({ tag }) => tag),
    );
    expect(tags).toEqual(['hz-offer-card']);
    const nav = window.getByRole('navigation', { name: 'Écrans du studio' });
    await nav.getByRole('link', { name: 'Playground', exact: true }).click();
    await expect(nav.getByRole('link', { name: 'Playground', exact: true })).toHaveAttribute('aria-current', 'page');
  });
});
