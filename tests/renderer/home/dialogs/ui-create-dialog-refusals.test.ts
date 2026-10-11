import { page } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { CreateProjectOutcome } from '../../../../src/modules/project/application/create/create-project.js';
import type { ChooseLocationOutcome, UiGenApi } from '../../../../src/preload/ui-gen-api.js';
import { UiCreateDialog } from '../../../../src/renderer/home/dialogs/ui-create-dialog.js';
import { UiHome } from '../../../../src/renderer/home/ui-home.js';
import { NOTICE_REQUESTED, PROJECT_CREATED } from '../../../../src/renderer/studio-events.js';
import { kitHorizonProject, listing, makeUiGenApi } from '../home-fixtures.js';
import { deepActiveElement, deepQueryAll, removeShownToasts, shownToasts, visibleText } from '../shadow-dom.js';

type ProjectApi = UiGenApi['project'];

const LOCATION = 'E:\\projets';
const REFUSED_MESSAGE = "La demande a été refusée. Rien n'a été modifié.";
const REFUSED_NOTICE = { tone: 'danger', title: 'Action impossible', message: REFUSED_MESSAGE };
const EXISTING_FOLDER = 'E:\\projets\\kit-horizon';

const mounted: HTMLElement[] = [];

function apiWith(project: Partial<ProjectApi>): UiGenApi {
  const base = makeUiGenApi({ list: () => Promise.resolve(listing([])) });
  return { ...base, project: { ...base.project, ...project } };
}

function proposing(path: string): ProjectApi['defaultLocation'] {
  return vi.fn<ProjectApi['defaultLocation']>(() => Promise.resolve(path));
}

function creationResolving(outcome: CreateProjectOutcome): ProjectApi['create'] {
  return vi.fn<ProjectApi['create']>(() => Promise.resolve(outcome));
}

function closedChannel(): Promise<never> {
  return Promise.reject(new Error('IPC channel closed'));
}

async function mountDialog(api: UiGenApi): Promise<UiCreateDialog> {
  const dialog = new UiCreateDialog();
  dialog.api = api;
  document.body.append(dialog);
  mounted.push(dialog);
  await dialog.updateComplete;
  return dialog;
}

function recordEvents(target: HTMLElement, type: string): unknown[] {
  const details: unknown[] = [];
  target.addEventListener(type, (event: Event) => {
    if (event instanceof CustomEvent) {
      const detail: unknown = event.detail;
      details.push(detail);
    }
  });
  return details;
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

function alertTexts(): string[] {
  return createDialog()
    .getByRole('alert')
    .elements()
    .map((element) => visibleText(element));
}

function locationErrorText(): string {
  return visibleText(deepQueryAll('#cr-loc-err')[0] ?? null);
}

async function openFilled(api: UiGenApi): Promise<UiCreateDialog> {
  const dialog = await mountDialog(api);
  await dialog.open(null);
  await field('Nom du projet').fill('Kit Horizon');
  return dialog;
}

async function failToCreateOver(entryCount: number): Promise<void> {
  const create = creationResolving({ kind: 'folder-not-empty', path: EXISTING_FOLDER, entryCount });
  await openFilled(apiWith({ defaultLocation: proposing(LOCATION), create }));
  await button('Créer le projet').click();
  await expect.poll(locationErrorText).not.toBe('');
}

beforeEach(() => {
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(() => {
  for (const element of mounted.splice(0)) {
    element.remove();
  }
  removeShownToasts();
  vi.restoreAllMocks();
});

describe('<ui-create-dialog> refused requests', () => {
  it('asks for the refusal notice, stays closed and gives the focus back to the trigger when the location cannot be proposed', async () => {
    const dialog = await mountDialog(apiWith({ defaultLocation: closedChannel }));
    const notices = recordEvents(dialog, NOTICE_REQUESTED);
    const trigger = document.createElement('button');
    document.body.append(trigger);
    mounted.push(trigger);

    await dialog.open(trigger);

    expect(notices).toEqual([{ notice: REFUSED_NOTICE }]);
    await expect.element(createDialog()).not.toBeInTheDocument();
    expect(deepActiveElement()).toBe(trigger);
  });

  it('says the request was refused under the location, keeps the form and gives the focus back to « Parcourir… » when the folder choice is refused', async () => {
    const choosing = Promise.withResolvers<ChooseLocationOutcome>();
    const chooseLocation = vi.fn<ProjectApi['chooseLocation']>(() => choosing.promise);
    const dialog = await openFilled(apiWith({ defaultLocation: proposing(LOCATION), chooseLocation }));
    const notices = recordEvents(dialog, NOTICE_REQUESTED);
    await button('Parcourir…').click();
    await field('Nom du projet').click();

    choosing.reject(new Error('IPC channel closed'));

    await expect.poll(locationErrorText).toBe(REFUSED_MESSAGE);
    await expect.poll(alertTexts).toEqual([REFUSED_MESSAGE]);
    await expect.element(createDialog()).toBeVisible();
    await expect.element(field('Emplacement')).toHaveValue(LOCATION);
    await expect.element(field('Nom du projet')).toHaveValue('Kit Horizon');
    await expect.poll(() => deepActiveElement()).toBe(button('Parcourir…').element());
    expect(notices).toEqual([]);
  });

  it('says the request was refused under the location, keeps the dialog open and the form filled, and gives the focus back to « Créer le projet » when the creation is refused', async () => {
    const creating = Promise.withResolvers<CreateProjectOutcome>();
    const create = vi.fn<ProjectApi['create']>(() => creating.promise);
    const dialog = await openFilled(apiWith({ defaultLocation: proposing(LOCATION), create }));
    const notices = recordEvents(dialog, NOTICE_REQUESTED);
    const created = recordEvents(dialog, PROJECT_CREATED);
    await button('Créer le projet').click();
    await field('Nom du projet').click();

    creating.reject(new Error('IPC channel closed'));

    await expect.poll(locationErrorText).toBe(REFUSED_MESSAGE);
    await expect.poll(alertTexts).toEqual([REFUSED_MESSAGE]);
    await expect.element(createDialog()).toBeVisible();
    await expect.element(button('Créer le projet')).toBeEnabled();
    await expect.element(button('Créer le projet')).not.toHaveAttribute('aria-busy', 'true');
    await expect.element(button('Annuler')).toBeEnabled();
    await expect.element(field('Nom du projet')).toHaveValue('Kit Horizon');
    await expect.poll(() => deepActiveElement()).toBe(button('Créer le projet').element());
    expect(notices).toEqual([]);
    expect(created).toEqual([]);
  });

  it('asks to choose the location again when the studio did not grant it', async () => {
    const refused: CreateProjectOutcome = {
      kind: 'invalid-request',
      errors: [{ field: 'location', code: 'not-granted' }],
    };
    const dialog = await openFilled(apiWith({ defaultLocation: proposing(LOCATION), create: creationResolving(refused) }));
    const created = recordEvents(dialog, PROJECT_CREATED);

    await button('Créer le projet').click();

    await expect
      .poll(locationErrorText)
      .toBe("Choisissez l'emplacement avec Parcourir…, puis corrigez les champs signalés.");
    await expect.element(createDialog()).toBeVisible();
    await expect.element(button('Créer le projet')).toBeEnabled();
    expect(created).toEqual([]);
  });
});

describe('<ui-create-dialog> announced errors', () => {
  it('announces the error under the location as an alert, and keeps the field errors silent', async () => {
    const create = creationResolving({ kind: 'folder-not-empty', path: EXISTING_FOLDER, entryCount: 214 });
    await openFilled(apiWith({ defaultLocation: proposing(LOCATION), create }));
    expect(alertTexts()).toEqual([]);

    await field('Préfixe des balises').fill('kh');
    await expect.element(createDialog().getByText('Ajoutez un tiret à la fin : kh-.', { exact: true })).toBeVisible();
    expect(alertTexts()).toEqual([]);

    await field('Préfixe des balises').fill('kh-');
    await button('Créer le projet').click();

    await expect
      .poll(alertTexts)
      .toEqual([
        "Le dossier E:\\projets\\kit-horizon existe déjà et contient 214 fichiers. Rien n'a été créé. Changez le nom du projet ou l'emplacement.",
      ]);
  });

  it('announces the failed write under the location too', async () => {
    const create = creationResolving({ kind: 'write-failed', path: 'C:\\Program Files\\kit-horizon', code: 'EPERM' });
    await openFilled(apiWith({ defaultLocation: proposing('C:\\Program Files'), create }));

    await button('Créer le projet').click();

    await expect
      .poll(alertTexts)
      .toEqual([
        "Le projet n'a pas pu être créé dans C:\\Program Files\\kit-horizon (EPERM). Aucun dossier ni fichier n'a été laissé sur le disque. Choisissez un autre emplacement.",
      ]);
  });
});

describe('<ui-create-dialog> existing folder error', () => {
  const counts: readonly (readonly [number, string])[] = [
    [1, '1 fichier'],
    [2, '2 fichiers'],
  ];

  // covers: FCT-0004
  it.each(counts)('counts %i entry in the existing folder message as « %s »', async (entryCount, counted) => {
    await failToCreateOver(entryCount);

    expect(locationErrorText()).toBe(
      `Le dossier E:\\projets\\kit-horizon existe déjà et contient ${counted}. Rien n'a été créé. Changez le nom du projet ou l'emplacement.`,
    );
  });

  // covers: FCT-0004
  it('clears the error as soon as the package name changes', async () => {
    await failToCreateOver(214);

    await field('Nom du package npm').fill('@horizon/ui');

    await expect.poll(locationErrorText).toBe('');
    await expect.element(createDialog().getByText('E:\\projets\\horizon', { exact: true })).toBeVisible();
  });

  // covers: FCT-0004
  it('clears the error when the package proposal is taken back', async () => {
    const create = creationResolving({ kind: 'folder-not-empty', path: 'E:\\projets\\horizon', entryCount: 3 });
    await openFilled(apiWith({ defaultLocation: proposing(LOCATION), create }));
    await field('Nom du package npm').fill('@horizon/ui');
    await button('Créer le projet').click();
    await expect.poll(locationErrorText).not.toBe('');

    await button('Reprendre la proposition').click();

    await expect.poll(locationErrorText).toBe('');
    await expect.element(field('Nom du package npm')).toHaveValue('@kit-horizon/ui');
  });
});

describe('<ui-create-dialog> successful creation', () => {
  it('keeps « Création… » on the button until the dialog is closed', async () => {
    const create = creationResolving({ kind: 'created', project: kitHorizonProject() });
    const dialog = await mountDialog(apiWith({ defaultLocation: proposing(LOCATION), create }));
    const labelsWhenCreated: unknown[] = [];
    dialog.addEventListener(PROJECT_CREATED, () => {
      const submit = deepQueryAll('#cr-submit')[0];
      labelsWhenCreated.push({ label: visibleText(submit ?? null), busy: submit?.getAttribute('aria-busy') });
    });
    await dialog.open(null);
    await field('Nom du projet').fill('Kit Horizon');

    await button('Créer le projet').click();

    await expect.poll(() => labelsWhenCreated).toEqual([{ label: 'Création…', busy: 'true' }]);
    await expect.element(createDialog()).not.toBeInTheDocument();
  });
});

describe('<ui-home> create dialog refusals', () => {
  async function mountHome(api: UiGenApi): Promise<void> {
    const home = new UiHome();
    home.api = api;
    document.body.append(home);
    mounted.push(home);
    await home.updateComplete;
  }

  it('shows the refusal notice on the home screen and gives the focus back to the button that was clicked when the location cannot be proposed', async () => {
    const proposal = Promise.withResolvers<string>();
    await mountHome(apiWith({ defaultLocation: () => proposal.promise }));
    const createButton = page.getByRole('button', { name: 'Créer un projet', exact: true }).first();
    await createButton.click();
    await page.getByRole('button', { name: 'Ouvrir un projet', exact: true }).first().click();

    proposal.reject(new Error('IPC channel closed'));

    await expect.poll(() => shownToasts()).toEqual([REFUSED_NOTICE]);
    await expect.poll(() => deepActiveElement()).toBe(createButton.element());
    await expect.element(createDialog()).not.toBeInTheDocument();
  });

  it('shows no notice on the home screen when a request is refused while the dialog is open, the message stays under the location', async () => {
    const creating = Promise.withResolvers<CreateProjectOutcome>();
    await mountHome(apiWith({ defaultLocation: proposing(LOCATION), create: () => creating.promise }));
    await page.getByRole('button', { name: 'Créer un projet', exact: true }).first().click();
    await field('Nom du projet').fill('Kit Horizon');
    await button('Créer le projet').click();

    creating.reject(new Error('IPC channel closed'));

    await expect.poll(locationErrorText).toBe(REFUSED_MESSAGE);
    expect(shownToasts()).toEqual([]);
    await expect.element(createDialog()).toBeVisible();
  });
});
