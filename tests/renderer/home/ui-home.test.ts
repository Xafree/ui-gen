import { page } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { OpenProjectOutcome } from '../../../src/modules/project/application/open/project-outcomes.js';
import type { RecentProjectsListing, RecentProjectView } from '../../../src/modules/project/application/recents/recent-projects-use-cases.js';
import type { UiGenApi } from '../../../src/preload/ui-gen-api.js';
import { UiHome } from '../../../src/renderer/home/ui-home.js';
import type { ToastRequest } from '../../../src/renderer/motion/toast.js';
import { PROJECT_OPENED, type ProjectOpenedDetail } from '../../../src/renderer/studio-events.js';
import { emulateColorScheme, resetThemeEnvironment } from '../theme/theme-environment.js';
import {
  atelierNova,
  composantsVerdure,
  kitHorizon,
  kitHorizonProject,
  listing,
  lumenUi,
  makeUiGenApi,
  prototypePivot,
  type RecentsApi,
} from './home-fixtures.js';
import { deepActiveElement, deepQueryAll, removeShownToasts, shownToasts, visibleText } from './shadow-dom.js';

const mounted: UiHome[] = [];

async function mountHome(api: UiGenApi, notice: ToastRequest | null = null): Promise<UiHome> {
  const home = new UiHome();
  home.api = api;
  home.notice = notice;
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

function recordProjectOpened(home: UiHome): unknown[] {
  const details: unknown[] = [];
  home.addEventListener(PROJECT_OPENED, (event: Event) => {
    if (event instanceof CustomEvent) {
      const detail: unknown = event.detail;
      details.push(detail);
    }
  });
  return details;
}

beforeEach(async () => {
  await resetThemeEnvironment();
  await emulateColorScheme('light');
});

afterEach(async () => {
  for (const home of mounted.splice(0)) {
    home.remove();
  }
  removeShownToasts();
  await resetThemeEnvironment();
});

describe('<ui-home> start screen', () => {
  // covers: FCT-0001
  it('shows the bare top bar, the start actions and the recent projects section', async () => {
    const list = listOf(listing([kitHorizon(), lumenUi(), composantsVerdure(), prototypePivot()]));
    await mountHome(makeUiGenApi({ list }));

    await expect.element(page.getByRole('heading', { level: 1, name: 'UI-GEN' })).toBeVisible();
    await expect.element(page.getByRole('button', { name: 'Créer un projet', exact: true })).toBeVisible();
    await expect.element(page.getByRole('button', { name: 'Ouvrir un projet', exact: true })).toBeVisible();
    await expect.element(page.getByRole('heading', { level: 2, name: 'Projets récents' })).toBeVisible();
    await expect.element(page.getByText('Aucun projet ouvert', { exact: true })).toBeVisible();
    await expect.element(page.getByRole('img', { name: 'Compte local' })).toBeVisible();
    await expect.element(page.getByRole('button', { name: 'Passer au thème sombre' })).toBeVisible();
    await expect.element(recentLinks().first()).toBeVisible();
    expect(document.title).toBe('Accueil, UI-GEN');
    expect(list).toHaveBeenCalledTimes(1);
    expect(shownToasts()).toEqual([]);
  });

  // covers: FCT-0001
  it('shows the empty state, without counter nor « Effacer la liste », when no project was opened yet', async () => {
    const home = await mountHome(makeUiGenApi({ list: listOf(listing([])) }));

    await expect.element(page.getByText('Aucun projet récent', { exact: true })).toBeVisible();
    await expect
      .element(
        page.getByText("Les projets que vous créez ou ouvrez s'affichent ici, du plus récent au plus ancien.", {
          exact: true,
        }),
      )
      .toBeVisible();
    expect(page.getByRole('button', { name: 'Créer un projet', exact: true }).elements()).toHaveLength(2);
    expect(page.getByRole('button', { name: 'Ouvrir un projet', exact: true }).elements()).toHaveLength(2);
    expect(page.getByRole('button', { name: 'Effacer la liste' }).query()).toBeNull();
    expect(recentLinks().elements()).toHaveLength(0);
    const visibleCounters = deepQueryAll('.ui-count', home.shadowRoot ?? document).filter((counter) =>
      counter.checkVisibility(),
    );
    expect(visibleCounters).toEqual([]);
    expect(shownToasts()).toEqual([]);
  });

  it('shows the counter and « Effacer la liste » when the list is not empty', async () => {
    await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon(), lumenUi(), prototypePivot()])) }));

    await expect.element(recentSection().getByText('3', { exact: true })).toBeVisible();
    await expect.element(page.getByRole('button', { name: 'Effacer la liste' })).toBeVisible();
  });
});

describe('<ui-home> recent projects', () => {
  // covers: FCT-0010
  it('lists available projects from the most recent to the oldest, each link with its name, path and relative date', async () => {
    await mountHome(
      makeUiGenApi({ list: listOf(listing([kitHorizon(), lumenUi(), composantsVerdure(), prototypePivot()])) }),
    );

    await expect.element(recentLinks().first()).toBeVisible();
    const texts = recentLinkTexts();
    expect(texts).toHaveLength(3);
    expect(texts[0]).toContain('Kit Horizon');
    expect(texts[0]).toContain('E:\\projets\\kit-horizon');
    expect(texts[0]).toContain('il y a 6 minutes');
    expect(texts[1]).toContain('Lumen UI');
    expect(texts[1]).toContain('E:\\projets\\lumen-ui');
    expect(texts[1]).toContain('hier');
    expect(texts[2]).toContain('Composants Verdure');
    expect(texts[2]).toContain('E:\\clients\\verdure\\composants');
    expect(texts[2]).toContain('il y a 4 jours');
  });

  // covers: FCT-0010
  it('keeps the ten projects of the listing in their order', async () => {
    const day = 24 * 60 * 60_000;
    const projects: RecentProjectView[] = Array.from({ length: 10 }, (_, index) => {
      const rank = String(index + 1).padStart(2, '0');
      return {
        path: `E:\\projets\\projet-${rank}`,
        name: `Projet ${rank}`,
        openedAt: new Date(Date.now() - (index + 1) * day).toISOString(),
        available: true,
      };
    });
    await mountHome(makeUiGenApi({ list: listOf(listing(projects)) }));

    await expect.element(recentLinks().first()).toBeVisible();
    const names = recentLinkTexts().map((text) => /Projet \d\d/.exec(text)?.[0]);
    expect(names).toEqual([
      'Projet 01',
      'Projet 02',
      'Projet 03',
      'Projet 04',
      'Projet 05',
      'Projet 06',
      'Projet 07',
      'Projet 08',
      'Projet 09',
      'Projet 10',
    ]);
  });

  // covers: FCT-0011
  it('marks a project whose folder is missing as « Introuvable », with its path and no link', async () => {
    await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon(), prototypePivot()])) }));

    await expect.element(recentSection().getByText('Introuvable', { exact: true })).toBeVisible();
    await expect.element(recentSection().getByText('Prototype Pivot', { exact: true })).toBeVisible();
    await expect.element(recentSection().getByText('D:\\archives\\pivot-ui', { exact: true })).toBeVisible();
    await expect.element(page.getByRole('button', { name: 'Retirer de la liste : Prototype Pivot' })).toBeVisible();
    expect(recentLinks().elements()).toHaveLength(1);
    expect(recentLinkTexts().some((text) => text.includes('Prototype Pivot'))).toBe(false);
    expect(page.getByRole('button', { name: /^Retirer de la liste/ }).elements()).toHaveLength(1);
  });

  // covers: FCT-0011
  it('removes a missing project through the api, focuses the first row and confirms that no folder changed', async () => {
    const remove = vi.fn<RecentsApi['remove']>(() => Promise.resolve([kitHorizon(), lumenUi()]));
    await mountHome(
      makeUiGenApi({ list: listOf(listing([kitHorizon(), lumenUi(), prototypePivot()])), remove }),
    );

    await page.getByRole('button', { name: 'Retirer de la liste : Prototype Pivot' }).click();

    expect(remove).toHaveBeenCalledTimes(1);
    expect(remove).toHaveBeenCalledWith('D:\\archives\\pivot-ui');
    await expect.poll(() => page.getByRole('button', { name: /^Retirer de la liste/ }).elements()).toHaveLength(0);
    await expect.poll(() => recentSection().getByText('Prototype Pivot').elements()).toHaveLength(0);
    await expect.poll(() => recentLinks().elements()).toHaveLength(2);
    await expect.poll(() => deepActiveElement() === recentLinks().elements()[0]).toBe(true);
    await expect.poll(() => visibleText(deepActiveElement())).toContain('Kit Horizon');
    await expect.poll(() => shownToasts()).toContainEqual({
      tone: 'success',
      title: 'Prototype Pivot retiré de la liste',
      message: "Le dossier D:\\archives\\pivot-ui n'a pas été modifié.",
    });
  });

  // covers: FCT-0011
  it('shows the list returned by the api after a removal', async () => {
    const remove = vi.fn<RecentsApi['remove']>(() => Promise.resolve([kitHorizon(), atelierNova(true)]));
    await mountHome(
      makeUiGenApi({ list: listOf(listing([kitHorizon(), prototypePivot(), atelierNova(false)])), remove }),
    );

    await page.getByRole('button', { name: 'Retirer de la liste : Prototype Pivot' }).click();

    await expect.poll(() => recentLinks().elements()).toHaveLength(2);
    const texts = recentLinkTexts();
    expect(texts[0]).toContain('Kit Horizon');
    expect(texts[1]).toContain('Atelier Nova');
    expect(page.getByRole('button', { name: /^Retirer de la liste/ }).elements()).toHaveLength(0);
  });

  it('focuses the remove button of the first row when that project is missing too', async () => {
    const remove = vi.fn<RecentsApi['remove']>(() => Promise.resolve([atelierNova(false)]));
    await mountHome(makeUiGenApi({ list: listOf(listing([prototypePivot(), atelierNova(false)])), remove }));

    await page.getByRole('button', { name: 'Retirer de la liste : Prototype Pivot' }).click();

    await expect
      .poll(
        () => deepActiveElement() === page.getByRole('button', { name: 'Retirer de la liste : Atelier Nova' }).query(),
      )
      .toBe(true);
  });

  it('focuses « Ouvrir un projet » and shows the empty state when the last project is removed', async () => {
    const remove = vi.fn<RecentsApi['remove']>(() => Promise.resolve([]));
    await mountHome(makeUiGenApi({ list: listOf(listing([prototypePivot()])), remove }));

    await page.getByRole('button', { name: 'Retirer de la liste : Prototype Pivot' }).click();

    await expect.element(page.getByText('Aucun projet récent', { exact: true })).toBeVisible();
    await expect.poll(() => visibleText(deepActiveElement())).toBe('Ouvrir un projet');
    expect(deepActiveElement()?.closest('button')).not.toBeNull();
    await expect.poll(() => shownToasts()).toContainEqual({
      tone: 'success',
      title: 'Prototype Pivot retiré de la liste',
      message: "Le dossier D:\\archives\\pivot-ui n'a pas été modifié.",
    });
  });
});

describe('<ui-home> notices', () => {
  it('warns that the recent projects could not be read, and shows the empty state', async () => {
    await mountHome(makeUiGenApi({ list: listOf(listing([], true)) }));

    await expect.poll(() => shownToasts()).toContainEqual({
      tone: 'warning',
      title: 'Projets récents illisibles',
      message: "La liste n'a pas pu être lue. Aucun dossier n'a été modifié.",
    });
    await expect.element(page.getByText('Aucun projet récent', { exact: true })).toBeVisible();
  });

  it('gives the same warning when the recent projects cannot be listed at all', async () => {
    const list = vi.fn<RecentsApi['list']>(() => Promise.reject(new Error('IPC channel closed')));
    await mountHome(makeUiGenApi({ list }));

    await expect.poll(() => shownToasts()).toContainEqual({
      tone: 'warning',
      title: 'Projets récents illisibles',
      message: "La liste n'a pas pu être lue. Aucun dossier n'a été modifié.",
    });
  });

  it('shows the notice it was given on arrival', async () => {
    const notice: ToastRequest = {
      tone: 'info',
      title: 'Kit Horizon fermé',
      message: 'Le projet a été fermé. Aucun dossier n’a été modifié.',
    };
    await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon()])) }), notice);

    await expect.poll(() => shownToasts()).toContainEqual({
      tone: 'info',
      title: 'Kit Horizon fermé',
      message: 'Le projet a été fermé. Aucun dossier n’a été modifié.',
    });
  });
});

describe('<ui-home> opening a recent project', () => {
  it('opens an available project from its link, busy while opening, then announces project-opened without notice', async () => {
    const opening = Promise.withResolvers<OpenProjectOutcome>();
    const open = vi.fn<RecentsApi['open']>(() => opening.promise);
    const home = await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon(), lumenUi()])), open }));
    const announced = recordProjectOpened(home);

    await recentLinks().first().click();

    expect(open).toHaveBeenCalledTimes(1);
    expect(open).toHaveBeenCalledWith('E:\\projets\\kit-horizon');
    await expect.poll(() => recentLinks().elements()[0]?.getAttribute('aria-busy')).toBe('true');
    expect(announced).toEqual([]);

    opening.resolve({ kind: 'opened', project: kitHorizonProject() });

    await expect.poll(() => announced).toHaveLength(1);
    const expected: ProjectOpenedDetail = { project: kitHorizonProject(), notice: null };
    expect(announced[0]).toEqual(expected);
  });

  it('does not announce project-opened when the project cannot be opened', async () => {
    const open = vi.fn<RecentsApi['open']>(() =>
      Promise.resolve({ kind: 'missing', path: 'E:\\projets\\kit-horizon' } satisfies OpenProjectOutcome),
    );
    const home = await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon()])), open }));
    const announced = recordProjectOpened(home);

    await recentLinks().first().click();
    await expect.poll(() => open.mock.calls).toHaveLength(1);
    await open.mock.results[0]?.value;
    await home.updateComplete;

    expect(announced).toEqual([]);
  });
});

describe('<ui-home> theme button', () => {
  // covers: FCT-0016
  it('switches the studio to the dark theme and back from the top bar', async () => {
    await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon()])) }));

    await page.getByRole('button', { name: 'Passer au thème sombre' }).click();

    expect(document.documentElement.dataset['theme']).toBe('dark');
    await expect.element(page.getByRole('button', { name: 'Passer au thème clair' })).toBeVisible();

    await page.getByRole('button', { name: 'Passer au thème clair' }).click();

    expect(document.documentElement.dataset['theme']).toBe('light');
    await expect.element(page.getByRole('button', { name: 'Passer au thème sombre' })).toBeVisible();
  });

  // covers: FCT-0020
  it('offers the light theme when Windows is dark and no theme was chosen', async () => {
    await emulateColorScheme('dark');
    await mountHome(makeUiGenApi({ list: listOf(listing([kitHorizon()])) }));

    await expect.element(page.getByRole('button', { name: 'Passer au thème clair' })).toBeVisible();
  });
});
