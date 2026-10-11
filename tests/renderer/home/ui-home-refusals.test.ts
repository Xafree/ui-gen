import { page } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { OpenProjectOutcome } from '../../../src/modules/project/application/open/project-outcomes.js';
import type { RecentProjectsListing } from '../../../src/modules/project/application/recents/recent-projects-use-cases.js';
import type { UiGenApi } from '../../../src/preload/ui-gen-api.js';
import { UiHome } from '../../../src/renderer/home/ui-home.js';
import { PROJECT_OPENED } from '../../../src/renderer/studio-events.js';
import {
  kitHorizon,
  kitHorizonProject,
  listing,
  lumenUi,
  makeUiGenApi,
  prototypePivot,
  type RecentsApi,
} from './home-fixtures.js';
import { deepActiveElement, removeShownToasts, shownToasts, visibleText } from './shadow-dom.js';

const REFUSED = {
  tone: 'danger',
  title: 'Action impossible',
  message: "La demande a été refusée. Rien n'a été modifié.",
};

const mounted: UiHome[] = [];

async function mountHome(api: UiGenApi): Promise<UiHome> {
  const home = new UiHome();
  home.api = api;
  home.notice = null;
  document.body.append(home);
  mounted.push(home);
  await home.updateComplete;
  return home;
}

function listOf(result: RecentProjectsListing): RecentsApi['list'] {
  return vi.fn<RecentsApi['list']>(() => Promise.resolve(result));
}

function recentSection() {
  return page.getByRole('region', { name: 'Projets récents' });
}

function recentLinks() {
  return recentSection().getByRole('link');
}

function recentLinkTexts(): string[] {
  return recentLinks()
    .elements()
    .map((link) => visibleText(link));
}

function removeButtons() {
  return page.getByRole('button', { name: /^Retirer de la liste/ });
}

function countProjectOpened(home: UiHome): () => number {
  let count = 0;
  home.addEventListener(PROJECT_OPENED, () => {
    count += 1;
  });
  return () => count;
}

afterEach(() => {
  for (const home of mounted.splice(0)) {
    home.remove();
  }
  removeShownToasts();
});

describe('<ui-home> refused requests', () => {
  // covers: FCT-0001
  it('says the request was refused, gives the focus back to the link and keeps the list when opening a recent project fails', async () => {
    const open = vi.fn<RecentsApi['open']>(() => Promise.reject(new Error('IPC channel closed')));
    const home = await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon(), lumenUi()])), open }));
    const opened = countProjectOpened(home);
    await expect.element(recentLinks().nth(1)).toBeVisible();

    await recentLinks().nth(1).click();

    await expect.poll(() => shownToasts()).toContainEqual(REFUSED);
    expect(open).toHaveBeenCalledTimes(1);
    expect(open).toHaveBeenCalledWith('E:\\projets\\lumen-ui');
    await expect.poll(() => deepActiveElement() === recentLinks().elements()[1]).toBe(true);
    await expect.poll(() => recentLinks().elements()[1]?.getAttribute('aria-busy')).not.toBe('true');
    expect(recentLinks().elements()).toHaveLength(2);
    const texts = recentLinkTexts();
    expect(texts[0]).toContain('Kit Horizon');
    expect(texts[1]).toContain('Lumen UI');
    expect(opened()).toBe(0);
    expect(shownToasts()).toHaveLength(1);
  });

  // covers: FCT-0011
  it('says the request was refused, gives the focus back to the remove button and keeps the list when removing fails', async () => {
    const remove = vi.fn<RecentsApi['remove']>(() => Promise.reject(new Error('IPC channel closed')));
    await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon(), prototypePivot()])), remove }));
    const removeButton = page.getByRole('button', { name: 'Retirer de la liste : Prototype Pivot' });
    await expect.element(removeButton).toBeVisible();

    await removeButton.click();

    await expect.poll(() => shownToasts()).toContainEqual(REFUSED);
    expect(remove).toHaveBeenCalledTimes(1);
    expect(remove).toHaveBeenCalledWith('D:\\archives\\pivot-ui');
    await expect.poll(() => deepActiveElement() === removeButton.query()).toBe(true);
    expect(removeButtons().elements()).toHaveLength(1);
    expect(recentSection().getByText('Prototype Pivot').elements()).not.toHaveLength(0);
    expect(recentLinks().elements()).toHaveLength(1);
    expect(recentLinkTexts()[0]).toContain('Kit Horizon');
    expect(shownToasts()).toHaveLength(1);
  });
});

describe('<ui-home> repeated gestures', () => {
  // covers: FCT-0001
  it('sends a single open request when a recent project is double clicked while the first request is pending', async () => {
    const opening = Promise.withResolvers<OpenProjectOutcome>();
    const open = vi.fn<RecentsApi['open']>(() => opening.promise);
    const home = await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon(), lumenUi()])), open }));
    const opened = countProjectOpened(home);
    await expect.element(recentLinks().first()).toBeVisible();

    await recentLinks().first().dblClick();
    await recentLinks().first().click();

    expect(open).toHaveBeenCalledTimes(1);
    expect(open).toHaveBeenCalledWith('E:\\projets\\kit-horizon');

    opening.resolve({ kind: 'opened', project: kitHorizonProject() });

    await expect.poll(() => opened()).toBe(1);
    expect(open).toHaveBeenCalledTimes(1);
  });
});
