import { page } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { OpenProjectOutcome } from '../../../../src/modules/project/application/open/project-outcomes.js';
import type { RecentProjectView } from '../../../../src/modules/project/application/recents/recent-projects-use-cases.js';
import type { OpenFolderOutcome, UiGenApi } from '../../../../src/preload/ui-gen-api.js';
import { UiHome } from '../../../../src/renderer/home/ui-home.js';
import { PROJECT_OPENED } from '../../../../src/renderer/studio-events.js';
import { kitHorizon, kitHorizonProject, listing, type RecentsApi } from '../home-fixtures.js';
import { removeShownToasts, shownToasts } from '../shadow-dom.js';
import { apiWith, closedChannel, recordDetails, type ProjectApi } from './opening-fixtures.js';

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

function listOf(projects: RecentProjectView[]): RecentsApi['list'] {
  return vi.fn<RecentsApi['list']>(() => Promise.resolve(listing(projects)));
}

function kitHorizonLink() {
  return page.getByRole('region', { name: 'Projets récents' }).getByRole('link');
}

async function clickOpen(): Promise<void> {
  await page.getByRole('button', { name: 'Ouvrir un projet', exact: true }).first().click();
}

afterEach(() => {
  for (const home of mounted.splice(0)) {
    home.remove();
  }
  removeShownToasts();
  vi.restoreAllMocks();
});

describe('<ui-home> one opening at a time', () => {
  it('ignores « Ouvrir un projet » while a recent project is being opened', async () => {
    const opening = Promise.withResolvers<OpenProjectOutcome>();
    const open = vi.fn<RecentsApi['open']>(() => opening.promise);
    const openFolder = vi.fn<ProjectApi['openFolder']>(closedChannel);
    const home = await mountHome(apiWith({ openFolder }, { list: listOf([kitHorizon()]), open }));
    const opened = recordDetails(home, PROJECT_OPENED);
    await expect.element(kitHorizonLink()).toBeVisible();
    await kitHorizonLink().click();
    await expect.poll(() => open.mock.calls).toHaveLength(1);

    await clickOpen();
    opening.resolve({ kind: 'opened', project: kitHorizonProject() });

    await expect.poll(() => opened).toHaveLength(1);
    expect(openFolder).not.toHaveBeenCalled();
    expect(shownToasts()).toEqual([]);
  });

  it('ignores a recent project while the folder to open is being read', async () => {
    const choosing = Promise.withResolvers<OpenFolderOutcome>();
    const openFolder = vi.fn<ProjectApi['openFolder']>(() => choosing.promise);
    const open = vi.fn<RecentsApi['open']>(closedChannel);
    const home = await mountHome(apiWith({ openFolder }, { list: listOf([kitHorizon()]), open }));
    const opened = recordDetails(home, PROJECT_OPENED);
    await expect.element(kitHorizonLink()).toBeVisible();
    await clickOpen();
    await expect.poll(() => openFolder.mock.calls).toHaveLength(1);

    await kitHorizonLink().click();
    choosing.resolve({ kind: 'opened', project: kitHorizonProject() });

    await expect.poll(() => opened).toHaveLength(1);
    expect(open).not.toHaveBeenCalled();
    expect(shownToasts()).toEqual([]);
  });

  it('ignores « Ouvrir un projet » while the creation dialog is being prepared', async () => {
    const location = Promise.withResolvers<string>();
    const defaultLocation = vi.fn<ProjectApi['defaultLocation']>(() => location.promise);
    const openFolder = vi.fn<ProjectApi['openFolder']>(closedChannel);
    await mountHome(apiWith({ defaultLocation, openFolder }, { list: listOf([]) }));
    await page.getByRole('button', { name: 'Créer un projet', exact: true }).first().click();
    await expect.poll(() => defaultLocation.mock.calls).toHaveLength(1);

    await clickOpen();
    location.resolve('E:\\projets');

    await expect.element(page.getByRole('dialog', { name: 'Créer un projet' })).toBeVisible();
    expect(openFolder).not.toHaveBeenCalled();
    expect(shownToasts()).toEqual([]);
  });
});
