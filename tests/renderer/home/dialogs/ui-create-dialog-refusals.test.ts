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
const REFUSED = {
  tone: 'danger',
  title: 'Action impossible',
  message: "La demande a été refusée. Rien n'a été modifié.",
};

const mounted: HTMLElement[] = [];

function apiWith(project: Partial<ProjectApi>): UiGenApi {
  const base = makeUiGenApi({ list: () => Promise.resolve(listing([])) });
  return { ...base, project: { ...base.project, ...project } };
}

function proposing(path: string): ProjectApi['defaultLocation'] {
  return vi.fn<ProjectApi['defaultLocation']>(() => Promise.resolve(path));
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

function locationErrorText(): string {
  return visibleText(deepQueryAll('#cr-loc-err')[0] ?? null);
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

    expect(notices).toEqual([{ notice: REFUSED }]);
    await expect.element(createDialog()).not.toBeInTheDocument();
    expect(deepActiveElement()).toBe(trigger);
  });

  it('keeps the form and gives the focus back to « Parcourir… » when the folder choice is refused', async () => {
    const choosing = Promise.withResolvers<ChooseLocationOutcome>();
    const chooseLocation = vi.fn<ProjectApi['chooseLocation']>(() => choosing.promise);
    const dialog = await mountDialog(apiWith({ defaultLocation: proposing(LOCATION), chooseLocation }));
    const notices = recordEvents(dialog, NOTICE_REQUESTED);
    await dialog.open(null);
    await field('Nom du projet').fill('Kit Horizon');
    await button('Parcourir…').click();
    await field('Nom du projet').click();

    choosing.reject(new Error('IPC channel closed'));

    await expect.poll(() => notices).toEqual([{ notice: REFUSED }]);
    await expect.element(createDialog()).toBeVisible();
    await expect.element(field('Emplacement')).toHaveValue(LOCATION);
    await expect.element(field('Nom du projet')).toHaveValue('Kit Horizon');
    await expect.poll(() => deepActiveElement()).toBe(button('Parcourir…').element());
  });

  it('keeps the dialog open and the form filled, and gives the focus back to « Créer le projet » when the creation is refused', async () => {
    const creating = Promise.withResolvers<CreateProjectOutcome>();
    const create = vi.fn<ProjectApi['create']>(() => creating.promise);
    const dialog = await mountDialog(apiWith({ defaultLocation: proposing(LOCATION), create }));
    const notices = recordEvents(dialog, NOTICE_REQUESTED);
    const created = recordEvents(dialog, PROJECT_CREATED);
    await dialog.open(null);
    await field('Nom du projet').fill('Kit Horizon');
    await button('Créer le projet').click();
    await field('Nom du projet').click();

    creating.reject(new Error('IPC channel closed'));

    await expect.poll(() => notices).toEqual([{ notice: REFUSED }]);
    await expect.element(createDialog()).toBeVisible();
    await expect.element(button('Créer le projet')).toBeEnabled();
    await expect.element(button('Créer le projet')).not.toHaveAttribute('aria-busy', 'true');
    await expect.element(button('Annuler')).toBeEnabled();
    await expect.element(field('Nom du projet')).toHaveValue('Kit Horizon');
    expect(locationErrorText()).toBe('');
    await expect.poll(() => deepActiveElement()).toBe(button('Créer le projet').element());
    expect(created).toEqual([]);
  });

  it('asks to choose the location again when the studio did not grant it', async () => {
    const refused: CreateProjectOutcome = {
      kind: 'invalid-request',
      errors: [{ field: 'location', code: 'not-granted' }],
    };
    const create = vi.fn<ProjectApi['create']>(() => Promise.resolve(refused));
    const dialog = await mountDialog(apiWith({ defaultLocation: proposing(LOCATION), create }));
    const created = recordEvents(dialog, PROJECT_CREATED);
    await dialog.open(null);
    await field('Nom du projet').fill('Kit Horizon');

    await button('Créer le projet').click();

    await expect
      .poll(locationErrorText)
      .toBe("Choisissez l'emplacement avec Parcourir…, puis corrigez les champs signalés.");
    await expect.element(createDialog()).toBeVisible();
    await expect.element(button('Créer le projet')).toBeEnabled();
    expect(created).toEqual([]);
  });
});

describe('<ui-create-dialog> successful creation', () => {
  it('keeps « Création… » on the button until the dialog is closed', async () => {
    const create = vi.fn<ProjectApi['create']>(() =>
      Promise.resolve({ kind: 'created', project: kitHorizonProject() }),
    );
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

describe('<ui-home> refused create dialog', () => {
  it('shows the refusal notice on the home screen and gives the focus back to the button that was clicked', async () => {
    const proposal = Promise.withResolvers<string>();
    const home = new UiHome();
    home.api = apiWith({ defaultLocation: () => proposal.promise });
    document.body.append(home);
    mounted.push(home);
    await home.updateComplete;
    const createButton = page.getByRole('button', { name: 'Créer un projet', exact: true }).first();
    await createButton.click();
    await page.getByRole('button', { name: 'Ouvrir un projet', exact: true }).first().click();

    proposal.reject(new Error('IPC channel closed'));

    await expect.poll(() => shownToasts()).toEqual([REFUSED]);
    await expect.poll(() => deepActiveElement()).toBe(createButton.element());
    await expect.element(createDialog()).not.toBeInTheDocument();
  });
});
