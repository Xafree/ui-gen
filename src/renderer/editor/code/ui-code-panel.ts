import { css, html, LitElement, nothing, type PropertyValues, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import type { Diagnostic } from '../../../modules/editor/domain/code/code-diagnostics.js';
import {
  clampCodeHeight,
  codeHeightAfterKey,
  DEFAULT_CODE_HEIGHT,
  maxCodeHeight,
} from '../../../modules/editor/domain/code/code-panel-height.js';
import type { TextRange } from '../../../modules/editor/domain/code/code-targets.js';
import {
  diagnosticLocation,
  expressionSource,
  fileNameOf,
  problemCounter,
  syncStateOf,
} from '../../../modules/editor/domain/code/problem-summary.js';
import type { OpenedComponent } from '../../../modules/sync/application/components/component-views.js';
import { icon } from '../../icons/icon.js';
import type { ToastRequest } from '../../motion/toast.js';
import { NOTICE_REQUESTED, type NoticeRequestedDetail } from '../../studio-events.js';
import { editorStyles } from '../editor-styles.js';
import './ui-code-view.js';

declare global {
  interface HTMLElementTagNameMap {
    'ui-code-panel': UiCodePanel;
  }
}

const COPY_BLOCKED: ToastRequest = {
  tone: 'warning',
  title: 'Copie impossible',
  message: 'Le presse-papiers est bloqué. Sélectionnez le texte puis appuyez sur Ctrl C.',
};

function copied(file: string): ToastRequest {
  return { tone: 'success', title: `${file} copié`, message: '', duration: 2500 };
}

@customElement('ui-code-panel')
export class UiCodePanel extends LitElement {
  static override readonly styles = [
    ...editorStyles,
    css`
      :host {
        display: contents;
      }
    `,
  ];

  @property({ attribute: false }) component: OpenedComponent | null = null;
  @property({ attribute: false }) displayPath = '';
  @property({ attribute: false }) restarting = false;
  @property({ attribute: false }) targetRange: TextRange | null = null;
  @property({ attribute: false }) diagnostics: readonly Diagnostic[] = [];

  @state() private tab: 'code' | 'problems' = 'code';
  @state() private collapsed = false;
  @state() private height = DEFAULT_CODE_HEIGHT;
  @state() private maximum = maxCodeHeight(0);

  override connectedCallback(): void {
    super.connectedCallback();
    document.addEventListener('keydown', this.onDocumentKeydown);
  }

  override disconnectedCallback(): void {
    document.removeEventListener('keydown', this.onDocumentKeydown);
    super.disconnectedCallback();
  }

  override render(): TemplateResult {
    const { component } = this;
    const file = component === null ? 'Code' : fileNameOf(component.path);
    const sync = syncStateOf(this.diagnostics, this.restarting);
    const counter = problemCounter(this.diagnostics);
    const tab = component === null ? 'code' : this.tab;
    return html`
      <section class="ed-code" id="code-panel" aria-label="Code Lit du composant" data-collapsed=${String(this.collapsed)}>
        <div class="ed-split" id="code-split" role="separator" tabindex="0" aria-orientation="horizontal" aria-label="Redimensionner le panneau de code" aria-controls="code-panel" aria-valuemin="120" aria-valuemax=${String(this.maximum)} aria-valuenow=${String(this.height)} @pointerdown=${this.onSplitPointerDown} @dblclick=${this.onSplitReset} @keydown=${this.onSplitKeydown}></div>
        <div class="ed-code__head">
          <div class="ui-tabs ui-tabs--bare" role="tablist" aria-label="Panneau de code" @click=${this.onTabsClick}>
            <button class="ui-tab" type="button" role="tab" id="tab-code" aria-controls="tp-code" aria-selected=${String(tab === 'code')} @click=${this.onCodeTab}>${icon('fichier-code')}${file}</button>
            ${component === null ? nothing : html`<button class="ui-tab" type="button" role="tab" id="tab-problems" aria-controls="tp-problems" aria-selected=${String(tab === 'problems')} @click=${this.onProblemsTab}>Problèmes ${counter === null ? nothing : html`<span class="ui-count" id="problem-count">${String(counter.count)}</span>`}</button>`}
          </div>
          ${component === null ? nothing : html`<span class="ed-code__path" id="code-path">${this.displayPath}</span>`}
          <span class="ui-spacer"></span>
          ${component === null ? nothing : html`<span class="ed-sync" id="code-sync"><span class="ui-status-dot ui-status-dot--${sync.tone}" aria-hidden="true"></span><span class="ed-sync__label">${sync.code}</span></span>`}
          ${component === null ? nothing : html`<button class="ui-icon-btn ui-icon-btn--sm" type="button" id="code-copy" aria-label="Copier le code" data-tip="Copier le code" data-tip-pos="end" @click=${this.onCopy}>${icon('copier')}</button>`}
          <button class="ui-icon-btn ui-icon-btn--sm ed-collapse" type="button" id="code-collapse" aria-expanded=${String(!this.collapsed)} aria-controls="code-body" aria-label=${this.collapsed ? 'Déplier le panneau de code' : 'Replier le panneau de code'} data-tip=${this.collapsed ? 'Déplier (Ctrl J)' : 'Replier (Ctrl J)'} data-tip-pos="end" @click=${this.toggle}>${icon('chevron-bas')}</button>
        </div>
        <div class="ed-code__body" id="code-body">
          <div class="ed-code__panel" role="tabpanel" id="tp-code" aria-labelledby="tab-code" ?hidden=${tab !== 'code'}>
            ${component === null ? nothing : html`<ui-code-view .path=${component.path} .label=${file} .text=${component.text} .highlight=${component.highlight} .template=${component.model.template} .targetRange=${this.targetRange} .diagnostics=${this.diagnostics}></ui-code-view>`}
          </div>
          ${component === null ? nothing : this.renderProblems(component.text, tab === 'problems')}
        </div>
      </section>
    `;
  }

  expand(): void {
    this.collapsed = false;
  }

  showProblems(): void {
    this.expand();
    this.tab = 'problems';
    void this.updateComplete.then(() => {
      this.renderRoot.querySelector<HTMLElement>('.ed-problem')?.focus();
    });
  }

  protected override willUpdate(changed: PropertyValues<this>): void {
    if (changed.has('component') && this.component !== null) this.refreshMaximum();
  }

  protected override updated(): void {
    this.section?.style.setProperty('--ed-code-h', `${String(this.height)}px`);
  }

  private get section(): HTMLElement | null {
    return this.renderRoot.querySelector<HTMLElement>('#code-panel');
  }

  private renderProblems(text: string, selected: boolean): TemplateResult {
    const none = this.diagnostics.length === 0;
    return html`
      <div class="ed-code__panel" role="tabpanel" id="tp-problems" aria-labelledby="tab-problems" ?hidden=${!selected}>
        <ul class="ui-problems" id="problems" ?hidden=${none}>${this.diagnostics.map((diagnostic, index) => this.renderProblem(text, diagnostic, index))}</ul>
        <div class="ui-empty" id="problems-empty" ?hidden=${!none}>
          <span class="ui-empty__icon">${icon('succes')}</span>
          <p class="ui-empty__title">Aucun problème</p>
          <p class="ui-empty__text">Tout le code de ce composant est représenté dans l'éditeur visuel.</p>
        </div>
        <p class="ed-problems-foot" id="problems-foot" ?hidden=${none}>Le code reste la référence : rien n'est supprimé quand le canvas ne peut pas l'afficher.</p>
      </div>
    `;
  }

  private renderProblem(text: string, diagnostic: Diagnostic, index: number): TemplateResult {
    const warning = diagnostic.severity === 'warning';
    const source = expressionSource(text, diagnostic);
    return html`
      <li>
        <button class=${warning ? 'ed-problem ui-problem ui-problem--warning' : 'ed-problem ui-problem'} type="button" @click=${() => { this.revealDiagnostic(index); }}>
          ${icon(warning ? 'alerte' : 'erreur')}<span>${diagnostic.message}${source === null ? nothing : html` <code>${source}</code>`}</span>
          <span class="ui-problem__loc">${diagnosticLocation(diagnostic)}</span>
        </button>
      </li>
    `;
  }

  private revealDiagnostic(index: number): void {
    this.tab = 'code';
    void this.updateComplete.then(() => {
      this.renderRoot.querySelector('ui-code-view')?.focusDiagnostic(index);
    });
  }

  private refreshMaximum(): void {
    this.maximum = maxCodeHeight(this.parentElement?.clientHeight ?? 0);
  }

  private setHeight(height: number): void {
    this.refreshMaximum();
    this.height = clampCodeHeight(height, this.maximum);
  }

  private readonly toggle = (): void => {
    this.collapsed = !this.collapsed;
  };

  private async copy(component: OpenedComponent): Promise<void> {
    const file = `${component.model.tag}.ts`;
    try {
      await navigator.clipboard.writeText(component.text);
      this.notify(copied(file));
    } catch (error) {
      console.warn('The code could not be copied to the clipboard', error);
      this.notify(COPY_BLOCKED);
    }
  }

  private notify(notice: ToastRequest): void {
    this.dispatchEvent(new CustomEvent<NoticeRequestedDetail>(NOTICE_REQUESTED, { detail: { notice }, bubbles: true, composed: true }));
  }

  private readonly onCopy = (): void => {
    if (this.component !== null) void this.copy(this.component);
  };

  private readonly onCodeTab = (): void => {
    this.tab = 'code';
  };

  private readonly onProblemsTab = (): void => {
    this.tab = 'problems';
  };

  private readonly onTabsClick =(event: MouseEvent): void => {
    if (event.target instanceof Element && event.target.closest('[role="tab"]') !== null) this.expand();
  };

  private readonly onDocumentKeydown = (event: KeyboardEvent): void => {
    if (event.key.toLowerCase() !== 'j' || !(event.ctrlKey || event.metaKey) || event.shiftKey || event.altKey || this.section?.checkVisibility() !== true) return;
    event.preventDefault();
    this.toggle();
  };

  private readonly onSplitKeydown = (event: KeyboardEvent): void => {
    this.refreshMaximum();
    const height = codeHeightAfterKey(event.key, this.height, this.maximum);
    if (height === null) return;
    event.preventDefault();
    this.height = height;
  };

  private readonly onSplitReset = (): void => {
    this.setHeight(DEFAULT_CODE_HEIGHT);
  };

  private readonly onSplitPointerDown = (event: PointerEvent): void => {
    const split = event.currentTarget;
    const section = this.section;
    if (!(split instanceof HTMLElement) || section === null) return;
    event.preventDefault();
    const startY = event.clientY;
    const startHeight = section.offsetHeight;
    split.classList.add('is-dragging');
    document.body.classList.add('is-resizing');
    split.setPointerCapture(event.pointerId);
    const move = (moved: PointerEvent): void => {
      this.setHeight(startHeight - (moved.clientY - startY));
    };
    const stop = (): void => {
      split.classList.remove('is-dragging');
      document.body.classList.remove('is-resizing');
      split.removeEventListener('pointermove', move);
      split.removeEventListener('pointerup', stop);
      split.removeEventListener('pointercancel', stop);
    };
    split.addEventListener('pointermove', move);
    split.addEventListener('pointerup', stop);
    split.addEventListener('pointercancel', stop);
  };
}
