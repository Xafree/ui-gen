import { page } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { OpenProjectOutcome } from '../../../../src/modules/project/application/open/project-outcomes.js';
import type { UiGenApi } from '../../../../src/preload/ui-gen-api.js';
import { UiHome } from '../../../../src/renderer/home/ui-home.js';
import { PROJECT_OPENED, type ProjectOpenedDetail } from '../../../../src/renderer/studio-events.js';
import { kitHorizon, listing, type RecentsApi } from '../home-fixtures.js';
import { deepActiveElement, removeShownToasts, shownToasts, visibleText } from '../shadow-dom.js';
import {
  apiWith,
  ATLAS_PATH,
  atlasProject,
  atlasProposal,
  atlasRecent,
  closedChannel,
  recordDetails,
  type ProjectApi,
} from './opening-fixtures.js';

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

function recentSection() {
  return page.getByRole('region', { name: 'Projets récents' });
}

function atlasLink() {
  return recentSection().getByRole('link').filter({ hasText: 'Atlas' });
}

function adoptDialog() {
  return page.getByRole('dialog', { name: 'Ouvrir Atlas avec UI-GEN ?' });
}

function openResolving(outcome: OpenProjectOutcome) {
  return vi.fn<RecentsApi['open']>(() => Promise.resolve(outcome));
}

async function homeOpening(
  open: RecentsApi['open'],
  project: Partial<ProjectApi> = {},
  list: RecentsApi['list'] = () => Promise.resolve(listing([atlasRecent(true), kitHorizon()])),
): Promise<UiHome> {
  const home = await mountHome(apiWith(project, { list, open }));
  await expect.element(atlasLink()).toBeVisible();
  return home;
}

afterEach(() => {
  for (const home of mounted.splice(0)) {
    home.remove();
  }
  removeShownToasts();
  vi.restoreAllMocks();
});

describe('<ui-home> opening a recent project that does not open directly', () => {
  // covers: FCT-0007
  // covers: FCT-0021
  it('asks to adopt a recent Lit library whose uigen.json is gone', async () => {
    const open = openResolving({ kind: 'needs-adoption', proposal: atlasProposal() });
    const adopt = vi.fn<ProjectApi['adopt']>(closedChannel);
    const home = await homeOpening(open, { adopt });
    const opened = recordDetails(home, PROJECT_OPENED);

    await atlasLink().click();

    await expect.element(adoptDialog()).toBeVisible();
    expect(open).toHaveBeenCalledWith(ATLAS_PATH);
    await expect
      .element(adoptDialog().getByRole('textbox', { name: 'Préfixe des balises', exact: true }))
      .toHaveValue('atlas-');
    await expect
      .element(adoptDialog().getByRole('textbox', { name: 'Dossier des composants', exact: true }))
      .toHaveValue('src/components');
    expect(adopt).not.toHaveBeenCalled();
    expect(opened).toEqual([]);
  });

  // covers: FCT-0007
  it('opens the recent library once adopted, with the « ouvert avec UI-GEN » notice', async () => {
    const open = openResolving({ kind: 'needs-adoption', proposal: atlasProposal() });
    const adopt = vi.fn<ProjectApi['adopt']>(() =>
      Promise.resolve({ kind: 'opened', project: atlasProject('atlas-', 'src/components') } satisfies OpenProjectOutcome),
    );
    const home = await homeOpening(open, { adopt });
    const opened = recordDetails(home, PROJECT_OPENED);
    await atlasLink().click();

    await adoptDialog().getByRole('button', { name: 'Ajouter uigen.json et ouvrir', exact: true }).click();

    await expect.poll(() => opened).toHaveLength(1);
    const expected: ProjectOpenedDetail = {
      project: atlasProject('atlas-', 'src/components'),
      notice: {
        tone: 'success',
        title: 'Atlas ouvert avec UI-GEN',
        message: `uigen.json ajouté à ${ATLAS_PATH} avec le préfixe atlas-.`,
      },
    };
    expect(opened[0]).toEqual(expected);
  });

  // covers: FCT-0008
  it('stays on the home screen with the recent list when the adoption of a recent library is canceled', async () => {
    const open = openResolving({ kind: 'needs-adoption', proposal: atlasProposal() });
    const adopt = vi.fn<ProjectApi['adopt']>(closedChannel);
    const home = await homeOpening(open, { adopt });
    const opened = recordDetails(home, PROJECT_OPENED);
    await atlasLink().click();
    await expect.element(adoptDialog()).toBeVisible();

    await adoptDialog().getByRole('button', { name: 'Annuler', exact: true }).click();

    await expect.element(adoptDialog()).not.toBeInTheDocument();
    expect(adopt).not.toHaveBeenCalled();
    expect(opened).toEqual([]);
    await expect.element(atlasLink()).toBeVisible();
    await expect.poll(() => deepActiveElement()).toBe(atlasLink().element());
  });

  // covers: FCT-0009
  it('explains that a recent folder is no longer a Lit library and gives the focus back to its link', async () => {
    const open = openResolving({ kind: 'not-a-lit-library', path: ATLAS_PATH, reason: 'missing-lit-dependency' });
    const home = await homeOpening(open);
    const opened = recordDetails(home, PROJECT_OPENED);

    await atlasLink().click();

    await expect.poll(() => shownToasts()).toEqual([
      {
        tone: 'danger',
        title: "Ce dossier n'est pas une bibliothèque Lit",
        message:
          "Aucun package.json ne déclare la dépendance lit. Rien n'a été modifié. Choisissez le dossier racine de la bibliothèque, ou créez un projet.",
      },
    ]);
    await expect.poll(() => deepActiveElement()).toBe(atlasLink().element());
    expect(atlasLink().element().getAttribute('aria-busy')).not.toBe('true');
    expect(opened).toEqual([]);
  });

  it('explains why the uigen.json of a recent project cannot be read', async () => {
    const open = openResolving({
      kind: 'invalid-settings',
      path: ATLAS_PATH,
      errors: [{ code: 'newer-format-version', field: 'formatVersion' }],
    });
    const home = await homeOpening(open);
    const opened = recordDetails(home, PROJECT_OPENED);

    await atlasLink().click();

    await expect.poll(() => shownToasts()).toEqual([
      {
        tone: 'danger',
        title: "Ce projet ne s'ouvre pas",
        message:
          "uigen.json a été créé par une version plus récente d'UI-GEN. Rien n'a été modifié. Mettez à jour UI-GEN pour ouvrir ce projet.",
      },
    ]);
    expect(opened).toEqual([]);
  });

  // covers: FCT-0011
  it('warns that a recent project was moved and reads the recent list again to mark it « Introuvable »', async () => {
    const list = vi
      .fn<RecentsApi['list']>()
      .mockResolvedValueOnce(listing([atlasRecent(true), kitHorizon()]))
      .mockResolvedValue(listing([atlasRecent(false), kitHorizon()]));
    const open = openResolving({ kind: 'missing', path: ATLAS_PATH });
    const home = await homeOpening(open, {}, list);
    const opened = recordDetails(home, PROJECT_OPENED);

    await atlasLink().click();

    await expect.poll(() => shownToasts()).toEqual([
      {
        tone: 'warning',
        title: 'Projet introuvable',
        message: `Le dossier ${ATLAS_PATH} a été déplacé ou supprimé.`,
      },
    ]);
    await expect.poll(() => list.mock.calls.length).toBeGreaterThanOrEqual(2);
    await expect.element(recentSection().getByText('Introuvable', { exact: true })).toBeVisible();
    await expect.element(atlasLink()).not.toBeInTheDocument();
    expect(recentSection().getByRole('link').elements().map((link) => visibleText(link))).toHaveLength(1);
    expect(opened).toEqual([]);
  });
});
