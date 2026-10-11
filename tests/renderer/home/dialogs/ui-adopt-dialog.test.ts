import { page, userEvent } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type {
  AdoptionProposal,
  OpenProjectOutcome,
} from '../../../../src/modules/project/application/open/project-outcomes.js';
import type { UiGenApi } from '../../../../src/preload/ui-gen-api.js';
import { UiAdoptDialog } from '../../../../src/renderer/home/dialogs/ui-adopt-dialog.js';
import { NOTICE_REQUESTED, PROJECT_ADOPTED, type ProjectAdoptedDetail } from '../../../../src/renderer/studio-events.js';
import {
  apiWith,
  ATLAS_PATH,
  atlasProject,
  atlasProposal,
  closedChannel,
  recordDetails,
  REFUSED_MESSAGE,
  type ProjectApi,
} from '../opening/opening-fixtures.js';
import { deepActiveElement, deepQueryAll, removeShownToasts, shownToasts, visibleText } from '../shadow-dom.js';

const AFTER_CONFIRMATION = 'Après confirmation, UI-GEN ajoute seulement ce fichier à la racine du dossier. Annuler ne modifie rien.';
const DIR_REFUSED = 'Indiquez un chemin relatif dans le dossier du projet, par exemple src/components.';

const mounted: HTMLElement[] = [];

async function mountDialog(api: UiGenApi): Promise<UiAdoptDialog> {
  const dialog = new UiAdoptDialog();
  dialog.api = api;
  document.body.append(dialog);
  mounted.push(dialog);
  await dialog.updateComplete;
  return dialog;
}

function mountTrigger(): HTMLButtonElement {
  const trigger = document.createElement('button');
  trigger.textContent = 'Ouvrir un projet';
  document.body.append(trigger);
  mounted.push(trigger);
  return trigger;
}

async function openDialog(api: UiGenApi, proposal: AdoptionProposal = atlasProposal()): Promise<UiAdoptDialog> {
  const dialog = await mountDialog(api);
  await dialog.open(proposal, null);
  await dialog.updateComplete;
  return dialog;
}

function adoptResolving(outcome: OpenProjectOutcome) {
  return vi.fn<ProjectApi['adopt']>(() => Promise.resolve(outcome));
}

function adoptDialog(name = 'Ouvrir Atlas avec UI-GEN ?') {
  return page.getByRole('dialog', { name });
}

function field(label: string) {
  return adoptDialog().getByRole('textbox', { name: label, exact: true });
}

function button(label: string) {
  return adoptDialog().getByRole('button', { name: label, exact: true });
}

function textOf(selector: string): string {
  return visibleText(deepQueryAll(selector)[0] ?? null);
}

function factValue(term: string): string {
  const terms = deepQueryAll('#dlg-adopt dt').filter((element) => visibleText(element) === term);
  return visibleText(terms[0]?.nextElementSibling ?? null);
}

function alertTexts(): string[] {
  return adoptDialog()
    .getByRole('alert')
    .elements()
    .map((element) => visibleText(element));
}

function adoptionErrorText(): string {
  return textOf('#ad-err');
}

async function confirmFailingWith(outcome: Promise<OpenProjectOutcome>): Promise<UiAdoptDialog> {
  const adopt = vi.fn<ProjectApi['adopt']>(() => outcome);
  const dialog = await openDialog(apiWith({ adopt }));
  await button('Ajouter uigen.json et ouvrir').click();
  await expect.poll(adoptionErrorText).not.toBe('');
  return dialog;
}

afterEach(() => {
  for (const element of mounted.splice(0)) {
    element.remove();
  }
  removeShownToasts();
  vi.restoreAllMocks();
});

describe('<ui-adopt-dialog> proposal', () => {
  // covers: FCT-0007
  // covers: FCT-0021
  it('presents the library, the detected prefix and components folder, and the uigen.json it would add', async () => {
    const adopt = vi.fn<ProjectApi['adopt']>(closedChannel);
    await openDialog(apiWith({ adopt }));

    await expect.element(adoptDialog()).toBeVisible();
    expect(deepQueryAll('dialog#dlg-adopt')).toHaveLength(1);
    await expect
      .element(
        adoptDialog().getByText(
          "Cette bibliothèque Lit n'a pas été créée avec UI-GEN. Vérifiez le préfixe et le dossier des composants détectés avant de l'ouvrir.",
          { exact: true },
        ),
      )
      .toBeVisible();
    expect(factValue('Dossier')).toBe(ATLAS_PATH);
    expect(factValue('Package')).toBe('@atlas/components 2.3.1');
    expect(factValue('Composants Lit')).toBe('12 trouvés dans src/components');
    await expect.element(field('Préfixe des balises')).toHaveValue('atlas-');
    await expect.element(field('Dossier des composants')).toHaveValue('src/components');
    await expect
      .element(adoptDialog().getByText('Détecté dans les 12 composants, par exemple <atlas-button>.', { exact: true }))
      .toBeVisible();
    await expect
      .element(adoptDialog().getByText('Dossier qui contient les composants détectés.', { exact: true }))
      .toBeVisible();
    await expect.element(adoptDialog().getByText('Nouveau fichier', { exact: true })).toBeVisible();
    expect(textOf('#ad-json-value')).toBe('"atlas-"');
    expect(textOf('#ad-json-dir-value')).toBe('"src/components"');
    await expect.element(adoptDialog().getByText(AFTER_CONFIRMATION, { exact: true })).toBeVisible();
    await expect.element(button('Annuler')).toBeEnabled();
    await expect.element(button('Ajouter uigen.json et ouvrir')).toBeEnabled();
    expect(adopt).not.toHaveBeenCalled();
  });

  // covers: FCT-0007
  it('puts the focus on the proposed prefix, fully selected, so that typing replaces it', async () => {
    await openDialog(apiWith({}));

    const prefix = field('Préfixe des balises');
    await expect.poll(() => deepActiveElement()).toBe(prefix.element());
    const input = prefix.element();
    expect(input).toBeInstanceOf(HTMLInputElement);
    if (input instanceof HTMLInputElement) {
      expect(input.selectionStart).toBe(0);
      expect(input.selectionEnd).toBe('atlas-'.length);
    }

    await userEvent.keyboard('at-');

    await expect.element(prefix).toHaveValue('at-');
  });

  // covers: FCT-0021
  it('proposes another detected components folder', async () => {
    await openDialog(apiWith({}), { ...atlasProposal(), componentCount: 4, componentsDir: 'src/lib' });

    await expect.element(field('Dossier des composants')).toHaveValue('src/lib');
    expect(factValue('Composants Lit')).toBe('4 trouvés dans src/lib');
    expect(textOf('#ad-json-dir-value')).toBe('"src/lib"');
  });

  // covers: FCT-0007
  it('asks for the prefix when no component was detected, and cannot be confirmed until one is given', async () => {
    await openDialog(apiWith({}), {
      ...atlasProposal(),
      componentCount: 0,
      tagPrefix: null,
      exampleTag: null,
    });

    await expect.element(field('Préfixe des balises')).toHaveValue('');
    await expect
      .element(adoptDialog().getByText('Aucun composant détecté : indiquez le préfixe de vos balises.', { exact: true }))
      .toBeVisible();
    await expect.element(button('Ajouter uigen.json et ouvrir')).toBeDisabled();

    await field('Préfixe des balises').fill('at-');

    await expect.element(button('Ajouter uigen.json et ouvrir')).toBeEnabled();
    expect(textOf('#ad-json-value')).toBe('"at-"');
  });

  // covers: FCT-0007
  // covers: FCT-0021
  it('rewrites the uigen.json preview as the prefix and the folder are typed', async () => {
    await openDialog(apiWith({}));

    await field('Préfixe des balises').fill('at-');
    await field('Dossier des composants').fill('packages/ui/src');

    await expect.poll(() => textOf('#ad-json-value')).toBe('"at-"');
    await expect.poll(() => textOf('#ad-json-dir-value')).toBe('"packages/ui/src"');
  });
});

describe('<ui-adopt-dialog> refused values', () => {
  // covers: FCT-0007
  it('explains an uppercase prefix and cannot be confirmed', async () => {
    await openDialog(apiWith({}));

    await field('Préfixe des balises').fill('Atlas-');

    await expect
      .element(adoptDialog().getByText("Le préfixe s'écrit en minuscules : atlas- au lieu de Atlas-.", { exact: true }))
      .toBeVisible();
    await expect.element(field('Préfixe des balises')).toHaveAttribute('aria-invalid', 'true');
    await expect.element(button('Ajouter uigen.json et ouvrir')).toBeDisabled();
  });

  // covers: FCT-0007
  it('asks for a prefix, with the detected one as example, when the field is emptied', async () => {
    await openDialog(apiWith({}));

    await field('Préfixe des balises').fill('');

    await expect
      .element(adoptDialog().getByText('Indiquez un préfixe, par exemple atlas-.', { exact: true }))
      .toBeVisible();
    await expect.element(button('Ajouter uigen.json et ouvrir')).toBeDisabled();
  });

  // covers: FCT-0007
  it('asks for the trailing dash and accepts the prefix once it is added', async () => {
    await openDialog(apiWith({}));

    await field('Préfixe des balises').fill('at');
    await expect.element(adoptDialog().getByText('Ajoutez un tiret à la fin : at-.', { exact: true })).toBeVisible();
    await expect.element(button('Ajouter uigen.json et ouvrir')).toBeDisabled();

    await field('Préfixe des balises').fill('at-');

    await expect.element(adoptDialog().getByText('Ajoutez un tiret à la fin : at-.', { exact: true })).not.toBeInTheDocument();
    await expect.element(field('Préfixe des balises')).not.toHaveAttribute('aria-invalid', 'true');
    await expect.element(button('Ajouter uigen.json et ouvrir')).toBeEnabled();
  });

  // covers: FCT-0021
  it.each(['', '../shared', 'src\\components', 'C:/lib', '/src/components', 'src//components', 'src/components/'])(
    'refuses the components folder %j and cannot be confirmed',
    async (folder) => {
      await openDialog(apiWith({}));

      await field('Dossier des composants').fill(folder);

      await expect.element(adoptDialog().getByText(DIR_REFUSED, { exact: true })).toBeVisible();
      await expect.element(field('Dossier des composants')).toHaveAttribute('aria-invalid', 'true');
      await expect.element(button('Ajouter uigen.json et ouvrir')).toBeDisabled();
    },
  );

  // covers: FCT-0021
  it('accepts the project root and a nested relative folder', async () => {
    await openDialog(apiWith({}));

    await field('Dossier des composants').fill('.');
    await expect.element(button('Ajouter uigen.json et ouvrir')).toBeEnabled();
    await field('Dossier des composants').fill('packages/ui/src');

    await expect.element(adoptDialog().getByText(DIR_REFUSED, { exact: true })).not.toBeInTheDocument();
    await expect.element(button('Ajouter uigen.json et ouvrir')).toBeEnabled();
  });
});

describe('<ui-adopt-dialog> cancel', () => {
  // covers: FCT-0008
  it('closes on « Annuler » without writing anything nor announcing an adoption, and gives the focus back to the trigger', async () => {
    const adopt = vi.fn<ProjectApi['adopt']>(closedChannel);
    const dialog = await mountDialog(apiWith({ adopt }));
    const adopted = recordDetails(dialog, PROJECT_ADOPTED);
    const trigger = mountTrigger();
    await dialog.open(atlasProposal(), trigger);
    await field('Préfixe des balises').fill('at-');

    await button('Annuler').click();

    await expect.element(adoptDialog()).not.toBeInTheDocument();
    expect(adopt).not.toHaveBeenCalled();
    expect(adopted).toEqual([]);
    await expect.poll(() => deepActiveElement()).toBe(trigger);
  });

  // covers: FCT-0008
  it('closes on Escape without writing anything', async () => {
    const adopt = vi.fn<ProjectApi['adopt']>(closedChannel);
    const dialog = await mountDialog(apiWith({ adopt }));
    const adopted = recordDetails(dialog, PROJECT_ADOPTED);
    const trigger = mountTrigger();
    await dialog.open(atlasProposal(), trigger);
    await expect.poll(() => deepActiveElement()).toBe(field('Préfixe des balises').element());

    await userEvent.keyboard('{Escape}');

    await expect.element(adoptDialog()).not.toBeInTheDocument();
    expect(adopt).not.toHaveBeenCalled();
    expect(adopted).toEqual([]);
    await expect.poll(() => deepActiveElement()).toBe(trigger);
  });

  // covers: FCT-0008
  it('starts again from the new proposal when opened after a cancel', async () => {
    const dialog = await openDialog(apiWith({}));
    await field('Préfixe des balises').fill('Atlas');
    await field('Dossier des composants').fill('../shared');
    await button('Annuler').click();
    await expect.element(adoptDialog()).not.toBeInTheDocument();

    await dialog.open({ ...atlasProposal(), componentsDir: 'src/lib' }, null);

    await expect.element(field('Préfixe des balises')).toHaveValue('atlas-');
    await expect.element(field('Dossier des composants')).toHaveValue('src/lib');
    await expect.element(adoptDialog().getByText(DIR_REFUSED, { exact: true })).not.toBeInTheDocument();
    await expect.element(button('Ajouter uigen.json et ouvrir')).toBeEnabled();
  });
});

describe('<ui-adopt-dialog> confirmation', () => {
  // covers: FCT-0007
  // covers: FCT-0021
  it('writes uigen.json with the proposals only once confirmed, busy meanwhile, then closes and announces project-adopted', async () => {
    const adopting = Promise.withResolvers<OpenProjectOutcome>();
    const adopt = vi.fn<ProjectApi['adopt']>(() => adopting.promise);
    const dialog = await openDialog(apiWith({ adopt }));
    const adopted = recordDetails(dialog, PROJECT_ADOPTED);
    expect(adopt).not.toHaveBeenCalled();

    await button('Ajouter uigen.json et ouvrir').click();

    expect(adopt).toHaveBeenCalledTimes(1);
    expect(adopt).toHaveBeenCalledWith({ path: ATLAS_PATH, tagPrefix: 'atlas-', componentsDir: 'src/components' });
    await expect.element(button('Écriture de uigen.json…')).toHaveAttribute('aria-busy', 'true');
    expect(adopted).toEqual([]);

    adopting.resolve({ kind: 'opened', project: atlasProject('atlas-', 'src/components') });

    await expect.poll(() => adopted).toHaveLength(1);
    const expected: ProjectAdoptedDetail = { project: atlasProject('atlas-', 'src/components') };
    expect(adopted[0]).toEqual(expected);
    await expect.element(adoptDialog()).not.toBeInTheDocument();
  });

  // covers: FCT-0007
  // covers: FCT-0021
  it('writes the prefix and the folder the user changed', async () => {
    const adopt = adoptResolving({ kind: 'opened', project: atlasProject('at-', 'src/lib') });
    const dialog = await openDialog(apiWith({ adopt }));
    const adopted = recordDetails(dialog, PROJECT_ADOPTED);

    await field('Préfixe des balises').fill('at-');
    await field('Dossier des composants').fill('src/lib');
    await button('Ajouter uigen.json et ouvrir').click();

    await expect.poll(() => adopted).toHaveLength(1);
    expect(adopt).toHaveBeenCalledTimes(1);
    expect(adopt).toHaveBeenCalledWith({ path: ATLAS_PATH, tagPrefix: 'at-', componentsDir: 'src/lib' });
  });

  // covers: FCT-0007
  it('sends a single request when confirmed twice while writing', async () => {
    const adopting = Promise.withResolvers<OpenProjectOutcome>();
    const adopt = vi.fn<ProjectApi['adopt']>(() => adopting.promise);
    const dialog = await openDialog(apiWith({ adopt }));
    const adopted = recordDetails(dialog, PROJECT_ADOPTED);

    await button('Ajouter uigen.json et ouvrir').click();
    await expect.element(button('Écriture de uigen.json…')).toHaveAttribute('aria-busy', 'true');
    const busyButton = button('Écriture de uigen.json…').element();
    if (busyButton instanceof HTMLElement) busyButton.click();
    const form = busyButton.closest('dialog')?.querySelector('form');
    form?.requestSubmit();
    adopting.resolve({ kind: 'opened', project: atlasProject('atlas-', 'src/components') });

    await expect.poll(() => adopted).toHaveLength(1);
    expect(adopt).toHaveBeenCalledTimes(1);
  });
});

describe('<ui-adopt-dialog> failed adoption', () => {
  it('explains a failed write under the form, keeps the dialog open with the typed values, and never shows a toast', async () => {
    const adopt = adoptResolving({ kind: 'write-failed', path: ATLAS_PATH, code: 'EPERM' });
    const dialog = await openDialog(apiWith({ adopt }));
    const adopted = recordDetails(dialog, PROJECT_ADOPTED);
    const notices = recordDetails(dialog, NOTICE_REQUESTED);
    await field('Préfixe des balises').fill('at-');

    await button('Ajouter uigen.json et ouvrir').click();

    const message = `uigen.json n'a pas pu être écrit dans ${ATLAS_PATH} (EPERM). Rien n'a été modifié.`;
    await expect.poll(adoptionErrorText).toBe(message);
    await expect.poll(alertTexts).toEqual([message]);
    const error = deepQueryAll('#ad-err')[0];
    expect(error?.tagName).toBe('P');
    expect(error?.classList.contains('ui-field__error')).toBe(true);
    expect(error?.getAttribute('role')).toBe('alert');
    await expect.element(adoptDialog()).toBeVisible();
    await expect.element(field('Préfixe des balises')).toHaveValue('at-');
    await expect.element(field('Dossier des composants')).toHaveValue('src/components');
    await expect.element(button('Ajouter uigen.json et ouvrir')).not.toHaveAttribute('aria-busy', 'true');
    expect(adopted).toEqual([]);
    expect(notices).toEqual([]);
    expect(shownToasts()).toEqual([]);
  });

  it('places the failure after the sentence about what the confirmation adds', async () => {
    await confirmFailingWith(Promise.resolve({ kind: 'write-failed', path: ATLAS_PATH, code: 'EACCES' }));

    const sentence = deepQueryAll('#dlg-adopt p').find((element) => visibleText(element) === AFTER_CONFIRMATION);
    const error = deepQueryAll('#ad-err')[0];
    expect(sentence).toBeDefined();
    expect(error).toBeDefined();
    if (sentence !== undefined && error !== undefined) {
      expect(sentence.compareDocumentPosition(error) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    }
  });

  it('says under the form that the request was refused when the main process refuses the values', async () => {
    const dialog = await confirmFailingWith(
      Promise.resolve({ kind: 'invalid-request', errors: [{ field: 'tagPrefix', code: 'double-dash' }] }),
    );
    const notices = recordDetails(dialog, NOTICE_REQUESTED);

    expect(adoptionErrorText()).toBe(REFUSED_MESSAGE);
    await expect.poll(alertTexts).toEqual([REFUSED_MESSAGE]);
    await expect.element(adoptDialog()).toBeVisible();
    expect(notices).toEqual([]);
    expect(shownToasts()).toEqual([]);
  });

  it('says under the form that the request was refused when the IPC call is rejected, and keeps the dialog usable', async () => {
    const adopt = vi
      .fn<ProjectApi['adopt']>()
      .mockImplementationOnce(closedChannel)
      .mockResolvedValueOnce({ kind: 'opened', project: atlasProject('atlas-', 'src/components') });
    const dialog = await openDialog(apiWith({ adopt }));
    const adopted = recordDetails(dialog, PROJECT_ADOPTED);
    const notices = recordDetails(dialog, NOTICE_REQUESTED);

    await button('Ajouter uigen.json et ouvrir').click();

    await expect.poll(adoptionErrorText).toBe(REFUSED_MESSAGE);
    await expect.poll(alertTexts).toEqual([REFUSED_MESSAGE]);
    expect(notices).toEqual([]);
    expect(shownToasts()).toEqual([]);
    await expect.element(button('Ajouter uigen.json et ouvrir')).toBeEnabled();

    await button('Ajouter uigen.json et ouvrir').click();

    await expect.poll(() => adopted).toHaveLength(1);
    expect(adopt).toHaveBeenCalledTimes(2);
  });

  it('says under the form that the folder was moved or deleted', async () => {
    await confirmFailingWith(Promise.resolve({ kind: 'missing', path: ATLAS_PATH }));

    expect(adoptionErrorText()).toBe(`Le dossier ${ATLAS_PATH} a été déplacé ou supprimé.`);
    await expect.element(adoptDialog()).toBeVisible();
    expect(shownToasts()).toEqual([]);
  });

  // covers: FCT-0008
  it('forgets the failure when the dialog is opened again', async () => {
    const dialog = await confirmFailingWith(Promise.resolve({ kind: 'write-failed', path: ATLAS_PATH, code: 'EPERM' }));
    await button('Annuler').click();
    await expect.element(adoptDialog()).not.toBeInTheDocument();

    await dialog.open(atlasProposal(), null);

    await expect.element(adoptDialog()).toBeVisible();
    expect(adoptionErrorText()).toBe('');
    expect(alertTexts()).toEqual([]);
  });
});
