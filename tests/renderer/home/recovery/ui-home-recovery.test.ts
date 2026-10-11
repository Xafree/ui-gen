import { page } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { OpenProjectOutcome } from '../../../../src/modules/project/application/open/project-outcomes.js';
import type { UiGenApi } from '../../../../src/preload/ui-gen-api.js';
import { UiHome } from '../../../../src/renderer/home/ui-home.js';
import { PROJECT_OPENED } from '../../../../src/renderer/studio-events.js';
import { kitHorizon, kitHorizonProject, listing, lumenUi, makeUiGenApi, type RecentsApi } from '../home-fixtures.js';
import { deepActiveElement, removeShownToasts, shownToasts, visibleText } from '../shadow-dom.js';

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

function recentLinks() {
  return page.getByRole('region', { name: 'Projets récents' }).getByRole('link');
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

describe('<ui-home> recovering from a refused request', () => {
  // covers: FCT-0011
  it('keeps the list and says the request was refused when the list cannot be reloaded after a missing folder', async () => {
    const list = vi
      .fn<RecentsApi['list']>()
      .mockResolvedValueOnce(listing([kitHorizon(), lumenUi()]))
      .mockRejectedValueOnce(new Error('IPC channel closed'));
    const open = vi.fn<RecentsApi['open']>(() =>
      Promise.resolve({ kind: 'missing', path: 'E:\\projets\\lumen-ui' } satisfies OpenProjectOutcome),
    );
    const home = await mountHome(makeUiGenApi({ list, open }));
    const opened = countProjectOpened(home);
    await expect.element(recentLinks().nth(1)).toBeVisible();

    await recentLinks().nth(1).click();

    await expect.poll(() => shownToasts()).toContainEqual(REFUSED);
    expect(list).toHaveBeenCalledTimes(2);
    await expect.poll(() => deepActiveElement() === recentLinks().elements()[1]).toBe(true);
    await expect.poll(() => recentLinks().elements()[1]?.getAttribute('aria-busy')).not.toBe('true');
    const texts = recentLinks()
      .elements()
      .map((link) => visibleText(link));
    expect(texts).toHaveLength(2);
    expect(texts[0]).toContain('Kit Horizon');
    expect(texts[1]).toContain('Lumen UI');
    expect(opened()).toBe(0);
    expect(shownToasts()).toHaveLength(1);
  });

  // covers: FCT-0001
  it('sends a new open request once the refused one has ended', async () => {
    const open = vi
      .fn<RecentsApi['open']>()
      .mockRejectedValueOnce(new Error('IPC channel closed'))
      .mockResolvedValueOnce({ kind: 'opened', project: kitHorizonProject() });
    const list = vi.fn<RecentsApi['list']>(() => Promise.resolve(listing([kitHorizon(), lumenUi()])));
    const home = await mountHome(makeUiGenApi({ list, open }));
    const opened = countProjectOpened(home);
    await expect.element(recentLinks().first()).toBeVisible();
    await recentLinks().first().click();
    await expect.poll(() => shownToasts()).toContainEqual(REFUSED);

    await recentLinks().first().click();

    await expect.poll(() => opened()).toBe(1);
    expect(open).toHaveBeenCalledTimes(2);
    expect(open).toHaveBeenLastCalledWith('E:\\projets\\kit-horizon');
  });
});
