import { join } from 'node:path';
import { expect, type Locator, type Page } from '@playwright/test';
import { closeStudio, launchStudio, makeTempFolder, removeFolder, type Studio } from '../../project/studio-app';
import {
  OFFER_CARD_FILE,
  canvasContent,
  fileBytes,
  openRecentProject,
  rememberOpenedComponent,
  writeOfferCardProject,
} from '../canvas/canvas-project';

export interface OfferCardEditor {
  readonly root: string;
  readonly userData: string;
  readonly project: string;
}

export interface Box {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

const BOX_TOLERANCE = 2;
const ACTUAL_SIZE_LABEL = '100 %';

export async function prepareOfferCardEditor(label: string): Promise<OfferCardEditor> {
  const root = await makeTempFolder(label);
  const userData = await makeTempFolder(`${label}-user-data`);
  const project = join(root, 'kit-horizon');
  await writeOfferCardProject(project);
  await rememberOpenedComponent(userData, project, 'hz-offer-card');
  return { root, userData, project };
}

export async function disposeOfferCardEditor(editor: OfferCardEditor): Promise<void> {
  await removeFolder(editor.root);
  await removeFolder(editor.userData);
}

async function zoomToActualSize(window: Page): Promise<void> {
  const zoomValue = window.locator('#zoom-value');
  await zoomValue.click();
  await expect(zoomValue).toHaveText(ACTUAL_SIZE_LABEL);
}

export async function withOfferCard(editor: OfferCardEditor, run: (studio: Studio) => Promise<void>): Promise<void> {
  const studio = await launchStudio(editor.userData);
  try {
    await openRecentProject(studio.window);
    await expect(canvasContent(studio.window).locator('hz-offer-card')).toHaveCount(3);
    await expect(canvasContent(studio.window).locator('hz-offer-card[variant="pro"] h3.plan')).toHaveText('Horizon');
    await zoomToActualSize(studio.window);
    await run(studio);
  } finally {
    await closeStudio(studio.app);
  }
}

export async function offerCardBytes(editor: OfferCardEditor): Promise<Buffer> {
  return fileBytes(editor.project, OFFER_CARD_FILE);
}

export function cardPart(window: Page, variant: string, selector: string): Locator {
  return canvasContent(window).locator(`hz-offer-card[variant="${variant}"]`).locator(selector);
}

export async function nodeIdOf(part: Locator): Promise<string> {
  const id = await part.getAttribute('data-uigen-node');
  if (id === null) throw new Error('The canvas element carries no node id');
  return id;
}

export function treeRow(window: Page, id: string): Locator {
  return window.locator(`[role="tree"] [role="treeitem"][data-id="${id}"]`);
}

export async function revealRow(window: Page, id: string): Promise<Locator> {
  const row = treeRow(window, id);
  await expect(row).toHaveCount(1);
  const collapsed = window.locator(`[role="tree"] [role="treeitem"][aria-expanded="false"]:has([data-id="${id}"])`);
  for (let guard = 0; guard < 8 && (await collapsed.count()) > 0; guard += 1) {
    await collapsed.first().locator('.ui-tree__toggle').first().click();
  }
  await expect(row).toBeVisible();
  return row;
}

export async function clickRow(window: Page, id: string): Promise<void> {
  const row = await revealRow(window, id);
  await row.locator('.ui-tree__name').first().click();
}

export async function rowAction(window: Page, id: string, action: 'hide' | 'lock'): Promise<void> {
  const row = await revealRow(window, id);
  await row.locator('.ui-tree__row').first().hover();
  await row.locator(`.ui-tree__actions [data-act="${action}"]`).first().click();
}

export async function boxOf(locator: Locator): Promise<Box> {
  const box = await locator.boundingBox();
  if (box === null) throw new Error('The element has no box on screen');
  return box;
}

export async function clickOnCanvas(window: Page, part: Locator): Promise<void> {
  const box = await boxOf(part);
  await window.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
}

function near(actual: Box, expected: Box): boolean {
  return (
    Math.abs(actual.x - expected.x) <= BOX_TOLERANCE &&
    Math.abs(actual.y - expected.y) <= BOX_TOLERANCE &&
    Math.abs(actual.width - expected.width) <= BOX_TOLERANCE &&
    Math.abs(actual.height - expected.height) <= BOX_TOLERANCE
  );
}

export async function expectBoxOver(box: Locator, part: Locator): Promise<void> {
  await expect(box).toBeVisible();
  await expect.poll(async () => near(await boxOf(box), await boxOf(part))).toBe(true);
}

export async function cssSize(part: Locator): Promise<string> {
  return part.evaluate((element: { getBoundingClientRect(): { width: number; height: number } }) => {
    const rect = element.getBoundingClientRect();
    return `${String(Math.round(rect.width))} × ${String(Math.round(rect.height))}`;
  });
}

export function selectionBox(window: Page): Locator {
  return window.locator('#sel-box');
}

export function hoverBox(window: Page): Locator {
  return window.locator('#hover-box');
}

export function selectionSize(window: Page): Locator {
  return window.locator('#sel-size');
}
