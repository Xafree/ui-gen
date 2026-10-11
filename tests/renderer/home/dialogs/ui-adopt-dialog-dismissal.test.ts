import { page } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { OpenProjectOutcome } from '../../../../src/modules/project/application/open/project-outcomes.js';
import type { UiGenApi } from '../../../../src/preload/ui-gen-api.js';
import { UiAdoptDialog } from '../../../../src/renderer/home/dialogs/ui-adopt-dialog.js';
import { PROJECT_ADOPTED } from '../../../../src/renderer/studio-events.js';
import { apiWith, ATLAS_PATH, atlasProposal, closedChannel, recordDetails, type ProjectApi } from '../opening/opening-fixtures.js';
import { deepActiveElement, deepQueryAll, removeShownToasts, visibleText } from '../shadow-dom.js';

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

function adoptDialog() {
  return page.getByRole('dialog', { name: 'Ouvrir Atlas avec UI-GEN ?' });
}

function button(label: string) {
  return adoptDialog().getByRole('button', { name: label, exact: true });
}

function clickDialogAt(clientX: number, clientY: number): void {
  const dialog = deepQueryAll('dialog#dlg-adopt')[0];
  dialog?.dispatchEvent(new MouseEvent('click', { bubbles: true, clientX, clientY }));
}

function factValue(term: string): string {
  const terms = deepQueryAll('#dlg-adopt dt').filter((element) => visibleText(element) === term);
  return visibleText(terms[0]?.nextElementSibling ?? null);
}

afterEach(() => {
  for (const element of mounted.splice(0)) {
    element.remove();
  }
  removeShownToasts();
  vi.restoreAllMocks();
});

describe('<ui-adopt-dialog> dismissal', () => {
  // covers: FCT-0008
  it('closes when the backdrop is clicked, writing nothing, and gives the focus back to the trigger', async () => {
    const adopt = vi.fn<ProjectApi['adopt']>(closedChannel);
    const dialog = await mountDialog(apiWith({ adopt }));
    const adopted = recordDetails(dialog, PROJECT_ADOPTED);
    const trigger = mountTrigger();
    await dialog.open(atlasProposal(), trigger);

    clickDialogAt(1, 1);

    await expect.element(adoptDialog()).not.toBeInTheDocument();
    expect(adopt).not.toHaveBeenCalled();
    expect(adopted).toEqual([]);
    await expect.poll(() => deepActiveElement()).toBe(trigger);
  });

  // covers: FCT-0008
  it('closes with the « Fermer » button of the header and gives the focus back to the trigger', async () => {
    const adopt = vi.fn<ProjectApi['adopt']>(closedChannel);
    const dialog = await mountDialog(apiWith({ adopt }));
    const trigger = mountTrigger();
    await dialog.open(atlasProposal(), trigger);

    await button('Fermer').click();

    await expect.element(adoptDialog()).not.toBeInTheDocument();
    expect(adopt).not.toHaveBeenCalled();
    await expect.poll(() => deepActiveElement()).toBe(trigger);
  });

  it('cannot be canceled while uigen.json is being written, and can be again once the write failed', async () => {
    const adopting = Promise.withResolvers<OpenProjectOutcome>();
    const adopt = vi.fn<ProjectApi['adopt']>(() => adopting.promise);
    const dialog = await mountDialog(apiWith({ adopt }));
    await dialog.open(atlasProposal(), null);

    await button('Ajouter uigen.json et ouvrir').click();

    await expect.element(button('Écriture de uigen.json…')).toHaveAttribute('aria-busy', 'true');
    await expect.element(button('Annuler')).toBeDisabled();

    adopting.resolve({ kind: 'write-failed', path: ATLAS_PATH, code: 'EPERM' });

    await expect.element(button('Annuler')).toBeEnabled();
    await expect.element(button('Ajouter uigen.json et ouvrir')).not.toHaveAttribute('aria-busy', 'true');
  });
});

describe('<ui-adopt-dialog> answer asking for adoption again', () => {
  it('shows the library as it is now read and lets the user confirm again', async () => {
    const adopt = vi
      .fn<ProjectApi['adopt']>()
      .mockResolvedValueOnce({ kind: 'needs-adoption', proposal: { ...atlasProposal(), componentCount: 5, componentsDir: 'src/lib' } });
    const dialog = await mountDialog(apiWith({ adopt }));
    await dialog.open(atlasProposal(), null);

    await button('Ajouter uigen.json et ouvrir').click();

    await expect.poll(() => factValue('Composants Lit')).toBe('5 trouvés dans src/lib');
    await expect.element(button('Ajouter uigen.json et ouvrir')).toBeEnabled();
    await expect.element(button('Ajouter uigen.json et ouvrir')).not.toHaveAttribute('aria-busy', 'true');
    expect(adopt).toHaveBeenCalledTimes(1);
  });
});
