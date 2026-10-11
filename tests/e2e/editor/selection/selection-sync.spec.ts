import { expect, test } from '@playwright/test';
import {
  cardPart,
  clickOnCanvas,
  clickRow,
  cssSize,
  disposeOfferCardEditor,
  expectBoxOver,
  hoverBox,
  nodeIdOf,
  prepareOfferCardEditor,
  revealRow,
  selectionBox,
  selectionSize,
  treeRow,
  withOfferCard,
  type OfferCardEditor,
} from './selection-project';

let editor: OfferCardEditor;

test.beforeEach(async () => {
  editor = await prepareOfferCardEditor('selection-sync');
});

test.afterEach(async () => {
  await disposeOfferCardEditor(editor);
});

// covers: FCT-0035
test('shows the layers of the opened component, one branch per variant, with their count', async () => {
  await withOfferCard(editor, async ({ window }) => {
    const titleId = await nodeIdOf(cardPart(window, 'pro', 'h3.plan'));

    await expect(window.getByRole('tree', { name: 'Calques de hz-offer-card' })).toBeVisible();
    for (const variant of ['starter', 'pro', 'equipe']) {
      await expect(treeRow(window, `${variant}:${titleId}`)).toHaveCount(1);
    }
    await expect(window.locator('#layer-count')).toHaveText(/^\d+\u00a0calques$/);
  });
});

// covers: FCT-0036
test('selects in the tree the element clicked in the canvas, and frames it with the selection box', async () => {
  await withOfferCard(editor, async ({ window }) => {
    const title = cardPart(window, 'pro', 'h3.plan');
    const titleId = await nodeIdOf(title);

    await clickOnCanvas(window, title);

    await expect(treeRow(window, `pro:${titleId}`)).toHaveAttribute('aria-selected', 'true');
    await expect(window.locator('[role="tree"] [role="treeitem"][aria-selected="true"]')).toHaveCount(1);
    await expect(treeRow(window, `pro:${titleId}`)).toBeVisible();
    await expect(selectionBox(window)).toHaveClass(/\bed-sel\b/);
    await expectBoxOver(selectionBox(window), title);
    await expect(selectionSize(window)).toHaveText(await cssSize(title));
  });
});

// covers: FCT-0036
test('tells apart the same element in two variant instances when it is clicked in the canvas', async () => {
  await withOfferCard(editor, async ({ window }) => {
    const starterText = cardPart(window, 'starter', 'p.variant');
    const equipeText = cardPart(window, 'equipe', 'p.variant');
    const textId = await nodeIdOf(equipeText);

    await clickOnCanvas(window, equipeText);
    await expect(treeRow(window, `equipe:${textId}`)).toHaveAttribute('aria-selected', 'true');
    await expectBoxOver(selectionBox(window), equipeText);

    await clickOnCanvas(window, starterText);
    await expect(treeRow(window, `starter:${textId}`)).toHaveAttribute('aria-selected', 'true');
    await expect(treeRow(window, `equipe:${textId}`)).toHaveAttribute('aria-selected', 'false');
    await expectBoxOver(selectionBox(window), starterText);
  });
});

// covers: FCT-0036
test('frames in the canvas the element whose layer is selected in the tree', async () => {
  await withOfferCard(editor, async ({ window }) => {
    const text = cardPart(window, 'pro', 'p.variant');
    const textId = await nodeIdOf(text);

    await clickRow(window, `pro:${textId}`);

    await expect(treeRow(window, `pro:${textId}`)).toHaveAttribute('aria-selected', 'true');
    await expectBoxOver(selectionBox(window), text);
    await expect(selectionSize(window)).toHaveText(await cssSize(text));
  });
});

// covers: FCT-0036
test('moves the selection box when another layer is chosen with the keyboard', async () => {
  await withOfferCard(editor, async ({ window }) => {
    const card = cardPart(window, 'starter', 'article.card');
    const title = cardPart(window, 'starter', 'h3.plan');
    const cardId = await nodeIdOf(card);
    const titleId = await nodeIdOf(title);
    await clickRow(window, `starter:${cardId}`);
    await expectBoxOver(selectionBox(window), card);
    await revealRow(window, `starter:${titleId}`);

    await treeRow(window, `starter:${titleId}`).focus();
    await window.keyboard.press('Enter');

    await expect(treeRow(window, `starter:${titleId}`)).toHaveAttribute('aria-selected', 'true');
    await expectBoxOver(selectionBox(window), title);
  });
});

// covers: FCT-0036
test('outlines in the canvas the element whose layer the pointer is over', async () => {
  await withOfferCard(editor, async ({ window }) => {
    const card = cardPart(window, 'pro', 'article.card');
    const text = cardPart(window, 'pro', 'p.variant');
    await clickRow(window, `pro:${await nodeIdOf(card)}`);
    const row = await revealRow(window, `pro:${await nodeIdOf(text)}`);

    await row.locator('.ui-tree__name').first().hover();

    await expectBoxOver(hoverBox(window), text);
  });
});
