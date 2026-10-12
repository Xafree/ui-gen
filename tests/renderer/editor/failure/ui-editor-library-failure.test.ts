import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { userEvent } from 'vitest/browser';
import { KIT_HORIZON_LIBRARY, fakeEditorApi, htmlElement, isShown, isTextShown, mountEditor, openedMarks, optionNames } from '../editor-fixtures.js';

const TITLE = 'Bibliothèque illisible';
const TEXT = "Les composants n'ont pas pu être lus.";
const mounted: Element[] = [];

beforeEach(() => {
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
  for (const element of mounted.splice(0)) element.remove();
});

function listFailingTimes(failures: number) {
  let calls = 0;
  return {
    calls: () => calls,
    list: () => {
      calls += 1;
      return calls <= failures ? Promise.reject(new Error('EACCES')) : Promise.resolve([...KIT_HORIZON_LIBRARY]);
    },
  };
}

describe('<ui-editor> when the component library cannot be read', () => {
  it('shows an error state with a retry button instead of loading for ever', async () => {
    const { list } = listFailingTimes(1);

    await mountEditor(fakeEditorApi({ list }).api, mounted);

    await expect.poll(() => isTextShown(TITLE)).toBe(true);
    expect(isTextShown(TEXT)).toBe(true);
    expect(isTextShown('Réessayer')).toBe(true);
    expect(isShown('#library-failed')).toBe(true);
    expect(isTextShown('Aucun composant dans cette bibliothèque')).toBe(false);
  });

  it('reads the library again on retry, and shows the components once it works', async () => {
    const failing = listFailingTimes(1);
    const fake = fakeEditorApi({ list: failing.list });
    await mountEditor(fake.api, mounted);
    await expect.poll(() => isTextShown(TITLE)).toBe(true);

    await userEvent.click(htmlElement('#library-retry'));

    await expect.poll(() => isTextShown(TITLE)).toBe(false);
    expect(failing.calls()).toBe(2);
    await userEvent.click(htmlElement('#tab-comp'));
    expect(optionNames()).toEqual(['FeatureList', 'PlanBadge', 'PricingCard']);
    expect(openedMarks()).toHaveLength(1);
    expect(fake.opened).toHaveLength(1);
  });

  it('keeps the error state when the retry fails too', async () => {
    const failing = listFailingTimes(2);
    await mountEditor(fakeEditorApi({ list: failing.list }).api, mounted);
    await expect.poll(() => isTextShown(TITLE)).toBe(true);

    await userEvent.click(htmlElement('#library-retry'));

    await expect.poll(() => failing.calls()).toBe(2);
    await expect.poll(() => isTextShown(TITLE)).toBe(true);
  });

  it('shows the same state when the last opened component cannot be read', async () => {
    const fake = fakeEditorApi({ lastOpened: () => Promise.reject(new Error('EACCES')) });

    await mountEditor(fake.api, mounted);

    await expect.poll(() => isTextShown(TITLE)).toBe(true);
  });
});
