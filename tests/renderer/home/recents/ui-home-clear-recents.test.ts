import { page } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { emulateColorScheme, resetThemeEnvironment } from '../../theme/theme-environment.js';
import {
  atelierNova,
  kitHorizon,
  listing,
  lumenUi,
  makeUiGenApi,
  prototypePivot,
  type RecentsApi,
} from '../home-fixtures.js';
import { deepActiveElement, shownToasts, visibleText } from '../shadow-dom.js';
import {
  hasUndo,
  listOf,
  mountHome,
  recentLinkTexts,
  recentLinks,
  recentSection,
  removeButtonOf,
  sentProjects,
  undo,
  unmountHomes,
} from './recents-harness.js';

const CLEARED_TITLE = 'Liste des projets récents effacée';

function clearButton() {
  return page.getByRole('button', { name: 'Effacer la liste' });
}

beforeEach(async () => {
  await resetThemeEnvironment();
  await emulateColorScheme('light');
});

afterEach(async () => {
  unmountHomes();
  await resetThemeEnvironment();
});

describe('<ui-home> clearing the recent list', () => {
  // covers: FCT-0022
  it('clears the list through the api, shows the empty state and focuses « Créer un projet » of the empty state', async () => {
    const clear = vi.fn<RecentsApi['clear']>(() => Promise.resolve([]));
    await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon(), lumenUi(), prototypePivot()])), clear }));

    await clearButton().click();

    expect(clear).toHaveBeenCalledTimes(1);
    await expect.element(page.getByText('Aucun projet récent', { exact: true })).toBeVisible();
    expect(recentLinks().elements()).toHaveLength(0);
    expect(removeButtonOf('Prototype Pivot').query()).toBeNull();
    expect(clearButton().query()).toBeNull();
    await expect.poll(() => deepActiveElement()?.id).toBe('empty-create');
    expect(visibleText(deepActiveElement())).toBe('Créer un projet');
  });

  // covers: FCT-0022
  it('confirms that no folder was modified, with « Annuler » in the notification', async () => {
    const clear = vi.fn<RecentsApi['clear']>(() => Promise.resolve([]));
    await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon(), lumenUi()])), clear }));

    await clearButton().click();

    await expect.poll(() => shownToasts()).toContainEqual({
      tone: 'success',
      title: CLEARED_TITLE,
      message: "Aucun dossier n'a été modifié.",
    });
    expect(await hasUndo(CLEARED_TITLE)).toBe(true);
  });

  // covers: FCT-0022
  it('restores the projects listed when the list was cleared, then focuses the first one', async () => {
    const before = [kitHorizon(), lumenUi(), prototypePivot()];
    const clear = vi.fn<RecentsApi['clear']>(() => Promise.resolve([]));
    const restore = vi.fn<RecentsApi['restore']>(() => Promise.resolve(before));
    await mountHome(makeUiGenApi({ list: listOf(listing(before)), clear, restore }));
    await clearButton().click();
    await expect.element(page.getByText('Aucun projet récent', { exact: true })).toBeVisible();

    await undo(CLEARED_TITLE);

    await expect.poll(() => restore.mock.calls.length).toBe(1);
    expect(sentProjects(restore)).toEqual(
      before.map(({ path, name, openedAt }) => ({ path, name, openedAt })),
    );
    await expect.poll(() => recentLinks().elements()).toHaveLength(2);
    const texts = recentLinkTexts();
    expect(texts[0]).toContain('Kit Horizon');
    expect(texts[1]).toContain('Lumen UI');
    expect(removeButtonOf('Prototype Pivot').query()).not.toBeNull();
    await expect.element(clearButton()).toBeVisible();
    await expect.poll(() => deepActiveElement() === recentLinks().elements()[0]).toBe(true);
  });

  // covers: FCT-0022
  it('shows the list returned by the api after the undo, without duplicate', async () => {
    const clear = vi.fn<RecentsApi['clear']>(() => Promise.resolve([]));
    const restore = vi.fn<RecentsApi['restore']>(() => Promise.resolve([kitHorizon(), lumenUi()]));
    await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon(), lumenUi()])), clear, restore }));
    await clearButton().click();
    await expect.element(page.getByText('Aucun projet récent', { exact: true })).toBeVisible();

    await undo(CLEARED_TITLE);

    await expect.poll(() => recentLinks().elements()).toHaveLength(2);
    await expect.element(recentSection().getByText('2', { exact: true })).toBeVisible();
  });

  // covers: FCT-0022
  it('focuses the remove button of the first restored project when its folder is missing', async () => {
    const before = [prototypePivot(), atelierNova(false)];
    const clear = vi.fn<RecentsApi['clear']>(() => Promise.resolve([]));
    const restore = vi.fn<RecentsApi['restore']>(() => Promise.resolve(before));
    await mountHome(makeUiGenApi({ list: listOf(listing(before)), clear, restore }));
    await clearButton().click();
    await expect.element(page.getByText('Aucun projet récent', { exact: true })).toBeVisible();

    await undo(CLEARED_TITLE);

    await expect.poll(() => deepActiveElement() === removeButtonOf('Prototype Pivot').query()).toBe(true);
  });

  // covers: FCT-0022
  it('focuses « Ouvrir un projet » when the api restores no project', async () => {
    const clear = vi.fn<RecentsApi['clear']>(() => Promise.resolve([]));
    const restore = vi.fn<RecentsApi['restore']>(() => Promise.resolve([]));
    await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon()])), clear, restore }));
    await clearButton().click();
    await expect.poll(() => deepActiveElement()?.id).toBe('empty-create');

    await undo(CLEARED_TITLE);

    await expect.poll(() => restore.mock.calls.length).toBe(1);
    await expect.poll(() => visibleText(deepActiveElement())).toBe('Ouvrir un projet');
    expect(deepActiveElement()?.closest('button')).not.toBeNull();
  });
});
