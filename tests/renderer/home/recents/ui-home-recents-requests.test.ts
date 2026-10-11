import { page } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { emulateColorScheme, resetThemeEnvironment } from '../../theme/theme-environment.js';
import { kitHorizon, listing, lumenUi, makeUiGenApi, prototypePivot, type RecentsApi } from '../home-fixtures.js';
import { deepActiveElement, shownToasts } from '../shadow-dom.js';
import { listOf, mountHome, recentLinks, removeButtonOf, undo, unmountHomes } from './recents-harness.js';

const PIVOT = 'Prototype Pivot';
const PIVOT_REMOVED = 'Prototype Pivot retiré de la liste';
const CLEARED = 'Liste des projets récents effacée';

const REFUSED = {
  tone: 'danger',
  title: 'Action impossible',
  message: "La demande a été refusée. Rien n'a été modifié.",
};

function clearButton() {
  return page.getByRole('button', { name: 'Effacer la liste' });
}

function plainCopy({ path, name, openedAt }: { path: string; name: string; openedAt: string }) {
  return { path, name, openedAt };
}

beforeEach(async () => {
  await resetThemeEnvironment();
  await emulateColorScheme('light');
  vi.spyOn(console, 'warn').mockReturnValue(undefined);
});

afterEach(async () => {
  unmountHomes();
  await resetThemeEnvironment();
  vi.restoreAllMocks();
});

describe('<ui-home> requests to restore the recent list', () => {
  // covers: FCT-0023
  it('sends the removed project as a plain copy, without its availability', async () => {
    const pivot = prototypePivot();
    const remove = vi.fn<RecentsApi['remove']>(() => Promise.resolve([kitHorizon()]));
    const restore = vi.fn<RecentsApi['restore']>(() => Promise.resolve([kitHorizon(), pivot]));
    await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon(), pivot])), remove, restore }));
    await removeButtonOf(PIVOT).click();
    await expect.poll(() => removeButtonOf(PIVOT).query()).toBeNull();

    await undo(PIVOT_REMOVED);

    await expect.poll(() => restore.mock.calls.length).toBe(1);
    expect(restore.mock.calls[0]?.[0]).toStrictEqual([plainCopy(pivot)]);
  });

  // covers: FCT-0022
  it('sends the cleared projects as plain copies, in the order of the list', async () => {
    const before = [kitHorizon(), lumenUi(), prototypePivot()];
    const clear = vi.fn<RecentsApi['clear']>(() => Promise.resolve([]));
    const restore = vi.fn<RecentsApi['restore']>(() => Promise.resolve(before));
    await mountHome(makeUiGenApi({ list: listOf(listing(before)), clear, restore }));
    await clearButton().click();
    await expect.element(page.getByText('Aucun projet récent', { exact: true })).toBeVisible();

    await undo(CLEARED);

    await expect.poll(() => restore.mock.calls.length).toBe(1);
    expect(restore.mock.calls[0]?.[0]).toStrictEqual(before.map(plainCopy));
  });
});

describe('<ui-home> refused requests to clear or restore the recent list', () => {
  // covers: FCT-0022
  it('says the request was refused, gives the focus back to « Effacer la liste » and keeps the list when clearing fails', async () => {
    const clear = vi.fn<RecentsApi['clear']>(() => Promise.reject(new Error('IPC channel closed')));
    await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon(), lumenUi()])), clear }));

    await clearButton().click();

    await expect.poll(() => shownToasts()).toContainEqual(REFUSED);
    expect(clear).toHaveBeenCalledTimes(1);
    await expect.poll(() => deepActiveElement() === clearButton().query()).toBe(true);
    expect(recentLinks().elements()).toHaveLength(2);
    expect(shownToasts()).toHaveLength(1);
  });

  // covers: FCT-0023
  it('says the request was refused, keeps the list and focuses its first project when « Annuler » fails', async () => {
    const remove = vi.fn<RecentsApi['remove']>(() => Promise.resolve([kitHorizon()]));
    const restore = vi.fn<RecentsApi['restore']>(() => Promise.reject(new Error('IPC message rejected (invalid-payload)')));
    await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon(), prototypePivot()])), remove, restore }));
    await removeButtonOf(PIVOT).click();
    await expect.poll(() => removeButtonOf(PIVOT).query()).toBeNull();

    await undo(PIVOT_REMOVED);

    await expect.poll(() => shownToasts()).toContainEqual(REFUSED);
    expect(restore).toHaveBeenCalledTimes(1);
    expect(removeButtonOf(PIVOT).query()).toBeNull();
    expect(recentLinks().elements()).toHaveLength(1);
    await expect.poll(() => deepActiveElement() === recentLinks().elements()[0]).toBe(true);
  });
});
