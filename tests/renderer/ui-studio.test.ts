import { page } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { UiGenApi } from '../../src/preload/ui-gen-api.js';
import type { ToastRequest } from '../../src/renderer/motion/toast.js';
import { PROJECT_OPENED, type ProjectOpenedDetail } from '../../src/renderer/studio-events.js';
import { UiStudio } from '../../src/renderer/ui-studio.js';
import { kitHorizon, kitHorizonProject, listing, lumenUi, makeUiGenApi, type RecentsApi } from './home/home-fixtures.js';
import { deepQueryAll, removeShownToasts } from './home/shadow-dom.js';

const mounted: UiStudio[] = [];

async function mountStudio(api: UiGenApi | null): Promise<UiStudio> {
  const studio = new UiStudio();
  if (api !== null) studio.api = api;
  document.body.append(studio);
  mounted.push(studio);
  await studio.updateComplete;
  return studio;
}

function listOfTwo(): RecentsApi['list'] {
  return vi.fn<RecentsApi['list']>(() => Promise.resolve(listing([kitHorizon(), lumenUi()])));
}

function shownHome(): Element | undefined {
  return deepQueryAll('ui-home')[0];
}

function shownShell(): Element | undefined {
  return deepQueryAll('ui-shell')[0];
}

afterEach(() => {
  for (const studio of mounted.splice(0)) {
    studio.remove();
  }
  removeShownToasts();
  Reflect.deleteProperty(window, 'uiGen');
});

describe('<ui-studio>', () => {
  // covers: FCT-0001
  it('shows the home screen while no project is open', async () => {
    const list = listOfTwo();
    await mountStudio(makeUiGenApi({ list }));

    await expect.poll(() => shownHome()).toBeDefined();
    await expect.element(page.getByRole('heading', { level: 1, name: 'UI-GEN' })).toBeVisible();
    await expect.element(page.getByRole('button', { name: 'Créer un projet', exact: true })).toBeVisible();
    await expect.element(page.getByRole('button', { name: 'Ouvrir un projet', exact: true })).toBeVisible();
    expect(shownShell()).toBeUndefined();
    expect(list).toHaveBeenCalledTimes(1);
  });

  // covers: FCT-0001
  it('uses the window bridge when no api is given', async () => {
    const list = listOfTwo();
    Object.defineProperty(window, 'uiGen', { value: makeUiGenApi({ list }), configurable: true });
    await mountStudio(null);

    await expect.poll(() => list).toHaveBeenCalledTimes(1);
    await expect.element(page.getByRole('heading', { level: 2, name: 'Projets récents' })).toBeVisible();
  });

  it('replaces the home screen with the shell of the project opened from the recent list', async () => {
    const open = vi.fn<RecentsApi['open']>(() => Promise.resolve({ kind: 'opened', project: kitHorizonProject() }));
    await mountStudio(makeUiGenApi({ list: listOfTwo(), open }));
    const recentLinks = page.getByRole('region', { name: 'Projets récents' }).getByRole('link');
    await expect.element(recentLinks.first()).toBeVisible();

    await recentLinks.first().click();

    await expect.poll(() => shownShell()).toBeDefined();
    const shell = shownShell();
    expect(Reflect.get(shell ?? {}, 'project')).toEqual(kitHorizonProject());
    expect(Reflect.get(shell ?? {}, 'notice')).toBeNull();
    expect(shownHome()).toBeUndefined();
  });

  it('hands the arrival notice of project-opened to the shell', async () => {
    await mountStudio(makeUiGenApi({ list: listOfTwo() }));
    await expect.poll(() => shownHome()).toBeDefined();
    const notice: ToastRequest = {
      tone: 'success',
      title: 'Atlas ouvert avec UI-GEN',
      message: 'uigen.json a été ajouté à la racine du dossier.',
    };
    const detail: ProjectOpenedDetail = { project: kitHorizonProject(), notice };

    shownHome()?.dispatchEvent(
      new CustomEvent<ProjectOpenedDetail>(PROJECT_OPENED, { detail, bubbles: true, composed: true }),
    );

    await expect.poll(() => shownShell()).toBeDefined();
    const shell = shownShell();
    expect(Reflect.get(shell ?? {}, 'project')).toEqual(kitHorizonProject());
    expect(Reflect.get(shell ?? {}, 'notice')).toEqual(notice);
    expect(shownHome()).toBeUndefined();
  });
});
