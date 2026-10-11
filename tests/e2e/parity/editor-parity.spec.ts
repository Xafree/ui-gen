import { join } from 'node:path';
import { expect, test, type ElectronApplication, type Page } from '@playwright/test';
import { makeTempFolder, removeFolder, writeFiles } from '../project/studio-app';
import { canvasContent, openRecentProject } from '../editor/canvas/canvas-project';
import {
  PRICING_CARD_FILE,
  fixtureText,
  listedPath,
  openComponent,
  openProject,
  withStudio,
  writeEmptyLibrary,
  writeKitHorizon,
} from '../editor/editor-project';
import {
  expectEditorOn,
  rememberRecentProject,
  writeKitHorizonSupport,
  writeTsconfig,
} from '../editor/screen/editor-screen';
import { clickRow, nodeIdOf, selectionBox, treeRow } from '../editor/selection/selection-project';
import {
  closeReferenceWindow,
  expectFontsLoaded,
  expectIdenticalCaptures,
  mockupPath,
  openReferenceWindow,
  settledCapture,
  type Theme,
} from './mockup-reference';

const THEMES: readonly Theme[] = ['light', 'dark'];
const FRAME_BODY = '#frame-body';
const PRICING_TAG = 'hz-pricing-card';
const DATA_DEPENDENT_REGIONS: readonly string[] = [
  '#tree',
  '#layer-count',
  '#comp-list',
  '#comp-count',
  '#code-pre',
  '#status-cursor',
  '#status-sel',
  '#insp-name',
  '#insp-sub',
  '#rule-count',
  '#tp-props .ui-section > :not(.ui-section__header)',
  '#frame-body',
];
const MASK_OVERLAY_ID = 'parity-data-masks';
const MASK_OVERLAY_COLOR = '#FF00FF';

const HORIZON_PACKAGE = `{
  "name": "@horizon/ui",
  "version": "0.4.0",
  "type": "module",
  "dependencies": {
    "lit": "^3.3.0"
  }
}
`;

const LOCALE_LINE = "@property() locale = 'fr-FR';";
const FORMAT_PRICE_IMPORT = "import { formatPrice } from '../utils/format-price.js';\n";
const FORMAT_PRICE_CALL = /\$\{formatPrice\([^)]*\)\}/;

interface Box {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

type MaskBoxes = Readonly<Record<string, readonly Box[]>>;

interface MaskedCapture {
  readonly image: Buffer;
  readonly boxes: MaskBoxes;
}

interface MockupState {
  readonly name: string;
  readonly query: Readonly<Record<string, string>>;
  readonly customize?: (project: string) => Promise<void>;
  readonly ready: (window: Page) => Promise<void>;
}

async function rewritePricingCard(project: string, rewrite: (source: string) => string): Promise<void> {
  const source = await fixtureText(PRICING_CARD_FILE);
  const rewritten = rewrite(source);
  expect(rewritten, 'the fixture must actually change').not.toBe(source);
  await writeFiles(project, { [PRICING_CARD_FILE]: rewritten });
}

async function expectComponentOnCanvas(window: Page): Promise<void> {
  await expectEditorOn(window, PRICING_TAG);
  await expect(window.locator('#layer-count')).toHaveText(/\d/);
  await expect(window.locator('ui-canvas-frame iframe').last().contentFrame().locator(PRICING_TAG).first()).toBeAttached();
}

async function selectProCta(window: Page): Promise<void> {
  const cta = canvasContent(window).locator(`${PRICING_TAG}[variant="pro"] hz-button.cta`);
  const row = `pro:${await nodeIdOf(cta)}`;
  await clickRow(window, row);
  await expect(treeRow(window, row)).toHaveAttribute('aria-selected', 'true');
  await expect(selectionBox(window)).toBeVisible();
  await expect(window.locator('#insp-name')).toBeVisible();
}

async function expectProCtaSelected(window: Page): Promise<void> {
  await expectComponentOnCanvas(window);
  await selectProCta(window);
}

const DEFAULT_STATE: MockupState = { name: 'default', query: {}, ready: expectProCtaSelected };

const NO_PROBLEMS_STATE: MockupState = {
  name: 'no-problems',
  query: { problemes: '0' },
  customize: (project) =>
    rewritePricingCard(project, (source) =>
      source.replace(FORMAT_PRICE_IMPORT, '').replace(FORMAT_PRICE_CALL, () => '${this.price}'),
    ),
  ready: expectProCtaSelected,
};

const EMPTY_LIBRARY_STATE: MockupState = {
  name: 'empty-library',
  query: { composants: 'vide' },
  ready: async (window) => {
    await expect(window.locator('ui-editor')).toBeVisible();
    await expect(window.locator('#canvas-empty')).toBeVisible();
    await expect(window.locator('#layers-empty')).toBeVisible();
  },
};

const SYNTAX_ERROR_STATE: MockupState = {
  name: 'syntax-error',
  query: { etat: 'syntaxe' },
  customize: (project) =>
    rewritePricingCard(project, (source) => source.replace(LOCALE_LINE, LOCALE_LINE.slice(0, -1))),
  ready: async (window) => {
    await expect(window.locator('ui-editor')).toBeVisible();
    await expect(window.locator('#problem-count')).toBeVisible();
    await selectProCta(window);
  },
};

let root: string;
let userData: string;
let project: string;

test.beforeEach(async () => {
  root = await makeTempFolder('editor-parity');
  userData = await makeTempFolder('editor-parity-user-data');
  project = join(root, 'kit-horizon');
});

test.afterEach(async () => {
  await removeFolder(root);
  await removeFolder(userData);
});

interface RectLike {
  readonly left: number;
  readonly top: number;
  readonly right: number;
  readonly bottom: number;
}

interface NodeLike {
  readonly parentNode: NodeLike | null;
  readonly host?: NodeLike;
  readonly nodeType: number;
  getBoundingClientRect(): RectLike;
}

interface ClipScope {
  readonly innerWidth: number;
  readonly innerHeight: number;
  getComputedStyle(node: NodeLike): { readonly display: string; readonly overflowX: string; readonly overflowY: string };
}

async function measureRegion(page: Page, selector: string): Promise<Box[]> {
  return page.locator(selector).evaluateAll((elements: NodeLike[]) => {
    const scope = globalThis as unknown as ClipScope;
    const ELEMENT_NODE = 1;
    const parentOf = (node: NodeLike): NodeLike | null => node.parentNode?.host ?? node.parentNode;
    const boxes: { x: number; y: number; width: number; height: number }[] = [];
    for (const element of elements) {
      const own = element.getBoundingClientRect();
      let left = Math.max(own.left, 0);
      let top = Math.max(own.top, 0);
      let right = Math.min(own.right, scope.innerWidth);
      let bottom = Math.min(own.bottom, scope.innerHeight);
      for (let node = parentOf(element); node?.nodeType === ELEMENT_NODE; node = parentOf(node)) {
        const style = scope.getComputedStyle(node);
        if (style.display === 'contents' || (style.overflowX === 'visible' && style.overflowY === 'visible')) continue;
        const clip = node.getBoundingClientRect();
        left = Math.max(left, clip.left);
        top = Math.max(top, clip.top);
        right = Math.min(right, clip.right);
        bottom = Math.min(bottom, clip.bottom);
      }
      if (right <= left || bottom <= top) continue;
      const x = Math.floor(left);
      const y = Math.floor(top);
      boxes.push({ x, y, width: Math.ceil(right) - x, height: Math.ceil(bottom) - y });
    }
    return boxes;
  });
}

async function measureRegions(page: Page): Promise<Record<string, Box[]>> {
  const measured: Record<string, Box[]> = {};
  for (const selector of DATA_DEPENDENT_REGIONS) measured[selector] = await measureRegion(page, selector);
  return measured;
}

async function settledRegions(page: Page): Promise<Record<string, Box[]>> {
  let previous = '';
  let latest: Record<string, Box[]> = {};
  await expect
    .poll(
      async () => {
        latest = await measureRegions(page);
        const serialized = JSON.stringify(latest);
        const stable = serialized === previous;
        previous = serialized;
        return stable;
      },
      { intervals: [250], timeout: 15_000 },
    )
    .toBe(true);
  return latest;
}

function paintMasksExpression(boxes: readonly Box[]): string {
  return `(() => {
    const host = document.createElement('div');
    host.id = ${JSON.stringify(MASK_OVERLAY_ID)};
    host.style.cssText = 'position:fixed;inset:0;pointer-events:none;z-index:2147483647;';
    for (const box of ${JSON.stringify(boxes)}) {
      const cover = document.createElement('div');
      cover.style.cssText = 'position:absolute;background:${MASK_OVERLAY_COLOR};left:' + box.x + 'px;top:' + box.y + 'px;width:' + box.width + 'px;height:' + box.height + 'px;';
      host.append(cover);
    }
    document.documentElement.append(host);
  })()`;
}

async function paintMasks(page: Page, boxes: readonly Box[]): Promise<void> {
  await page.evaluate(paintMasksExpression(boxes));
}

async function removeMasks(page: Page): Promise<void> {
  await page.evaluate(`document.getElementById(${JSON.stringify(MASK_OVERLAY_ID)})?.remove()`);
}

async function maskedCapture(page: Page): Promise<MaskedCapture> {
  const boxes = await settledRegions(page);
  await paintMasks(page, Object.values(boxes).flat());
  try {
    return { image: await settledCapture(page, page.locator(FRAME_BODY)), boxes };
  } finally {
    await removeMasks(page);
  }
}

function describeBoxes(boxes: readonly Box[]): string {
  if (boxes.length === 0) return 'none';
  return boxes.map((box) => `${String(box.x)},${String(box.y)} ${String(box.width)}x${String(box.height)}`).join(' | ');
}

function maskBoxGaps(label: string, mockup: MaskBoxes, studio: MaskBoxes): string[] {
  const gaps: string[] = [];
  for (const selector of DATA_DEPENDENT_REGIONS) {
    const expected = mockup[selector] ?? [];
    const actual = studio[selector] ?? [];
    if (JSON.stringify(expected) === JSON.stringify(actual)) continue;
    gaps.push(`${label}: masked box ${selector} differs, mockup ${describeBoxes(expected)} vs studio ${describeBoxes(actual)}`);
  }
  return gaps;
}

async function chooseStudioTheme(window: Page, theme: Theme): Promise<void> {
  await window.evaluate((chosen) => {
    localStorage.setItem('uigen-theme', chosen);
  }, theme);
}

async function prepareProject(empty: boolean, state: MockupState): Promise<void> {
  if (empty) {
    await writeEmptyLibrary(project);
  } else {
    await writeKitHorizon(project);
    await writeTsconfig(project);
    await writeKitHorizonSupport(project);
  }
  await writeFiles(project, { 'package.json': HORIZON_PACKAGE });
  await withStudio(userData, async (studio) => {
    await openProject(studio, project);
    if (!empty) await openComponent(studio.window, await listedPath(studio.window, PRICING_TAG));
    await chooseStudioTheme(studio.window, 'light');
  });
  await rememberRecentProject(userData, project);
  await state.customize?.(project);
}

async function loadMockupState(
  app: ElectronApplication,
  windowId: number,
  query: Readonly<Record<string, string>>,
): Promise<void> {
  await app.evaluate(
    async ({ BrowserWindow }, target) => {
      const window = BrowserWindow.fromId(target.id);
      if (window === null) throw new Error(`reference window ${String(target.id)} is gone`);
      await window.loadFile(target.file, { query: target.query });
    },
    { id: windowId, file: mockupPath('editeur.html'), query },
  );
}

async function captureMockupEditor(app: ElectronApplication, state: MockupState, theme: Theme): Promise<MaskedCapture> {
  const reference = await openReferenceWindow(app);
  try {
    await loadMockupState(app, reference.id, state.query);
    await reference.page.evaluate((chosen) => {
      localStorage.setItem('uigen-theme', chosen);
    }, theme);
    await loadMockupState(app, reference.id, state.query);
    expect(await reference.page.evaluate('document.documentElement.getAttribute("data-theme")')).toBe(theme);
    await expect(reference.page.locator('#layer-count')).toHaveText(/\d/);
    await reference.page.mouse.move(1430, 890);
    await expectFontsLoaded(reference.page);
    return await maskedCapture(reference.page);
  } finally {
    await closeReferenceWindow(app, reference);
  }
}

function firstLine(error: unknown): string {
  const text = error instanceof Error ? error.message : String(error);
  return text.split('\n')[0] ?? text;
}

async function expectEditorParity(state: MockupState, empty: boolean): Promise<void> {
  test.setTimeout(240_000);
  await prepareProject(empty, state);
  const failures: string[] = [];

  for (const [index, theme] of THEMES.entries()) {
    await withStudio(userData, async ({ app, window }) => {
      await expect(window.locator('html')).toHaveAttribute('data-theme', theme);
      await openRecentProject(window);
      await state.ready(window);
      await window.mouse.move(1430, 890);
      await expectFontsLoaded(window);
      const studio = await maskedCapture(window);
      const mockup = await captureMockupEditor(app, state, theme);
      const name = `editor-${state.name}-${theme}`;
      try {
        await expectIdenticalCaptures(app, test.info(), name, mockup.image, studio.image);
      } catch (error) {
        failures.push(firstLine(error));
      }
      failures.push(...maskBoxGaps(name, mockup.boxes, studio.boxes));
      const next = THEMES[index + 1];
      if (next !== undefined) await chooseStudioTheme(window, next);
    });
  }

  expect(failures, 'every state and theme must match the mockup outside the canvas frame and the data-dependent regions').toEqual([]);
}

// covers: FCT-0030
// covers: FCT-0035
test('shows the editor pixel for pixel like editeur.html outside the canvas frame, in light then dark theme', async () => {
  await expectEditorParity(DEFAULT_STATE, false);
});

// covers: FCT-0040
test('shows the editor without problems pixel for pixel like editeur.html?problemes=0 outside the canvas frame, in light then dark theme', async () => {
  await expectEditorParity(NO_PROBLEMS_STATE, false);
});

// covers: FCT-0027
test('shows the empty library pixel for pixel like editeur.html?composants=vide outside the canvas frame, in light then dark theme', async () => {
  await expectEditorParity(EMPTY_LIBRARY_STATE, true);
});

// covers: FCT-0043
test('shows a syntax error pixel for pixel like editeur.html?etat=syntaxe outside the canvas frame, in light then dark theme', async () => {
  await expectEditorParity(SYNTAX_ERROR_STATE, false);
});
