import { join } from 'node:path';
import { expect, test, type ElectronApplication, type Page } from '@playwright/test';
import { makeTempFolder, removeFolder, writeFiles } from '../project/studio-app';
import { openRecentProject } from '../editor/canvas/canvas-project';
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

const DEFAULT_STATE: MockupState = { name: 'default', query: {}, ready: expectComponentOnCanvas };

const NO_PROBLEMS_STATE: MockupState = {
  name: 'no-problems',
  query: { problemes: '0' },
  customize: (project) =>
    rewritePricingCard(project, (source) =>
      source.replace(FORMAT_PRICE_IMPORT, '').replace(FORMAT_PRICE_CALL, () => '${this.price}'),
    ),
  ready: expectComponentOnCanvas,
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

async function captureMockupEditor(app: ElectronApplication, state: MockupState, theme: Theme): Promise<Buffer> {
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
    return await settledCapture(reference.page, reference.page.locator(FRAME_BODY));
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
      const studio = await settledCapture(window, window.locator(FRAME_BODY));
      const mockup = await captureMockupEditor(app, state, theme);
      const name = `editor-${state.name}-${theme}`;
      try {
        await expectIdenticalCaptures(app, test.info(), name, mockup, studio);
      } catch (error) {
        failures.push(firstLine(error));
      }
      const next = THEMES[index + 1];
      if (next !== undefined) await chooseStudioTheme(window, next);
    });
  }

  expect(failures, 'every state and theme must match the mockup outside the canvas frame').toEqual([]);
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
