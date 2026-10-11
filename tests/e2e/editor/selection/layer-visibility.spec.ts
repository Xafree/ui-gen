import { expect, test } from '@playwright/test';
import { snapshotFolder } from '../editor-project';
import {
  cardPart,
  clickOnCanvas,
  disposeOfferCardEditor,
  expectBoxOver,
  nodeIdOf,
  offerCardBytes,
  prepareOfferCardEditor,
  rowAction,
  selectionBox,
  treeRow,
  withOfferCard,
  type OfferCardEditor,
} from './selection-project';

let editor: OfferCardEditor;

test.beforeEach(async () => {
  editor = await prepareOfferCardEditor('layer-visibility');
});

test.afterEach(async () => {
  await disposeOfferCardEditor(editor);
});

// covers: FCT-0037
test('hides a layer in the canvas only, leaving the component file byte for byte as it was', async () => {
  const before = await offerCardBytes(editor);
  const folderBefore = await snapshotFolder(editor.project);

  await withOfferCard(editor, async ({ window }) => {
    const proTitle = cardPart(window, 'pro', 'h3.plan');
    const titleId = await nodeIdOf(proTitle);

    await rowAction(window, `pro:${titleId}`, 'hide');

    await expect(proTitle).toBeHidden();
    await expect(cardPart(window, 'starter', 'h3.plan')).toBeVisible();
    await expect(cardPart(window, 'pro', 'p.variant')).toBeVisible();
    await expect(treeRow(window, `pro:${titleId}`)).toHaveAttribute('aria-label', /, masqué$/);
    expect((await offerCardBytes(editor)).equals(before)).toBe(true);

    await rowAction(window, `pro:${titleId}`, 'hide');

    await expect(proTitle).toBeVisible();
    await expect(proTitle).toHaveText('Horizon');
    expect((await offerCardBytes(editor)).equals(before)).toBe(true);
  });

  expect((await offerCardBytes(editor)).equals(before)).toBe(true);
  expect(await snapshotFolder(editor.project)).toEqual(folderBefore);
});

// covers: FCT-0037
test('selects the parent of a locked layer when it is clicked in the canvas, without touching the file', async () => {
  const before = await offerCardBytes(editor);

  await withOfferCard(editor, async ({ window }) => {
    const card = cardPart(window, 'pro', 'article.card');
    const title = cardPart(window, 'pro', 'h3.plan');
    const cardId = await nodeIdOf(card);
    const titleId = await nodeIdOf(title);

    await rowAction(window, `pro:${titleId}`, 'lock');
    await expect(treeRow(window, `pro:${titleId}`)).toHaveAttribute('aria-label', /, verrouillé$/);

    await clickOnCanvas(window, title);

    await expect(treeRow(window, `pro:${cardId}`)).toHaveAttribute('aria-selected', 'true');
    await expect(treeRow(window, `pro:${titleId}`)).toHaveAttribute('aria-selected', 'false');
    await expectBoxOver(selectionBox(window), card);
    await expect(title).toBeVisible();
    expect((await offerCardBytes(editor)).equals(before)).toBe(true);
  });

  expect((await offerCardBytes(editor)).equals(before)).toBe(true);
});

// covers: FCT-0037
test('selects a layer in the canvas again once it is unlocked', async () => {
  await withOfferCard(editor, async ({ window }) => {
    const title = cardPart(window, 'pro', 'h3.plan');
    const titleId = await nodeIdOf(title);
    await rowAction(window, `pro:${titleId}`, 'lock');
    await expect(treeRow(window, `pro:${titleId}`)).toHaveAttribute('aria-label', /, verrouillé$/);

    await rowAction(window, `pro:${titleId}`, 'lock');
    await expect(treeRow(window, `pro:${titleId}`)).not.toHaveAttribute('aria-label', /verrouillé/);
    await clickOnCanvas(window, title);

    await expect(treeRow(window, `pro:${titleId}`)).toHaveAttribute('aria-selected', 'true');
    await expectBoxOver(selectionBox(window), title);
  });
});

// covers: FCT-0037
test('locks only the layer of that variant instance', async () => {
  await withOfferCard(editor, async ({ window }) => {
    const proTitle = cardPart(window, 'pro', 'h3.plan');
    const starterTitle = cardPart(window, 'starter', 'h3.plan');
    const titleId = await nodeIdOf(proTitle);
    await rowAction(window, `pro:${titleId}`, 'lock');
    await expect(treeRow(window, `pro:${titleId}`)).toHaveAttribute('aria-label', /, verrouillé$/);

    await clickOnCanvas(window, starterTitle);

    await expect(treeRow(window, `starter:${titleId}`)).toHaveAttribute('aria-selected', 'true');
    await expectBoxOver(selectionBox(window), starterTitle);
  });
});
