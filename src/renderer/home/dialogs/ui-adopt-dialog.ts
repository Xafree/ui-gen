import { html, LitElement, nothing, type TemplateResult } from 'lit';
import { customElement, property, query, state } from 'lit/decorators.js';
import type {
  AdoptionProposal,
  OpenProjectOutcome,
} from '../../../modules/project/application/open/project-outcomes.js';
import { validateTagPrefix } from '../../../modules/project/domain/naming/tag-prefix.js';
import { isValidComponentsDir } from '../../../modules/project/domain/settings/components-dir.js';
import type { UiGenApi } from '../../../preload/ui-gen-api.js';
import { icon } from '../../icons/icon.js';
import { dialogClose, dialogOpen, highlight } from '../../motion/motion.js';
import { hostContents, studioStyles } from '../../styles/studio-styles.js';
import { PROJECT_ADOPTED, type ProjectAdoptedDetail } from '../../studio-events.js';
import { REQUEST_REFUSED } from '../../studio-notices.js';
import { homeStyles } from '../home-styles.js';
import { openFailureToast } from '../open-messages.js';
import { tagPrefixMessage } from './create-form.js';
import { fieldError, isOutside } from './ui-create-dialog.js';

declare global {
  interface HTMLElementTagNameMap {
    'ui-adopt-dialog': UiAdoptDialog;
  }
}

const COMPONENTS_DIR_ERROR = 'Indiquez un chemin relatif dans le dossier du projet, par exemple src/components.';

@customElement('ui-adopt-dialog')
export class UiAdoptDialog extends LitElement {
  static override styles = [studioStyles, homeStyles, hostContents];

  @property({ attribute: false }) api!: UiGenApi;

  @state() private proposal: AdoptionProposal | null = null;
  @state() private tagPrefix = '';
  @state() private componentsDir = '';
  @state() private busy = false;
  @state() private failure: string | null = null;

  @query('dialog') private dialog!: HTMLDialogElement;
  @query('#ad-prefix') private prefixField!: HTMLInputElement;
  @query('#ad-dir') private dirField!: HTMLInputElement;

  private trigger: Element | null = null;

  async open(proposal: AdoptionProposal, trigger: Element | null): Promise<void> {
    this.proposal = proposal;
    this.tagPrefix = proposal.tagPrefix ?? '';
    this.componentsDir = proposal.componentsDir;
    this.busy = false;
    this.failure = null;
    this.trigger = trigger;
    await this.updateComplete;
    await dialogOpen(this.dialog, trigger);
    this.prefixField.focus();
    this.prefixField.select();
  }

  override render(): TemplateResult {
    return html`<dialog class="ui-dialog" id="dlg-adopt" aria-labelledby="ad-title" aria-describedby="ad-desc" @cancel=${this.onCancel} @click=${this.onDialogClick}>${this.proposal === null ? nothing : this.renderContent(this.proposal)}</dialog>`;
  }

  private renderContent(proposal: AdoptionProposal): TemplateResult {
    const prefixCode = validateTagPrefix(this.tagPrefix);
    const prefixError = prefixCode === null ? null : tagPrefixMessage(prefixCode, this.tagPrefix, proposal.tagPrefix ?? '');
    const dirError = isValidComponentsDir(this.componentsDir) ? null : COMPONENTS_DIR_ERROR;
    const count = String(proposal.componentCount);
    return html`
      <div class="ui-dialog__header">
        <h2 class="ui-dialog__title" id="ad-title">Ouvrir ${proposal.name} avec UI-GEN ?</h2>
        <button class="ui-icon-btn" type="button" aria-label="Fermer" @click=${this.onCloseClick}>${icon('fermer')}</button>
      </div>
      <form class="ui-dialog__body ac-form" id="ad-form" novalidate @submit=${(event: SubmitEvent) => void this.submit(event)}>
        <p id="ad-desc">Cette bibliothèque Lit n'a pas été créée avec UI-GEN. Vérifiez le préfixe et le dossier des composants détectés avant de l'ouvrir.</p>
        <dl class="ac-facts">
          <div><dt>Dossier</dt><dd><code>${proposal.path}</code></dd></div>
          <div><dt>Package</dt><dd><code>${proposal.packageName}</code> ${proposal.version}</dd></div>
          <div><dt>Composants Lit</dt><dd class="ui-num">${count} trouvés dans <code>${proposal.componentsDir}</code></dd></div>
        </dl>
        <div class="ui-field">
          <label class="ui-label" for="ad-prefix">Préfixe des balises</label>
          <input class="ui-input ui-input--mono" id="ad-prefix" autocomplete="off" spellcheck="false" aria-describedby="ad-prefix-help ad-prefix-err" aria-invalid=${prefixError === null ? nothing : 'true'} .value=${this.tagPrefix} @input=${this.onPrefixInput}>
          <p class="ui-help" id="ad-prefix-help">${proposal.exampleTag === null ? 'Aucun composant détecté : indiquez le préfixe de vos balises.' : html`Détecté dans les ${count} composants, par exemple <code>&lt;${proposal.exampleTag}&gt;</code>.`}</p>
          ${fieldError('ad-prefix-err', prefixError)}
        </div>
        <div class="ui-field">
          <label class="ui-label" for="ad-dir">Dossier des composants</label>
          <input class="ui-input ui-input--mono" id="ad-dir" autocomplete="off" spellcheck="false" aria-describedby="ad-dir-help ad-dir-err" aria-invalid=${dirError === null ? nothing : 'true'} .value=${this.componentsDir} @input=${this.onDirInput}>
          <p class="ui-help" id="ad-dir-help">Dossier qui contient les composants détectés.</p>
          ${fieldError('ad-dir-err', dirError)}
        </div>
        <div class="ac-json">
          <div class="ac-json__head">${icon('fichier-code')}<code>uigen.json</code><span class="ui-badge ui-badge--info">Nouveau fichier</span></div>
          <figure class="ui-code" aria-label="Contenu du fichier uigen.json qui sera ajouté">
            <pre class="ui-code__pre" tabindex="0"><code>
<span class="ui-code__line"><span class="tk-punc">{</span></span>
<span class="ui-code__line">  <span class="tk-prop">"formatVersion"</span><span class="tk-punc">:</span> <span class="tk-num">1</span><span class="tk-punc">,</span></span>
<span class="ui-code__line is-target" id="ad-json-prefix">  <span class="tk-prop">"tagPrefix"</span><span class="tk-punc">:</span> <span class="tk-str" id="ad-json-value">${JSON.stringify(this.tagPrefix)}</span><span class="tk-punc">,</span></span>
<span class="ui-code__line is-target" id="ad-json-dir">  <span class="tk-prop">"componentsDir"</span><span class="tk-punc">:</span> <span class="tk-str" id="ad-json-dir-value">${JSON.stringify(this.componentsDir)}</span></span>
<span class="ui-code__line"><span class="tk-punc">}</span></span>
</code></pre>
          </figure>
        </div>
        <p>Après confirmation, UI-GEN ajoute seulement ce fichier à la racine du dossier. Annuler ne modifie rien.</p>
        ${fieldError('ad-err', this.failure, 'alert')}
      </form>
      <div class="ui-dialog__footer">
        <button class="ui-btn ui-btn--secondary" type="button" ?disabled=${this.busy} @click=${this.onCloseClick}>Annuler</button>
        <button class="ui-btn ui-btn--primary" type="submit" form="ad-form" id="ad-submit" ?disabled=${!this.confirmable} aria-busy=${this.busy ? 'true' : nothing}>${this.busy ? 'Écriture de uigen.json…' : 'Ajouter uigen.json et ouvrir'}</button>
      </div>
    `;
  }

  private get confirmable(): boolean {
    return validateTagPrefix(this.tagPrefix) === null && isValidComponentsDir(this.componentsDir);
  }

  private async submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    const proposal = this.proposal;
    if (proposal === null || this.busy || !this.confirmable) return;
    this.busy = true;
    this.failure = null;
    let outcome: OpenProjectOutcome;
    try {
      outcome = await this.api.project.adopt({
        path: proposal.path,
        tagPrefix: this.tagPrefix,
        componentsDir: this.componentsDir,
      });
    } catch (error) {
      console.warn('Request refused', error);
      this.busy = false;
      this.failure = REQUEST_REFUSED.message;
      return;
    }
    await this.applyOutcome(outcome);
  }

  private async applyOutcome(outcome: OpenProjectOutcome): Promise<void> {
    if (outcome.kind === 'opened') {
      await dialogClose(this.dialog, 'ok');
      this.dispatchEvent(new CustomEvent<ProjectAdoptedDetail>(PROJECT_ADOPTED, { detail: { project: outcome.project } }));
      return;
    }
    this.busy = false;
    if (outcome.kind === 'needs-adoption') {
      this.proposal = outcome.proposal;
      return;
    }
    this.failure = openFailureToast(outcome).message;
  }

  private async dismiss(returnValue?: string): Promise<void> {
    if (this.busy) return;
    await dialogClose(this.dialog, returnValue);
    if (this.trigger instanceof HTMLElement) this.trigger.focus();
  }

  private async highlightLine(selector: string, valid: boolean): Promise<void> {
    await this.updateComplete;
    const line = this.renderRoot.querySelector(selector);
    if (valid && line !== null) highlight([line]);
  }

  private readonly onPrefixInput = (): void => {
    this.tagPrefix = this.prefixField.value;
    void this.highlightLine('#ad-json-prefix', validateTagPrefix(this.tagPrefix) === null);
  };

  private readonly onDirInput = (): void => {
    this.componentsDir = this.dirField.value;
    void this.highlightLine('#ad-json-dir', isValidComponentsDir(this.componentsDir));
  };

  private readonly onCancel = (event: Event): void => {
    event.preventDefault();
    void this.dismiss();
  };

  private readonly onDialogClick = (event: MouseEvent): void => {
    if (event.target === this.dialog && isOutside(this.dialog, event)) void this.dismiss();
  };

  private readonly onCloseClick = (): void => {
    void this.dismiss('annuler');
  };
}
