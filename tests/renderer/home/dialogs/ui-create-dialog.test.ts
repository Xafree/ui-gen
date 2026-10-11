import { page, userEvent } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type {
  CreateProjectOutcome,
  CreateProjectRequest,
} from '../../../../src/modules/project/application/create/create-project.js';
import type { OpenedProject } from '../../../../src/modules/project/application/open/project-outcomes.js';
import type { RecentProjectView } from '../../../../src/modules/project/application/recents/recent-projects-use-cases.js';
import type { UiGenApi } from '../../../../src/preload/ui-gen-api.js';
import { UiCreateDialog } from '../../../../src/renderer/home/dialogs/ui-create-dialog.js';
import { UiHome } from '../../../../src/renderer/home/ui-home.js';
import {
  PROJECT_CREATED,
  PROJECT_OPENED,
  type ProjectCreatedDetail,
  type ProjectOpenedDetail,
} from '../../../../src/renderer/studio-events.js';
import { kitHorizon, listing, makeUiGenApi, type RecentsApi } from '../home-fixtures.js';
import { deepActiveElement, deepQueryAll, removeShownToasts, visibleText } from '../shadow-dom.js';

type ProjectApi = UiGenApi['project'];

const DEFAULT_LOCATION = 'E:\\projets';

const mounted: HTMLElement[] = [];

function createdKitHorizon(): OpenedProject {
  return {
    path: 'E:\\projets\\kit-horizon',
    name: 'Kit Horizon',
    packageName: '@kit-horizon/ui',
    version: '0.1.0',
    settings: { formatVersion: 1, tagPrefix: 'kh-', componentsDir: 'src/components' },
  };
}

function apiWith(project: Partial<ProjectApi>, recents: Partial<RecentsApi> = {}): UiGenApi {
  const base = makeUiGenApi(recents);
  return { ...base, project: { ...base.project, ...project } };
}

function defaultLocationAt(path: string): ProjectApi['defaultLocation'] {
  return vi.fn<ProjectApi['defaultLocation']>(() => Promise.resolve(path));
}

function createResolving(outcome: CreateProjectOutcome) {
  return vi.fn<ProjectApi['create']>(() => Promise.resolve(outcome));
}

async function openDialog(api: UiGenApi): Promise<UiCreateDialog> {
  const dialog = new UiCreateDialog();
  dialog.api = api;
  document.body.append(dialog);
  mounted.push(dialog);
  await dialog.updateComplete;
  await dialog.open(null);
  await dialog.updateComplete;
  return dialog;
}

function createDialog() {
  return page.getByRole('dialog', { name: 'Créer un projet' });
}

function field(label: string) {
  return createDialog().getByRole('textbox', { name: label, exact: true });
}

function button(label: string) {
  return createDialog().getByRole('button', { name: label, exact: true });
}

function locationError(): Element | undefined {
  return deepQueryAll('#cr-loc-err')[0];
}

function recordCreated(dialog: UiCreateDialog): unknown[] {
  const details: unknown[] = [];
  dialog.addEventListener(PROJECT_CREATED, (event: Event) => {
    if (event instanceof CustomEvent) {
      const detail: unknown = event.detail;
      details.push(detail);
    }
  });
  return details;
}

async function typeName(name: string): Promise<void> {
  await field('Nom du projet').fill(name);
}

afterEach(() => {
  for (const element of mounted.splice(0)) {
    element.remove();
  }
  removeShownToasts();
});

describe('<ui-create-dialog> opening', () => {
  it('opens a modal « Créer un projet » form, empty, in the proposed location, with the focus on the name', async () => {
    const defaultLocation = defaultLocationAt(DEFAULT_LOCATION);
    await openDialog(apiWith({ defaultLocation }));

    await expect.element(createDialog()).toBeVisible();
    await expect.element(field('Nom du projet')).toHaveValue('');
    await expect.element(field('Emplacement')).toHaveValue(DEFAULT_LOCATION);
    await expect.element(field('Emplacement')).toHaveAttribute('readonly');
    await expect.element(field('Nom du package npm')).toHaveValue('');
    await expect.element(field('Préfixe des balises')).toHaveValue('');
    await expect.element(button('Parcourir…')).toBeVisible();
    await expect.element(button('Annuler')).toBeEnabled();
    await expect.element(button('Créer le projet')).toBeDisabled();
    await expect.element(createDialog().getByText('Indiquez un nom de projet', { exact: true })).toBeVisible();
    await expect.element(createDialog().getByText('Indiquez un préfixe', { exact: true })).toBeVisible();
    expect(defaultLocation).toHaveBeenCalledTimes(1);
    await expect.poll(() => deepActiveElement()).toBe(field('Nom du projet').element());
  });

  it('starts again from an empty form and a fresh proposed location when opened a second time', async () => {
    const defaultLocation = vi
      .fn<ProjectApi['defaultLocation']>()
      .mockResolvedValueOnce(DEFAULT_LOCATION)
      .mockResolvedValueOnce('D:\\clients');
    const create = vi.fn<ProjectApi['create']>();
    const dialog = await openDialog(apiWith({ defaultLocation, create }));
    const created = recordCreated(dialog);

    await typeName('Kit Horizon');
    await field('Préfixe des balises').fill('hz-');
    await button('Annuler').click();
    await expect.element(createDialog()).not.toBeInTheDocument();

    await dialog.open(null);
    await dialog.updateComplete;

    await expect.element(field('Nom du projet')).toHaveValue('');
    await expect.element(field('Emplacement')).toHaveValue('D:\\clients');
    await expect.element(field('Préfixe des balises')).toHaveValue('');
    await expect.element(button('Créer le projet')).toBeDisabled();
    expect(create).not.toHaveBeenCalled();
    expect(created).toEqual([]);
  });
});

describe('<ui-create-dialog> proposals', () => {
  // covers: FCT-0002
  it('proposes @kit-horizon/ui and kh- for « Kit Horizon », with the project folder and the example tag', async () => {
    await openDialog(apiWith({ defaultLocation: defaultLocationAt(DEFAULT_LOCATION) }));

    await typeName('Kit Horizon');

    await expect.element(field('Nom du package npm')).toHaveValue('@kit-horizon/ui');
    await expect.element(field('Préfixe des balises')).toHaveValue('kh-');
    await expect.element(createDialog().getByText('E:\\projets\\kit-horizon', { exact: true })).toBeVisible();
    await expect.element(createDialog().getByText('<kh-button>', { exact: true })).toBeVisible();
    await expect.element(button('Créer le projet')).toBeEnabled();
    expect(createDialog().getByRole('button', { name: 'Reprendre la proposition' }).elements()).toEqual([]);
  });

  // covers: FCT-0002
  it('lets the user change both proposals, keeps them when the name changes, and takes the proposals back on request', async () => {
    await openDialog(apiWith({ defaultLocation: defaultLocationAt(DEFAULT_LOCATION) }));
    await typeName('Kit Horizon');

    await field('Nom du package npm').fill('@horizon/ui');
    await field('Préfixe des balises').fill('hz-');
    await typeName('Kit Horizon Pro');

    await expect.element(field('Nom du package npm')).toHaveValue('@horizon/ui');
    await expect.element(field('Préfixe des balises')).toHaveValue('hz-');
    await expect.element(createDialog().getByText('E:\\projets\\horizon', { exact: true })).toBeVisible();
    await expect.element(createDialog().getByText('<hz-button>', { exact: true })).toBeVisible();
    await expect.element(button('Créer le projet')).toBeEnabled();

    const resets = createDialog().getByRole('button', { name: 'Reprendre la proposition' });
    await expect.poll(() => resets.elements()).toHaveLength(2);
    await resets.first().click();
    await expect.element(field('Nom du package npm')).toHaveValue('@kit-horizon-pro/ui');
    await createDialog().getByRole('button', { name: 'Reprendre la proposition' }).first().click();
    await expect.element(field('Préfixe des balises')).toHaveValue('khp-');
  });

  it('asks for the project name once the name field is left empty', async () => {
    await openDialog(apiWith({ defaultLocation: defaultLocationAt(DEFAULT_LOCATION) }));

    await typeName('Kit');
    await typeName('');
    await field('Nom du package npm').click();

    await expect.element(createDialog().getByText('Indiquez le nom du projet.', { exact: true })).toBeVisible();
    await expect.element(button('Créer le projet')).toBeDisabled();
  });

  it('refuses a name without any letter or digit', async () => {
    await openDialog(apiWith({ defaultLocation: defaultLocationAt(DEFAULT_LOCATION) }));

    await typeName('!!!');

    await expect
      .element(createDialog().getByText('Le nom doit contenir au moins une lettre ou un chiffre.', { exact: true }))
      .toBeVisible();
    await expect.element(button('Créer le projet')).toBeDisabled();
  });

  it('refuses an invalid npm package name', async () => {
    await openDialog(apiWith({ defaultLocation: defaultLocationAt(DEFAULT_LOCATION) }));
    await typeName('Kit Horizon');

    await field('Nom du package npm').fill('Kit Horizon');

    await expect
      .element(
        createDialog().getByText('Nom de package npm invalide : minuscules, sans espace, par exemple @kit-horizon/ui.', {
          exact: true,
        }),
      )
      .toBeVisible();
    await expect.element(button('Créer le projet')).toBeDisabled();
  });
});

describe('<ui-create-dialog> tag prefix rules', () => {
  const rules: readonly (readonly [string, string])[] = [
    ['KH-', "Le préfixe s'écrit en minuscules : kh- au lieu de KH-."],
    ['1kh-', 'Le préfixe commence par une lettre, par exemple kh-.'],
    ['k_h-', 'Utilisez seulement des lettres minuscules, des chiffres et des tirets.'],
    ['kh', 'Ajoutez un tiret à la fin : kh-.'],
    ['k--h-', 'Retirez le double tiret : un seul tiret sépare le préfixe du nom.'],
    ['', 'Indiquez un préfixe, par exemple kh-.'],
  ];

  // covers: FCT-0005
  it.each(rules)('explains the rule broken by « %s » and forbids the creation', async (prefix, message) => {
    const create = vi.fn<ProjectApi['create']>();
    await openDialog(apiWith({ defaultLocation: defaultLocationAt(DEFAULT_LOCATION), create }));
    await typeName('Kit Horizon');

    await field('Préfixe des balises').fill(prefix);

    await expect.element(createDialog().getByText(message, { exact: true })).toBeVisible();
    await expect.element(field('Préfixe des balises')).toHaveAttribute('aria-invalid', 'true');
    await expect.element(button('Créer le projet')).toBeDisabled();
    await userEvent.keyboard('{Enter}');
    await expect.element(createDialog()).toBeVisible();
    expect(create).not.toHaveBeenCalled();
  });

  // covers: FCT-0005
  it('asks to fix the prefix in the example tag, then shows the tag again once the prefix is fixed', async () => {
    await openDialog(apiWith({ defaultLocation: defaultLocationAt(DEFAULT_LOCATION) }));
    await typeName('Kit Horizon');

    await field('Préfixe des balises').fill('kh--');
    await expect.element(createDialog().getByText('Corrigez le préfixe', { exact: true })).toBeVisible();

    await field('Préfixe des balises').fill('kh-');
    await expect.element(createDialog().getByText('<kh-button>', { exact: true })).toBeVisible();
    await expect.element(field('Préfixe des balises')).not.toHaveAttribute('aria-invalid', 'true');
    await expect.element(button('Créer le projet')).toBeEnabled();
  });
});

describe('<ui-create-dialog> location', () => {
  it('replaces the location by the chosen folder and gives the focus back to « Parcourir… »', async () => {
    const chooseLocation = vi.fn<ProjectApi['chooseLocation']>(() =>
      Promise.resolve({ canceled: false, path: 'D:\\clients' }),
    );
    await openDialog(apiWith({ defaultLocation: defaultLocationAt(DEFAULT_LOCATION), chooseLocation }));
    await typeName('Kit Horizon');

    await button('Parcourir…').click();

    await expect.element(field('Emplacement')).toHaveValue('D:\\clients');
    await expect.element(createDialog().getByText('D:\\clients\\kit-horizon', { exact: true })).toBeVisible();
    expect(chooseLocation).toHaveBeenCalledTimes(1);
    await expect.poll(() => deepActiveElement()).toBe(button('Parcourir…').element());
  });

  it('keeps the location when the folder choice is canceled', async () => {
    const chooseLocation = vi.fn<ProjectApi['chooseLocation']>(() => Promise.resolve({ canceled: true }));
    await openDialog(apiWith({ defaultLocation: defaultLocationAt(DEFAULT_LOCATION), chooseLocation }));
    await typeName('Kit Horizon');

    await button('Parcourir…').click();
    await expect.poll(() => chooseLocation.mock.calls).toHaveLength(1);
    await chooseLocation.mock.results[0]?.value;

    await expect.element(field('Emplacement')).toHaveValue(DEFAULT_LOCATION);
    await expect.element(createDialog().getByText('E:\\projets\\kit-horizon', { exact: true })).toBeVisible();
  });
});

describe('<ui-create-dialog> creation', () => {
  // covers: FCT-0003
  it('sends the name, the location and the settings, shows « Création… » meanwhile, then closes and announces project-created', async () => {
    const creating = Promise.withResolvers<CreateProjectOutcome>();
    const create = vi.fn<ProjectApi['create']>(() => creating.promise);
    const dialog = await openDialog(apiWith({ defaultLocation: defaultLocationAt(DEFAULT_LOCATION), create }));
    const created = recordCreated(dialog);
    await typeName('Kit Horizon');

    await button('Créer le projet').click();

    const expectedRequest: CreateProjectRequest = {
      name: 'Kit Horizon',
      location: DEFAULT_LOCATION,
      packageName: '@kit-horizon/ui',
      tagPrefix: 'kh-',
    };
    expect(create).toHaveBeenCalledTimes(1);
    expect(create).toHaveBeenCalledWith(expectedRequest);
    await expect.element(button('Création…')).toHaveAttribute('aria-busy', 'true');
    await expect.element(button('Annuler')).toBeDisabled();
    expect(created).toEqual([]);

    creating.resolve({ kind: 'created', project: createdKitHorizon() });

    await expect.poll(() => created).toHaveLength(1);
    const expected: ProjectCreatedDetail = { project: createdKitHorizon() };
    expect(created[0]).toEqual(expected);
    await expect.element(createDialog()).not.toBeInTheDocument();
  });

  // covers: FCT-0003
  it('sends the edited settings and the chosen location', async () => {
    const create = createResolving({ kind: 'created', project: createdKitHorizon() });
    const chooseLocation = vi.fn<ProjectApi['chooseLocation']>(() =>
      Promise.resolve({ canceled: false, path: 'D:\\clients' }),
    );
    const dialog = await openDialog(apiWith({ defaultLocation: defaultLocationAt(DEFAULT_LOCATION), chooseLocation, create }));
    const created = recordCreated(dialog);
    await typeName('Kit Horizon');
    await field('Nom du package npm').fill('@horizon/ui');
    await field('Préfixe des balises').fill('hz-');
    await button('Parcourir…').click();
    await expect.element(field('Emplacement')).toHaveValue('D:\\clients');

    await button('Créer le projet').click();

    const expectedRequest: CreateProjectRequest = {
      name: 'Kit Horizon',
      location: 'D:\\clients',
      packageName: '@horizon/ui',
      tagPrefix: 'hz-',
    };
    expect(create).toHaveBeenCalledWith(expectedRequest);
    await expect.poll(() => created).toHaveLength(1);
  });

  // covers: FCT-0004
  it('names the existing folder under the location, keeps the dialog open and puts the focus on the name', async () => {
    const create = createResolving({ kind: 'folder-not-empty', path: 'E:\\projets\\kit-horizon', entryCount: 214 });
    const dialog = await openDialog(apiWith({ defaultLocation: defaultLocationAt(DEFAULT_LOCATION), create }));
    const created = recordCreated(dialog);
    await typeName('Kit Horizon');

    await button('Créer le projet').click();

    await expect
      .poll(() => visibleText(locationError() ?? null))
      .toBe(
        "Le dossier E:\\projets\\kit-horizon existe déjà et contient 214 fichiers. Rien n'a été créé. Changez le nom du projet ou l'emplacement.",
      );
    expect(locationError()?.checkVisibility()).toBe(true);
    await expect.element(createDialog()).toBeVisible();
    await expect.element(button('Créer le projet')).not.toHaveAttribute('aria-busy', 'true');
    await expect.element(button('Annuler')).toBeEnabled();
    await expect.poll(() => deepActiveElement()).toBe(field('Nom du projet').element());
    expect(created).toEqual([]);
  });

  // covers: FCT-0004
  it('clears the location error as soon as the name changes', async () => {
    const create = createResolving({ kind: 'folder-not-empty', path: 'E:\\projets\\kit-horizon', entryCount: 214 });
    await openDialog(apiWith({ defaultLocation: defaultLocationAt(DEFAULT_LOCATION), create }));
    await typeName('Kit Horizon');
    await button('Créer le projet').click();
    await expect.poll(() => visibleText(locationError() ?? null)).not.toBe('');

    await typeName('Kit Horizon 2');

    await expect.poll(() => visibleText(locationError() ?? null)).toBe('');
    await expect.element(createDialog().getByText('E:\\projets\\kit-horizon-2', { exact: true })).toBeVisible();
  });

  // covers: FCT-0013
  it('explains a failed write under the location, says nothing was left on the disk and puts the focus on « Parcourir… »', async () => {
    const create = createResolving({ kind: 'write-failed', path: 'C:\\Program Files\\kit-horizon', code: 'EPERM' });
    const dialog = await openDialog(apiWith({ defaultLocation: defaultLocationAt('C:\\Program Files'), create }));
    const created = recordCreated(dialog);
    await typeName('Kit Horizon');

    await button('Créer le projet').click();

    await expect
      .poll(() => visibleText(locationError() ?? null))
      .toBe(
        "Le projet n'a pas pu être créé dans C:\\Program Files\\kit-horizon (EPERM). Aucun dossier ni fichier n'a été laissé sur le disque. Choisissez un autre emplacement.",
      );
    await expect.element(createDialog()).toBeVisible();
    await expect.element(button('Créer le projet')).toBeEnabled();
    await expect.element(button('Annuler')).toBeEnabled();
    await expect.poll(() => deepActiveElement()).toBe(button('Parcourir…').element());
    expect(created).toEqual([]);
  });

  // covers: FCT-0013
  it('clears the failed write message once another location is chosen', async () => {
    const create = createResolving({ kind: 'write-failed', path: 'C:\\Program Files\\kit-horizon', code: 'EPERM' });
    const chooseLocation = vi.fn<ProjectApi['chooseLocation']>(() =>
      Promise.resolve({ canceled: false, path: 'D:\\clients' }),
    );
    await openDialog(apiWith({ defaultLocation: defaultLocationAt('C:\\Program Files'), chooseLocation, create }));
    await typeName('Kit Horizon');
    await button('Créer le projet').click();
    await expect.poll(() => visibleText(locationError() ?? null)).not.toBe('');

    await button('Parcourir…').click();

    await expect.element(field('Emplacement')).toHaveValue('D:\\clients');
    await expect.poll(() => visibleText(locationError() ?? null)).toBe('');
  });
});

describe('<ui-home> create project', () => {
  async function mountHome(api: UiGenApi): Promise<UiHome> {
    const home = new UiHome();
    home.api = api;
    home.notice = null;
    document.body.append(home);
    mounted.push(home);
    await home.updateComplete;
    return home;
  }

  function recordOpened(home: UiHome): unknown[] {
    const details: unknown[] = [];
    home.addEventListener(PROJECT_OPENED, (event: Event) => {
      if (event instanceof CustomEvent) {
        const detail: unknown = event.detail;
        details.push(detail);
      }
    });
    return details;
  }

  function listOf(projects: RecentProjectView[]): RecentsApi['list'] {
    return vi.fn<RecentsApi['list']>(() => Promise.resolve(listing(projects)));
  }

  it('opens the create dialog from « Créer un projet » at the top of the home screen', async () => {
    await mountHome(
      apiWith({ defaultLocation: defaultLocationAt(DEFAULT_LOCATION) }, { list: listOf([kitHorizon()]) }),
    );

    await page.getByRole('button', { name: 'Créer un projet', exact: true }).click();

    await expect.element(createDialog()).toBeVisible();
    await expect.element(field('Emplacement')).toHaveValue(DEFAULT_LOCATION);
    await expect.poll(() => deepActiveElement()).toBe(field('Nom du projet').element());
  });

  it('opens the create dialog from the empty state too', async () => {
    await mountHome(apiWith({ defaultLocation: defaultLocationAt(DEFAULT_LOCATION) }, { list: listOf([]) }));
    const createButtons = page.getByRole('button', { name: 'Créer un projet', exact: true });
    await expect.poll(() => createButtons.elements()).toHaveLength(2);

    await createButtons.nth(1).click();

    await expect.element(createDialog()).toBeVisible();
  });

  // covers: FCT-0003
  it('opens the created project with the « Projet créé » notice', async () => {
    const create = createResolving({ kind: 'created', project: createdKitHorizon() });
    const home = await mountHome(
      apiWith({ defaultLocation: defaultLocationAt(DEFAULT_LOCATION), create }, { list: listOf([]) }),
    );
    const opened = recordOpened(home);
    await page.getByRole('button', { name: 'Créer un projet', exact: true }).first().click();
    await typeName('Kit Horizon');

    await button('Créer le projet').click();

    await expect.poll(() => opened).toHaveLength(1);
    const expected: ProjectOpenedDetail = {
      project: createdKitHorizon(),
      notice: { tone: 'success', title: 'Projet créé', message: 'Kit Horizon est prêt dans E:\\projets\\kit-horizon.' },
    };
    expect(opened[0]).toEqual(expected);
  });
});
