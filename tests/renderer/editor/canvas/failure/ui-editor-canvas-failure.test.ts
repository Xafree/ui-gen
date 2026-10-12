import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FAILING_HARNESS_URL, TALL_HARNESS_URL } from './failing-harness.js';
import { PRICING_CARD, fakeEditorApi, htmlElement, isShown, mountEditor, textOf } from '../../editor-fixtures.js';

let harnessUrl = FAILING_HARNESS_URL;

vi.mock('../../../../../src/modules/canvas/application/harness-url.js', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../../../../src/modules/canvas/application/harness-url.js')>()),
  buildHarnessUrl: () => harnessUrl,
}));

const mounted: Element[] = [];
const MAX_FRAME_HEIGHT = 4000;

beforeEach(() => {
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
  for (const element of mounted.splice(0)) element.remove();
});

describe('<ui-editor-canvas> render failure', () => {
  it('says the component does not render, and that the file was not modified', async () => {
    harnessUrl = FAILING_HARNESS_URL;
    await mountEditor(fakeEditorApi({ lastOpened: () => Promise.resolve(PRICING_CARD.path) }).api, mounted);

    await expect.poll(() => isShown('#render-failed')).toBe(true);

    expect(textOf('#render-failed .ui-empty__title')).toBe("Ce composant ne s'affiche pas");
    expect(textOf('#render-failed .ui-empty__text')).toBe(
      "Son code a levé une erreur au rendu. Le fichier n'a pas été modifié.",
    );
  });
});

describe('<ui-editor-canvas> frame height', () => {
  it('stays bounded when the rendered component asks for an enormous height', async () => {
    harnessUrl = TALL_HARNESS_URL;
    await mountEditor(fakeEditorApi({ lastOpened: () => Promise.resolve(PRICING_CARD.path) }).api, mounted);

    await expect.poll(() => htmlElement('#frame-body').offsetHeight).toBeGreaterThan(320);

    expect(htmlElement('#frame-body').offsetHeight).toBeLessThanOrEqual(MAX_FRAME_HEIGHT);
    expect(isShown('#render-failed')).toBe(false);
  });
});
