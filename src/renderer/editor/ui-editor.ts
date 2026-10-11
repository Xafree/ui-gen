import { css, html, LitElement, nothing, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import { samePath } from '../../modules/sync/domain/library/component-summary.js';
import type { ComponentSummary } from '../../modules/sync/domain/library/component-summary.js';
import type { ComponentChange, OpenedComponent } from '../../modules/sync/application/components/component-views.js';
import type { AnalyzerStatus } from '../../modules/sync/ports/component-analyzer.js';
import type { UiGenApi } from '../../preload/ui-gen-api.js';
import { icon } from '../icons/icon.js';
import type { ToastRequest } from '../motion/toast.js';
import { NOTICE_REQUESTED, type NoticeRequestedDetail } from '../studio-events.js';
import type { CanvasPhase } from './canvas/ui-editor-canvas.js';
import './canvas/ui-editor-canvas.js';
import { editorStyles } from './editor-styles.js';
import { EMPTY_LIBRARY_TITLE, IMPORT_HINT, IMPORT_SCREEN, type ComponentOpenDetail } from './library/ui-component-library.js';
import './library/ui-component-library.js';

declare global {
  interface HTMLElementTagNameMap {
    'ui-editor': UiEditor;
  }
}

type LeftTab = 'layers' | 'components';

const LEFT_TABS: readonly LeftTab[] = ['layers', 'components'];

function openFailure(path: string): ToastRequest {
  return {
    tone: 'danger',
    title: "Ce composant ne s'ouvre pas",
    message: `Le fichier ${path} n'a pas pu être lu. Rien n'a été modifié.`,
  };
}

function projectRelative(projectPath: string, path: string): string {
  const prefix = `${projectPath}\\`;
  const inside = projectPath !== '' && path.toLowerCase().startsWith(prefix.toLowerCase());
  return (inside ? path.slice(prefix.length) : path).replaceAll('\\', '/');
}

@customElement('ui-editor')
export class UiEditor extends LitElement {
  static override readonly styles = [
    ...editorStyles,
    css`
      :host {
        display: grid;
        grid-template-rows: minmax(0, 1fr) auto;
        grid-row: 2 / 4;
        min-width: 0;
        min-height: 0;
      }

      :host([hidden]) {
        display: none;
      }
    `,
  ];

  @property({ attribute: false }) api!: UiGenApi;
  @property({ attribute: false }) projectPath = '';

  @state() private library: readonly ComponentSummary[] | null = null;
  @state() private opened: OpenedComponent | null = null;
  @state() private restarting = false;
  @state() private leftTab: LeftTab = 'layers';

  private openRequest = 0;
  private stopFollowing: (() => void)[] = [];

  override connectedCallback(): void {
    super.connectedCallback();
    if (this.hasUpdated) this.follow();
  }

  override disconnectedCallback(): void {
    this.stopFollowing.forEach((stop) => { stop(); });
    this.stopFollowing = [];
    super.disconnectedCallback();
  }

  protected override firstUpdated(): void {
    this.follow();
    void this.load();
  }

  override render(): TemplateResult {
    return html`
      <div class="ui-workspace">
        ${this.renderLeftPanel()}${this.renderStage()}${this.renderRightPanel()}
      </div>
      ${this.renderStatusBar()}
    `;
  }

  focusStage(): void {
    this.renderRoot.querySelector<HTMLElement>('#main')?.focus();
  }

  private get phase(): CanvasPhase {
    if (this.library === null) return 'loading';
    return this.library.length === 0 ? 'empty' : 'ready';
  }

  private get tag(): string {
    return this.opened?.model.tag ?? '';
  }

  private renderLeftPanel(): TemplateResult {
    const count = this.library?.length;
    return html`
      <aside class="ui-panel ui-panel--left" id="panneau-gauche" aria-label="Calques et composants">
        <div class="ui-tabs" role="tablist" aria-label="Panneau de gauche" @keydown=${this.onTabKeydown}>
          <button class="ui-tab" type="button" role="tab" id="tab-calques" aria-controls="tp-calques" aria-selected=${String(this.leftTab === 'layers')} tabindex=${this.leftTab === 'layers' ? '0' : '-1'} @click=${() => { this.leftTab = 'layers'; }}>Calques</button>
          <button class="ui-tab" type="button" role="tab" id="tab-comp" aria-controls="tp-comp" aria-selected=${String(this.leftTab === 'components')} tabindex=${this.leftTab === 'components' ? '0' : '-1'} @click=${() => { this.leftTab = 'components'; }}>Composants ${count === undefined ? nothing : html`<span class="ui-count" id="comp-count">${String(count)}</span>`}</button>
        </div>
        <div class="ed-tabpanel" role="tabpanel" id="tp-calques" aria-labelledby="tab-calques" ?hidden=${this.leftTab !== 'layers'}>
          ${this.opened === null ? nothing : html`
            <div class="ed-file-head" id="layers-head">
              ${icon('fichier-code')}
              <span class="ed-file-head__name" id="layers-file">${this.tag}.ts</span>
            </div>`}
          <div class="ui-panel__body">
            ${this.phase === 'empty' ? this.renderLayersEmpty() : nothing}
          </div>
        </div>
        <div class="ed-tabpanel" role="tabpanel" id="tp-comp" aria-labelledby="tab-comp" ?hidden=${this.leftTab !== 'components'}>
          <ui-component-library .components=${this.library ?? []} .openedPath=${this.opened?.path ?? null} @component-open=${this.onComponentOpen}></ui-component-library>
        </div>
      </aside>
    `;
  }

  private renderLayersEmpty(): TemplateResult {
    return html`
      <div class="ui-empty" id="layers-empty">
        <span class="ui-empty__icon">${icon('composants')}</span>
        <p class="ui-empty__title">${EMPTY_LIBRARY_TITLE}</p>
        <p class="ui-empty__text">${IMPORT_HINT}</p>
        <div class="ui-empty__actions">
          <a class="ui-btn ui-btn--secondary ui-btn--sm" href=${IMPORT_SCREEN}>Importer un composant</a>
        </div>
      </div>
    `;
  }

  private renderStage(): TemplateResult {
    const file = this.opened === null ? 'Code' : `${this.tag}.ts`;
    return html`
      <main class="ui-stage ed-stage" id="main" tabindex="-1">
        <h1 class="sr-only" id="page-title">${this.opened === null ? 'Éditeur' : `Éditeur du composant ${this.tag}`}</h1>
        <ui-editor-canvas .phase=${this.phase} .component=${this.opened} .tag=${this.tag} .projectPath=${this.projectPath}></ui-editor-canvas>
        <section class="ed-code" id="code-panel" aria-label="Code Lit du composant">
          <div class="ed-code__head">
            <div class="ui-tabs ui-tabs--bare" role="tablist" aria-label="Panneau de code">
              <button class="ui-tab" type="button" role="tab" id="tab-code" aria-controls="tp-code" aria-selected="true">${icon('fichier-code')}${file}</button>
            </div>
            ${this.opened === null ? nothing : html`<span class="ed-code__path" id="code-path">${projectRelative(this.projectPath, this.opened.path)}</span>`}
            <span class="ui-spacer"></span>
            ${this.opened === null ? nothing : html`<span class="ed-sync" id="code-sync">${this.syncDot()}<span class="ed-sync__label">${this.restarting ? 'Analyse du code relancée…' : 'Synchronisé avec le canvas'}</span></span>`}
          </div>
          <div class="ed-code__body" id="code-body"><div class="ed-code__panel" role="tabpanel" id="tp-code" aria-labelledby="tab-code"></div></div>
        </section>
      </main>
    `;
  }

  private renderRightPanel(): TemplateResult {
    return html`
      <aside class="ui-panel ui-panel--right" id="panneau-droit" aria-label="Propriétés et interactions">
        <div class="ui-tabs" role="tablist" aria-label="Panneau de droite">
          <button class="ui-tab" type="button" role="tab" id="tab-props" aria-controls="tp-props" aria-selected="true">Propriétés</button>
        </div>
        <div class="ed-tabpanel" role="tabpanel" id="tp-props" aria-labelledby="tab-props">
          <div class="ui-panel__body">
            <div class="ui-empty" id="props-empty">
              <span class="ui-empty__icon">${icon('curseur')}</span>
              <p class="ui-empty__title">Aucun élément sélectionné</p>
            </div>
          </div>
        </div>
      </aside>
    `;
  }

  private renderStatusBar(): TemplateResult {
    return html`
      <footer class="ui-statusbar" aria-label="Barre d'état">
        <span class="ui-statusbar__item">${icon('code')}<span><span class="ed-status-long">Cible : </span>Lit<span class="ed-status-long"> + TypeScript</span></span></span>
        <span class="ui-statusbar__item" id="status-sync" role="status">${this.syncDot()}<span id="status-sync-label">${this.restarting ? "Relance de l'analyse…" : 'Synchronisé'}</span></span>
        <span class="ui-statusbar__spacer"></span>
      </footer>
    `;
  }

  private syncDot(): TemplateResult {
    return html`<span class="ui-status-dot ${this.restarting ? 'ui-status-dot--accent' : 'ui-status-dot--success'}" aria-hidden="true"></span>`;
  }

  private follow(): void {
    this.stopFollowing = [
      this.api.components.onChanged(this.onChanged),
      this.api.components.onStatus(this.onStatus),
    ];
  }

  private async load(): Promise<void> {
    try {
      const [library, lastOpened] = await Promise.all([this.api.components.list(), this.api.components.lastOpened()]);
      this.library = library;
      const path = lastOpened ?? library[0]?.path;
      if (path !== undefined) await this.openComponent(path);
    } catch (error) {
      console.warn('The component library could not be read', error);
    }
  }

  private async openComponent(path: string): Promise<void> {
    const request = ++this.openRequest;
    try {
      const opened = await this.api.components.open(path);
      if (request === this.openRequest) this.opened = opened;
    } catch (error) {
      console.warn('The component to show in the editor could not be opened', error);
      this.dispatchEvent(
        new CustomEvent<NoticeRequestedDetail>(NOTICE_REQUESTED, { detail: { notice: openFailure(path) }, bubbles: true, composed: true }),
      );
    }
  }

  private readonly onComponentOpen = (event: CustomEvent<ComponentOpenDetail>): void => {
    const { path } = event.detail;
    if (this.opened === null || !samePath(this.opened.path, path)) void this.openComponent(path);
  };

  private readonly onChanged = (change: ComponentChange): void => {
    if (this.opened !== null && samePath(this.opened.path, change.path)) this.opened = change;
  };

  private readonly onStatus = (status: AnalyzerStatus): void => {
    this.restarting = status === 'restarting';
  };

  private readonly onTabKeydown = (event: KeyboardEvent): void => {
    const index = LEFT_TABS.indexOf(this.leftTab);
    const last = LEFT_TABS.length - 1;
    const targets: Readonly<Record<string, number>> = {
      ArrowRight: Math.min(last, index + 1),
      ArrowLeft: Math.max(0, index - 1),
      Home: 0,
      End: last,
    };
    const target = LEFT_TABS[targets[event.key] ?? index];
    if (target === undefined || target === this.leftTab) return;
    event.preventDefault();
    this.leftTab = target;
    void this.updateComplete.then(() => {
      this.renderRoot.querySelector<HTMLElement>(`#${target === 'layers' ? 'tab-calques' : 'tab-comp'}`)?.focus();
    });
  };
}
