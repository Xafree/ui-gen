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
  removeButtonOf,
  sentProjects,
  undo,
  unmountHomes,
} from './recents-harness.js';

const PIVOT_REMOVED = 'Prototype Pivot retiré de la liste';
const NOVA_REMOVED = 'Atelier Nova retiré de la liste';

function asSent(...projects: readonly { path: string; name: string; openedAt: string }[]) {
  return projects.map(({ path, name, openedAt }) => ({ path, name, openedAt }));
}

beforeEach(async () => {
  await resetThemeEnvironment();
  await emulateColorScheme('light');
});

afterEach(async () => {
  unmountHomes();
  await resetThemeEnvironment();
});

describe('<ui-home> undoing a removal', () => {
  // covers: FCT-0023
  it('offers « Annuler » in the notification of a removal', async () => {
    const remove = vi.fn<RecentsApi['remove']>(() => Promise.resolve([kitHorizon()]));
    await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon(), prototypePivot()])), remove }));

    await removeButtonOf('Prototype Pivot').click();

    await expect.poll(() => shownToasts()).toContainEqual({
      tone: 'success',
      title: PIVOT_REMOVED,
      message: "Le dossier D:\\archives\\pivot-ui n'a pas été modifié.",
    });
    expect(await hasUndo(PIVOT_REMOVED)).toBe(true);
  });

  // covers: FCT-0023
  it('restores the removed project at its place and focuses it', async () => {
    const pivot = prototypePivot();
    const remove = vi.fn<RecentsApi['remove']>(() => Promise.resolve([kitHorizon(), lumenUi()]));
    const restore = vi.fn<RecentsApi['restore']>(() => Promise.resolve([kitHorizon(), lumenUi(), pivot]));
    await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon(), lumenUi(), pivot])), remove, restore }));
    await removeButtonOf('Prototype Pivot').click();
    await expect.poll(() => removeButtonOf('Prototype Pivot').query()).toBeNull();

    await undo(PIVOT_REMOVED);

    await expect.poll(() => restore.mock.calls.length).toBe(1);
    expect(sentProjects(restore)).toEqual(asSent(pivot));
    await expect.poll(() => removeButtonOf('Prototype Pivot').query()).not.toBeNull();
    await expect.poll(() => deepActiveElement() === removeButtonOf('Prototype Pivot').query()).toBe(true);
    const texts = recentLinkTexts();
    expect(texts[0]).toContain('Kit Horizon');
    expect(texts[1]).toContain('Lumen UI');
  });

  // covers: FCT-0023
  it('focuses the link of the restored project when its folder is available again', async () => {
    const nova = atelierNova(true);
    const remove = vi.fn<RecentsApi['remove']>(() => Promise.resolve([kitHorizon()]));
    const restore = vi.fn<RecentsApi['restore']>(() => Promise.resolve([kitHorizon(), nova]));
    await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon(), atelierNova(false)])), remove, restore }));
    await removeButtonOf('Atelier Nova').click();
    await expect.poll(() => removeButtonOf('Atelier Nova').query()).toBeNull();

    await undo(NOVA_REMOVED);

    await expect.poll(() => recentLinks().elements()).toHaveLength(2);
    await expect.poll(() => visibleText(deepActiveElement())).toContain('Atelier Nova');
    expect(deepActiveElement()).toBe(recentLinks().elements()[1]);
  });

  // covers: FCT-0023
  it('keeps its own copy in each notification, so undoing an older removal restores only that project', async () => {
    const pivot = prototypePivot();
    const nova = atelierNova(false);
    const remove = vi
      .fn<RecentsApi['remove']>()
      .mockResolvedValueOnce([kitHorizon(), nova])
      .mockResolvedValueOnce([kitHorizon()]);
    const restore = vi.fn<RecentsApi['restore']>(() => Promise.resolve([kitHorizon(), pivot]));
    await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon(), pivot, nova])), remove, restore }));
    await removeButtonOf('Prototype Pivot').click();
    await expect.poll(() => removeButtonOf('Prototype Pivot').query()).toBeNull();
    await removeButtonOf('Atelier Nova').click();
    await expect.poll(() => removeButtonOf('Atelier Nova').query()).toBeNull();

    await undo(PIVOT_REMOVED);

    await expect.poll(() => restore.mock.calls.length).toBe(1);
    expect(sentProjects(restore)).toEqual(asSent(pivot));
    await expect.poll(() => deepActiveElement() === removeButtonOf('Prototype Pivot').query()).toBe(true);
    expect(removeButtonOf('Atelier Nova').query()).toBeNull();
  });

  // covers: FCT-0023
  it('still undoes a removal after the list was cleared', async () => {
    const pivot = prototypePivot();
    const remove = vi.fn<RecentsApi['remove']>(() => Promise.resolve([kitHorizon()]));
    const clear = vi.fn<RecentsApi['clear']>(() => Promise.resolve([]));
    const restore = vi.fn<RecentsApi['restore']>(() => Promise.resolve([pivot]));
    await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon(), pivot])), remove, clear, restore }));
    await removeButtonOf('Prototype Pivot').click();
    await expect.poll(() => removeButtonOf('Prototype Pivot').query()).toBeNull();
    await page.getByRole('button', { name: 'Effacer la liste' }).click();
    await expect.element(page.getByText('Aucun projet récent', { exact: true })).toBeVisible();

    await undo(PIVOT_REMOVED);

    await expect.poll(() => restore.mock.calls.length).toBe(1);
    expect(sentProjects(restore)).toEqual(asSent(pivot));
    await expect.poll(() => deepActiveElement() === removeButtonOf('Prototype Pivot').query()).toBe(true);
    expect(recentLinks().elements()).toHaveLength(0);
  });

  // covers: FCT-0023
  it('focuses the first project of the list when the restored one is not returned', async () => {
    const remove = vi.fn<RecentsApi['remove']>(() => Promise.resolve([kitHorizon(), lumenUi()]));
    const restore = vi.fn<RecentsApi['restore']>(() => Promise.resolve([kitHorizon(), lumenUi()]));
    await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon(), lumenUi(), prototypePivot()])), remove, restore }));
    await removeButtonOf('Prototype Pivot').click();
    await expect.poll(() => removeButtonOf('Prototype Pivot').query()).toBeNull();

    await undo(PIVOT_REMOVED);

    await expect.poll(() => restore.mock.calls.length).toBe(1);
    await expect.poll(() => deepActiveElement() === recentLinks().elements()[0]).toBe(true);
  });

  // covers: FCT-0023
  it('focuses « Ouvrir un projet » when the list stays empty after the undo', async () => {
    const remove = vi.fn<RecentsApi['remove']>(() => Promise.resolve([]));
    const restore = vi.fn<RecentsApi['restore']>(() => Promise.resolve([]));
    await mountHome(makeUiGenApi({ list: listOf(listing([prototypePivot()])), remove, restore }));
    await removeButtonOf('Prototype Pivot').click();
    await expect.element(page.getByText('Aucun projet récent', { exact: true })).toBeVisible();

    await undo(PIVOT_REMOVED);

    await expect.poll(() => restore.mock.calls.length).toBe(1);
    await expect.poll(() => visibleText(deepActiveElement())).toBe('Ouvrir un projet');
    expect(deepActiveElement()?.closest('button')).not.toBeNull();
  });
});
