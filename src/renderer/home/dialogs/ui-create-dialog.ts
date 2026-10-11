import { html, LitElement, nothing, type TemplateResult } from 'lit';
import { customElement, property, query, state } from 'lit/decorators.js';
import type { CreateProjectOutcome } from '../../../modules/project/application/create/create-project.js';
import type { ChooseLocationOutcome, UiGenApi } from '../../../preload/ui-gen-api.js';
import { icon } from '../../icons/icon.js';
import { dialogClose, dialogOpen, highlight } from '../../motion/motion.js';
import { hostContents, studioStyles } from '../../styles/studio-styles.js';
import {
  NOTICE_REQUESTED,
  PROJECT_CREATED,
  type NoticeRequestedDetail,
  type ProjectCreatedDetail,
} from '../../studio-events.js';
import { REQUEST_REFUSED } from '../../studio-notices.js';
import { homeStyles } from '../home-styles.js';
import { deriveCreateForm, type CreateFormInput } from './create-form.js';

declare global {
  interface HTMLElementTagNameMap {
    'ui-create-dialog': UiCreateDialog;
  }
}

const EMPTY_INPUT: CreateFormInput = {
  name: '',
  location: '',
  packageName: '',
  tagPrefix: '',
  packageEdited: false,
  prefixEdited: false,
  nameTouched: false,
};

export function fieldError(id: string, message: string | null, role?: 'alert'): TemplateResult {
  return html`<p class="ui-field__error" id=${id} role=${message === null || role === undefined ? nothing : role} ?hidden=${message === null}>${message === null ? nothing : html`${icon('erreur')}<span>${message}</span>`}</p>`;
}

function existingFolderMessage(path: string, entryCount: number): string {
  const entries = entryCount === 1 ? '1 fichier' : `${String(entryCount)} fichiers`;
  return `Le dossier ${path} existe déjà et contient ${entries}. Rien n'a été créé. Changez le nom du projet ou l'emplacement.`;
}

export function isOutside(dialog: HTMLDialogElement, event: MouseEvent): boolean {
  const rect = dialog.getBoundingClientRect();
  return event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom;
}

@customElement('ui-create-dialog')
export class UiCreateDialog extends LitElement {
  static override styles = [studioStyles, homeStyles, hostContents];

  @property({ attribute: false }) api!: UiGenApi;

  @state() private input: CreateFormInput = EMPTY_INPUT;
  @state() private locationError: string | null = null;
  @state() private busy = false;

  @query('dialog') private dialog!: HTMLDialogElement;
  @query('#cr-name') private nameField!: HTMLInputElement;
  @query('#cr-loc') private locationField!: HTMLInputElement;
  @query('#cr-pkg') private packageField!: HTMLInputElement;
  @query('#cr-prefix') private prefixField!: HTMLInputElement;
  @query('#cr-browse') private browseButton!: HTMLButtonElement;
  @query('#cr-submit') private submitButton!: HTMLButtonElement;

  async open(trigger: Element | null): Promise<void> {
    let location: string;
    try {
      location = await this.api.project.defaultLocation();
    } catch (error) {
      this.refuseOpening(error, trigger instanceof HTMLElement ? trigger : null);
      return;
    }
    this.busy = false;
    this.locationError = null;
    this.input = { ...EMPTY_INPUT, location };
    await this.updateComplete;
    await dialogOpen(this.dialog, trigger);
    this.nameField.focus();
  }

  override render(): TemplateResult {
    const view = deriveCreateForm(this.input);
    return html`
      <dialog class="ui-dialog" id="dlg-create" aria-labelledby="cr-title" aria-describedby="cr-desc" @cancel=${this.onCancel} @click=${this.onDialogClick}>
        <div class="ui-dialog__header">
          <h2 class="ui-dialog__title" id="cr-title">Créer un projet</h2>
          <button class="ui-icon-btn" type="button" aria-label="Fermer" @click=${this.onCloseClick}>${icon('fermer')}</button>
        </div>
        <form class="ui-dialog__body ac-form" id="cr-form" novalidate @submit=${(event: SubmitEvent) => void this.submit(event)}>
          <p id="cr-desc">UI-GEN crée le dossier de la bibliothèque, son <code>package.json</code> et le fichier de réglages <code>uigen.json</code>.</p>

          <div class="ui-field">
            <label class="ui-label" for="cr-name">Nom du projet</label>
            <input class="ui-input" id="cr-name" name="nom" autocomplete="off" spellcheck="false" placeholder="Par exemple Kit Horizon" aria-describedby="cr-name-err" required aria-invalid=${view.nameError === null ? nothing : 'true'} .value=${this.input.name} @input=${this.onNameInput} @blur=${this.onNameBlur}>
            ${fieldError('cr-name-err', view.nameError)}
          </div>

          <div class="ui-field">
            <label class="ui-label" for="cr-loc">Emplacement</label>
            <div class="ac-path">
              <input class="ui-input ui-input--mono" id="cr-loc" readonly aria-describedby="cr-loc-help cr-loc-err" aria-invalid=${this.locationError === null ? nothing : 'true'} .value=${this.input.location}>
              <button class="ui-btn ui-btn--secondary" type="button" id="cr-browse" @click=${() => void this.browse()}>${icon('dossier')}Parcourir…</button>
            </div>
            <p class="ui-help" id="cr-loc-help">Dossier parent. Le projet est créé dans un nouveau dossier à cet endroit.</p>
            ${fieldError('cr-loc-err', this.locationError, 'alert')}
          </div>

          <fieldset>
            <legend>Réglages de la bibliothèque</legend>
            <div class="ac-pair">
              <div class="ui-field">
                <label class="ui-label" for="cr-pkg">Nom du package npm</label>
                <input class="ui-input ui-input--mono" id="cr-pkg" autocomplete="off" spellcheck="false" placeholder="@kit-horizon/ui" aria-describedby="cr-pkg-help cr-pkg-err" aria-invalid=${view.packageError === null ? nothing : 'true'} .value=${view.packageName} @input=${this.onPackageInput}>
                <div class="ac-help-row">
                  <p class="ui-help" id="cr-pkg-help">Proposé à partir du nom.</p>
                  <button class="ui-btn ui-btn--link" type="button" id="cr-pkg-reset" ?hidden=${!view.showPackageReset} @click=${this.onPackageReset}>Reprendre la proposition</button>
                </div>
                ${fieldError('cr-pkg-err', view.packageError)}
              </div>
              <div class="ui-field">
                <label class="ui-label" for="cr-prefix">Préfixe des balises</label>
                <input class="ui-input ui-input--mono" id="cr-prefix" autocomplete="off" spellcheck="false" placeholder="kh-" aria-describedby="cr-prefix-help cr-prefix-err" aria-invalid=${view.prefixError === null ? nothing : 'true'} .value=${view.tagPrefix} @input=${this.onPrefixInput}>
                <div class="ac-help-row">
                  <p class="ui-help" id="cr-prefix-help">Initiales du nom, suivies d'un tiret.</p>
                  <button class="ui-btn ui-btn--link" type="button" id="cr-prefix-reset" ?hidden=${!view.showPrefixReset} @click=${this.onPrefixReset}>Reprendre la proposition</button>
                </div>
                ${fieldError('cr-prefix-err', view.prefixError)}
              </div>
            </div>
          </fieldset>

          <dl class="ac-preview" aria-label="Aperçu du projet" aria-live="polite">
            <div><dt>Dossier du projet</dt><dd id="cr-prev-path" class=${view.previewPath === null ? 'is-pending' : ''}>${view.previewPath ?? 'Indiquez un nom de projet'}</dd></div>
            <div><dt>Balise exemple</dt><dd id="cr-prev-tag" class=${view.previewTagPending ? 'is-pending' : ''}>${view.previewTag}</dd></div>
          </dl>
        </form>
        <div class="ui-dialog__footer">
          <button class="ui-btn ui-btn--secondary" type="button" id="cr-cancel" ?disabled=${this.busy} @click=${this.onCloseClick}>Annuler</button>
          <button class="ui-btn ui-btn--primary" type="submit" form="cr-form" id="cr-submit" ?disabled=${!view.canSubmit} aria-busy=${this.busy ? 'true' : nothing}>${this.busy ? 'Création…' : 'Créer le projet'}</button>
        </div>
      </dialog>
    `;
  }

  private async browse(): Promise<void> {
    if (this.busy) return;
    let outcome: ChooseLocationOutcome;
    try {
      outcome = await this.api.project.chooseLocation();
    } catch (error) {
      this.refuseInDialog(error, this.browseButton);
      return;
    }
    if (outcome.canceled) return;
    this.locationError = null;
    this.input = { ...this.input, location: outcome.path };
    await this.updateComplete;
    highlight([this.locationField]);
    this.browseButton.focus();
  }

  private async submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    this.input = { ...this.input, nameTouched: true };
    const view = deriveCreateForm(this.input);
    if (!view.canSubmit || this.busy) return;
    this.busy = true;
    let outcome: CreateProjectOutcome;
    try {
      outcome = await this.api.project.create({
        name: this.input.name,
        location: this.input.location,
        packageName: view.packageName,
        tagPrefix: view.tagPrefix,
      });
    } catch (error) {
      this.busy = false;
      this.refuseInDialog(error, this.submitButton);
      return;
    }
    await this.applyOutcome(outcome);
  }

  private async applyOutcome(outcome: CreateProjectOutcome): Promise<void> {
    if (outcome.kind === 'created') {
      await dialogClose(this.dialog, 'cree');
      this.dispatchEvent(new CustomEvent<ProjectCreatedDetail>(PROJECT_CREATED, { detail: { project: outcome.project } }));
      return;
    }
    this.busy = false;
    switch (outcome.kind) {
      case 'folder-not-empty':
        this.locationError = existingFolderMessage(outcome.path, outcome.entryCount);
        this.nameField.focus();
        return;
      case 'write-failed':
        this.locationError = `Le projet n'a pas pu être créé dans ${outcome.path} (${outcome.code}). Aucun dossier ni fichier n'a été laissé sur le disque. Choisissez un autre emplacement.`;
        this.browseButton.focus();
        return;
      case 'invalid-request':
        this.locationError = "Choisissez l'emplacement avec Parcourir…, puis corrigez les champs signalés.";
        return;
    }
  }

  private refuseOpening(error: unknown, trigger: HTMLElement | null): void {
    console.warn('Request refused', error);
    this.dispatchEvent(new CustomEvent<NoticeRequestedDetail>(NOTICE_REQUESTED, { detail: { notice: REQUEST_REFUSED } }));
    trigger?.focus();
  }

  private refuseInDialog(error: unknown, trigger: HTMLElement): void {
    console.warn('Request refused', error);
    this.locationError = REQUEST_REFUSED.message;
    trigger.focus();
  }

  private readonly onNameInput = (): void => {
    this.locationError = null;
    this.input = { ...this.input, name: this.nameField.value };
  };

  private readonly onNameBlur = (): void => {
    if (this.dialog.open) this.input = { ...this.input, nameTouched: true };
  };

  private readonly onPackageInput = (): void => {
    this.locationError = null;
    this.input = { ...this.input, packageName: this.packageField.value, packageEdited: true };
  };

  private readonly onPrefixInput = (): void => {
    this.input = { ...this.input, tagPrefix: this.prefixField.value, prefixEdited: true };
  };

  private readonly onPackageReset = (): void => {
    this.locationError = null;
    this.input = { ...this.input, packageEdited: false };
    void this.updateComplete.then(() => {
      this.packageField.focus();
    });
  };

  private readonly onPrefixReset = (): void => {
    this.input = { ...this.input, prefixEdited: false };
    void this.updateComplete.then(() => {
      this.prefixField.focus();
    });
  };

  private readonly onCancel = (event: Event): void => {
    event.preventDefault();
    if (!this.busy) void dialogClose(this.dialog);
  };

  private readonly onDialogClick = (event: MouseEvent): void => {
    if (event.target === this.dialog && !this.busy && isOutside(this.dialog, event)) void dialogClose(this.dialog);
  };

  private readonly onCloseClick = (): void => {
    if (!this.busy) void dialogClose(this.dialog, 'annuler');
  };
}
