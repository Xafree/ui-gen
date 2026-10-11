import { page } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { OpenProjectOutcome } from '../../../../src/modules/project/application/open/project-outcomes.js';
import type { RecentProjectView } from '../../../../src/modules/project/application/recents/recent-projects-use-cases.js';
import type { OpenFolderOutcome, UiGenApi } from '../../../../src/preload/ui-gen-api.js';
import { UiHome } from '../../../../src/renderer/home/ui-home.js';
import { PROJECT_OPENED, type ProjectOpenedDetail } from '../../../../src/renderer/studio-events.js';
import { kitHorizon, kitHorizonProject, listing, type RecentsApi } from '../home-fixtures.js';
import { deepActiveElement, deepQueryAll, removeShownToasts, shownToasts, visibleText } from '../shadow-dom.js';
import {
  apiWith,
  ATLAS_PATH,
  atlasProject,
  atlasProposal,
  closedChannel,
  recordDetails,
  REFUSED_MESSAGE,
  REFUSED_NOTICE,
  type ProjectApi,
} from './opening-fixtures.js';

const VITRINE_PATH = 'E:\\documents\\site-vitrine';
const NOT_A_LIT_LIBRARY = {
  tone: 'danger',
  title: "Ce dossier n'est pas une bibliothèque Lit",
  message:
    "Aucun package.json ne déclare la dépendance lit. Rien n'a été modifié. Choisissez le dossier racine de la bibliothèque, ou créez un projet.",
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

function listOf(projects: RecentProjectView[]): RecentsApi['list'] {
  return vi.fn<RecentsApi['list']>(() => Promise.resolve(listing(projects)));
}

function openFolderResolving(outcome: OpenFolderOutcome) {
  return vi.fn<ProjectApi['openFolder']>(() => Promise.resolve(outcome));
}

async function homeWithProjects(project: Partial<ProjectApi>): Promise<UiHome> {
  const home = await mountHome(apiWith(project, { list: listOf([kitHorizon()]) }));
  await expect.element(page.getByRole('region', { name: 'Projets récents' }).getByRole('link')).toBeVisible();
  return home;
}

function topOpenButton(): Element | null {
  return deepQueryAll('#btn-open')[0] ?? null;
}

function adoptDialog() {
  return page.getByRole('dialog', { name: 'Ouvrir Atlas avec UI-GEN ?' });
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

describe('<ui-home> « Ouvrir un projet »', () => {
  // covers: FCT-0006
  it('reads the chosen folder, busy meanwhile with its icon kept, then opens the project without notice nor question', async () => {
    const choosing = Promise.withResolvers<OpenFolderOutcome>();
    const openFolder = vi.fn<ProjectApi['openFolder']>(() => choosing.promise);
    const home = await homeWithProjects({ openFolder });
    const opened = recordDetails(home, PROJECT_OPENED);

    await clickOpen();

    await expect.poll(() => openFolder.mock.calls).toHaveLength(1);
    await expect.poll(() => topOpenButton()?.getAttribute('aria-busy')).toBe('true');
    expect(visibleText(topOpenButton())).toBe('Lecture du dossier…');
    expect(topOpenButton()?.querySelector('svg')).not.toBeNull();
    expect(opened).toEqual([]);

    choosing.resolve({ kind: 'opened', project: kitHorizonProject() });

    await expect.poll(() => opened).toHaveLength(1);
    const expected: ProjectOpenedDetail = { project: kitHorizonProject(), notice: null };
    expect(opened[0]).toEqual(expected);
    expect(shownToasts()).toEqual([]);
    await expect.element(adoptDialog()).not.toBeInTheDocument();
  });

  it('does nothing when the folder choice is canceled, and takes its label back', async () => {
    const openFolder = openFolderResolving({ kind: 'canceled' });
    const home = await homeWithProjects({ openFolder });
    const opened = recordDetails(home, PROJECT_OPENED);

    await clickOpen();

    await expect.poll(() => openFolder.mock.calls).toHaveLength(1);
    await expect.poll(() => topOpenButton()?.getAttribute('aria-busy')).not.toBe('true');
    expect(visibleText(topOpenButton())).toBe('Ouvrir un projet');
    expect(topOpenButton()?.querySelector('svg')).not.toBeNull();
    expect(opened).toEqual([]);
    expect(shownToasts()).toEqual([]);
  });

  // covers: FCT-0006
  it('opens a project from « Ouvrir un projet » of the empty state, busy on that button', async () => {
    const choosing = Promise.withResolvers<OpenFolderOutcome>();
    const openFolder = vi.fn<ProjectApi['openFolder']>(() => choosing.promise);
    const home = await mountHome(apiWith({ openFolder }, { list: listOf([]) }));
    const opened = recordDetails(home, PROJECT_OPENED);
    await expect.poll(() => deepQueryAll('#empty-open')).toHaveLength(1);
    const emptyOpen = deepQueryAll('#empty-open')[0];

    if (emptyOpen instanceof HTMLElement) emptyOpen.click();

    await expect.poll(() => openFolder.mock.calls).toHaveLength(1);
    await expect.poll(() => emptyOpen?.getAttribute('aria-busy')).toBe('true');
    expect(visibleText(emptyOpen ?? null)).toBe('Lecture du dossier…');
    expect(topOpenButton()?.getAttribute('aria-busy')).not.toBe('true');

    choosing.resolve({ kind: 'opened', project: kitHorizonProject() });

    await expect.poll(() => opened).toHaveLength(1);
  });

  // covers: FCT-0007
  // covers: FCT-0021
  it('asks to adopt a Lit library without uigen.json, with the detected prefix and folder', async () => {
    const adopt = vi.fn<ProjectApi['adopt']>(closedChannel);
    const openFolder = openFolderResolving({ kind: 'needs-adoption', proposal: atlasProposal() });
    const home = await homeWithProjects({ openFolder, adopt });
    const opened = recordDetails(home, PROJECT_OPENED);

    await clickOpen();

    await expect.element(adoptDialog()).toBeVisible();
    await expect
      .element(adoptDialog().getByRole('textbox', { name: 'Préfixe des balises', exact: true }))
      .toHaveValue('atlas-');
    await expect
      .element(adoptDialog().getByRole('textbox', { name: 'Dossier des composants', exact: true }))
      .toHaveValue('src/components');
    await expect.poll(() => topOpenButton()?.getAttribute('aria-busy')).not.toBe('true');
    expect(visibleText(topOpenButton())).toBe('Ouvrir un projet');
    expect(adopt).not.toHaveBeenCalled();
    expect(opened).toEqual([]);
  });

  // covers: FCT-0008
  it('stays on the home screen and gives the focus back to « Ouvrir un projet » when the adoption is canceled', async () => {
    const adopt = vi.fn<ProjectApi['adopt']>(closedChannel);
    const openFolder = openFolderResolving({ kind: 'needs-adoption', proposal: atlasProposal() });
    const home = await homeWithProjects({ openFolder, adopt });
    const opened = recordDetails(home, PROJECT_OPENED);
    await clickOpen();
    await expect.element(adoptDialog()).toBeVisible();

    await adoptDialog().getByRole('button', { name: 'Annuler', exact: true }).click();

    await expect.element(adoptDialog()).not.toBeInTheDocument();
    await expect.poll(() => deepActiveElement()).toBe(topOpenButton());
    expect(adopt).not.toHaveBeenCalled();
    expect(opened).toEqual([]);
    expect(shownToasts()).toEqual([]);
    expect(home.isConnected).toBe(true);
  });

  // covers: FCT-0007
  it('opens the adopted library with the « ouvert avec UI-GEN » notice naming the folder and the confirmed prefix', async () => {
    const adopt = vi.fn<ProjectApi['adopt']>(() =>
      Promise.resolve({ kind: 'opened', project: atlasProject('at-', 'src/components') } satisfies OpenProjectOutcome),
    );
    const openFolder = openFolderResolving({ kind: 'needs-adoption', proposal: atlasProposal() });
    const home = await homeWithProjects({ openFolder, adopt });
    const opened = recordDetails(home, PROJECT_OPENED);
    await clickOpen();
    await adoptDialog().getByRole('textbox', { name: 'Préfixe des balises', exact: true }).fill('at-');

    await adoptDialog().getByRole('button', { name: 'Ajouter uigen.json et ouvrir', exact: true }).click();

    await expect.poll(() => opened).toHaveLength(1);
    const expected: ProjectOpenedDetail = {
      project: atlasProject('at-', 'src/components'),
      notice: {
        tone: 'success',
        title: 'Atlas ouvert avec UI-GEN',
        message: `uigen.json ajouté à ${ATLAS_PATH} avec le préfixe at-.`,
      },
    };
    expect(opened[0]).toEqual(expected);
    expect(adopt).toHaveBeenCalledWith({ path: ATLAS_PATH, tagPrefix: 'at-', componentsDir: 'src/components' });
  });

  it('keeps a refused adoption inside the dialog, never as a toast on the home screen', async () => {
    const adopt = vi.fn<ProjectApi['adopt']>(closedChannel);
    const openFolder = openFolderResolving({ kind: 'needs-adoption', proposal: atlasProposal() });
    const home = await homeWithProjects({ openFolder, adopt });
    const opened = recordDetails(home, PROJECT_OPENED);
    await clickOpen();

    await adoptDialog().getByRole('button', { name: 'Ajouter uigen.json et ouvrir', exact: true }).click();

    await expect.poll(() => visibleText(deepQueryAll('#ad-err')[0] ?? null)).toBe(REFUSED_MESSAGE);
    await expect.element(adoptDialog()).toBeVisible();
    expect(shownToasts()).toEqual([]);
    expect(opened).toEqual([]);
  });

  // covers: FCT-0009
  it('explains that the folder is not a Lit library, opens nothing and gives the focus back to the button', async () => {
    const openFolder = openFolderResolving({
      kind: 'not-a-lit-library',
      path: VITRINE_PATH,
      reason: 'missing-package-json',
    });
    const home = await homeWithProjects({ openFolder });
    const opened = recordDetails(home, PROJECT_OPENED);

    await clickOpen();

    await expect.poll(() => shownToasts()).toEqual([NOT_A_LIT_LIBRARY]);
    await expect.poll(() => deepActiveElement()).toBe(topOpenButton());
    expect(topOpenButton()?.getAttribute('aria-busy')).not.toBe('true');
    expect(visibleText(topOpenButton())).toBe('Ouvrir un projet');
    expect(opened).toEqual([]);
    await expect.element(adoptDialog()).not.toBeInTheDocument();
  });

  it('asks to update UI-GEN when uigen.json comes from a newer version, whatever the order of the errors', async () => {
    const openFolder = openFolderResolving({
      kind: 'invalid-settings',
      path: 'E:\\projets\\nova-ui',
      errors: [
        { code: 'unknown-field', field: 'theme' },
        { code: 'newer-format-version', field: 'formatVersion' },
      ],
    });
    const home = await homeWithProjects({ openFolder });
    const opened = recordDetails(home, PROJECT_OPENED);

    await clickOpen();

    await expect.poll(() => shownToasts()).toEqual([
      {
        tone: 'danger',
        title: "Ce projet ne s'ouvre pas",
        message:
          "uigen.json a été créé par une version plus récente d'UI-GEN. Rien n'a été modifié. Mettez à jour UI-GEN pour ouvrir ce projet.",
      },
    ]);
    await expect.poll(() => deepActiveElement()).toBe(topOpenButton());
    expect(opened).toEqual([]);
  });

  it('lists the uigen.json errors and asks to fix the file', async () => {
    const openFolder = openFolderResolving({
      kind: 'invalid-settings',
      path: 'E:\\clients\\orbite\\ui',
      errors: [
        { code: 'unknown-field', field: 'tagprefix' },
        { code: 'missing-field', field: 'tagPrefix' },
      ],
    });
    await homeWithProjects({ openFolder });

    await clickOpen();

    await expect.poll(() => shownToasts()).toEqual([
      {
        tone: 'danger',
        title: "Ce projet ne s'ouvre pas",
        message:
          "Champ inconnu « tagprefix » dans uigen.json. Champ obligatoire « tagPrefix » absent de uigen.json. Rien n'a été modifié. Corrigez uigen.json, puis ouvrez de nouveau le projet.",
      },
    ]);
  });

  it('says the request was refused when the folder cannot be read through the bridge, and takes its label back', async () => {
    const openFolder = vi.fn<ProjectApi['openFolder']>(closedChannel);
    const home = await homeWithProjects({ openFolder });
    const opened = recordDetails(home, PROJECT_OPENED);

    await clickOpen();

    await expect.poll(() => shownToasts()).toEqual([REFUSED_NOTICE]);
    await expect.poll(() => deepActiveElement()).toBe(topOpenButton());
    expect(topOpenButton()?.getAttribute('aria-busy')).not.toBe('true');
    expect(visibleText(topOpenButton())).toBe('Ouvrir un projet');
    expect(opened).toEqual([]);
  });

  it('sends a single request when « Ouvrir un projet » is clicked again while the folder is read', async () => {
    const choosing = Promise.withResolvers<OpenFolderOutcome>();
    const openFolder = vi.fn<ProjectApi['openFolder']>(() => choosing.promise);
    const home = await homeWithProjects({ openFolder });
    const opened = recordDetails(home, PROJECT_OPENED);
    await clickOpen();
    await expect.poll(() => topOpenButton()?.getAttribute('aria-busy')).toBe('true');

    const busy = topOpenButton();
    if (busy instanceof HTMLElement) busy.click();
    choosing.resolve({ kind: 'opened', project: kitHorizonProject() });

    await expect.poll(() => opened).toHaveLength(1);
    await expect.poll(() => openFolder.mock.calls).toHaveLength(1);
  });
});
