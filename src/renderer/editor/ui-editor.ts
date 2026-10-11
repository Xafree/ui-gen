import { css, html, LitElement, nothing, type PropertyValues, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import { findLayer, layerIds } from '../../modules/editor/domain/layers/layer-lookup.js';
import { buildLayerTree, countLayers, type LayerNode } from '../../modules/editor/domain/layers/layer-tree.js';
import { samePath } from '../../modules/sync/domain/library/component-summary.js';
import type { ComponentSummary } from '../../modules/sync/domain/library/component-summary.js';
import type { ComponentChange, OpenedComponent } from '../../modules/sync/application/components/component-views.js';
import type { AnalyzerStatus } from '../../modules/sync/ports/component-analyzer.js';
import type { UiGenApi } from '../../preload/ui-gen-api.js';
import { icon } from '../icons/icon.js';
import type { ToastRequest } from '../motion/toast.js';
import { NOTICE_REQUESTED, type NoticeRequestedDetail } from '../studio-events.js';
import type { CanvasPhase, CanvasSelectDetail } from './canvas/ui-editor-canvas.js';
import './canvas/ui-editor-canvas.js';
import { editorStyles } from './editor-styles.js';
import type { LayerHoverDetail, LayerIdDetail } from './layers/ui-layer-tree.js';
import './layers/ui-layer-tree.js';
import { EMPTY_LIBRARY_TITLE, IMPORT_HINT, IMPORT_SCREEN, type ComponentOpenDetail } from './library/ui-component-library.js';
import './library/ui-component-library.js';

declare global {
  interface HTMLElementTagNameMap {
    'ui-editor': UiEditor;
  }
}

type LeftTab = 'layers' | 'components';

const LEFT_TABS: readonly LeftTab[] = ['layers', 'components'];
const NO_BREAK_SPACE = String.fromCodePoint(0xa0);
const TYPING = 'input, textarea, select, [contenteditable="true"]';
const TOGGLE_KEYS: Readonly<Record<string, LayerToggle>> = { h: 'hidden', l: 'locked' };

type LayerToggle = 'hidden' | 'locked';

interface ToggleNotice {
  readonly verb: string;
  readonly message: string;
}

const TOGGLE_NOTICES: Readonly<Record<LayerToggle, { readonly on: ToggleNotice; readonly off: ToggleNotice }>> = {
  hidden: {
    on: { verb: 'masqué', message: 'Masqué dans le canvas seulement : le code ne change pas.' },
    off: { verb: 'affiché', message: 'Calque de nouveau visible.' },
  },
  locked: {
    on: { verb: 'verrouillé', message: 'Un clic sur le canvas sélectionne désormais son parent.' },
    off: { verb: 'déverrouillé', message: 'Le calque se sélectionne de nouveau sur le canvas.' },
  },
};

function layerCountLabel(count: number): string {
  return `${String(count)}${NO_BREAK_SPACE}${count > 1 ? 'calques' : 'calque'}`;
}

function toggled(ids: ReadonlySet<string>, id: string): Set<string> {
  const next = new Set(ids);
  if (!next.delete(id)) next.add(id);
  return next;
}

function keptIn(ids: ReadonlySet<string>, kept: ReadonlySet<string>): Set<string> {
  return new Set([...ids].filter((id) => kept.has(id)));
}

function isTyping(event: KeyboardEvent): boolean {
  const origin = event.composedPath()[0];
  return origin instanceof Element && origin.closest(TYPING) !== null;
}

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
  @state() private layers: readonly LayerNode[] = [];
  @state() private selectedId: string | null = null;
  @state() private hoverId: string | null = null;
  @state() private hiddenLayers: ReadonlySet<string> = new Set();
  @state() private lockedLayers: ReadonlySet<string> = new Set();

  private layersPath: string | null = null;
  private openRequest = 0;
  private stopFollowing: (() => void)[] = [];

  override connectedCallback(): void {
    super.connectedCallback();
    document.addEventListener('keydown', this.onDocumentKeydown);
    if (this.hasUpdated) this.follow();
  }

  override disconnectedCallback(): void {
    document.removeEventListener('keydown', this.onDocumentKeydown);
    this.stopFollowing.forEach((stop) => { stop(); });
    this.stopFollowing = [];
    super.disconnectedCallback();
  }

  protected override willUpdate(changed: PropertyValues): void {
    if (changed.has('opened')) this.rebuildLayers();
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
              <span class="ed-file-head__meta" id="layer-count">${layerCountLabel(countLayers(this.layers))}</span>
            </div>`}
          <div class="ui-panel__body">
            ${this.opened === null ? nothing : this.renderLayerTree()}
            ${this.phase === 'empty' ? this.renderLayersEmpty() : nothing}
          </div>
        </div>
        <div class="ed-tabpanel" role="tabpanel" id="tp-comp" aria-labelledby="tab-comp" ?hidden=${this.leftTab !== 'components'}>
          <ui-component-library .components=${this.library ?? []} .openedPath=${this.opened?.path ?? null} @component-open=${this.onComponentOpen}></ui-component-library>
        </div>
      </aside>
    `;
  }

  private renderLayerTree(): TemplateResult {
    return html`<ui-layer-tree .layers=${this.layers} .tag=${this.tag} .selectedId=${this.selectedId} .hiddenLayers=${this.hiddenLayers} .lockedLayers=${this.lockedLayers} @layer-select=${this.onLayerSelect} @layer-hover=${this.onLayerHover} @layer-hide=${this.onLayerHide} @layer-lock=${this.onLayerLock}></ui-layer-tree>`;
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
        <ui-editor-canvas .phase=${this.phase} .component=${this.opened} .tag=${this.tag} .projectPath=${this.projectPath} .layers=${this.layers} .selectedId=${this.selectedId} .hoverId=${this.hoverId} .hiddenLayers=${this.hiddenLayers} .lockedLayers=${this.lockedLayers} @canvas-select=${this.onCanvasSelect}></ui-editor-canvas>
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

  private rebuildLayers(): void {
    const opened = this.opened;
    this.layers = opened === null ? [] : buildLayerTree(opened.model);
    const sameComponent = opened !== null && this.layersPath !== null && samePath(this.layersPath, opened.path);
    this.layersPath = opened?.path ?? null;
    const kept = sameComponent ? layerIds(this.layers) : new Set<string>();
    this.selectedId = this.selectedId !== null && kept.has(this.selectedId) ? this.selectedId : null;
    this.hoverId = null;
    this.hiddenLayers = keptIn(this.hiddenLayers, kept);
    this.lockedLayers = keptIn(this.lockedLayers, kept);
  }

  private toggleLayer(id: string, toggle: LayerToggle): void {
    const layer = findLayer(this.layers, id);
    if (layer === undefined) return;
    const next = toggled(toggle === 'hidden' ? this.hiddenLayers : this.lockedLayers, id);
    if (toggle === 'hidden') this.hiddenLayers = next;
    else this.lockedLayers = next;
    const notice = TOGGLE_NOTICES[toggle][next.has(id) ? 'on' : 'off'];
    this.dispatchEvent(
      new CustomEvent<NoticeRequestedDetail>(NOTICE_REQUESTED, {
        detail: { notice: { tone: 'info', title: `${layer.name} ${notice.verb}`, message: notice.message, duration: 3000 } },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private readonly onLayerSelect = (event: CustomEvent<LayerIdDetail>): void => {
    this.selectedId = event.detail.id;
    this.hoverId = null;
  };

  private readonly onLayerHover = (event: CustomEvent<LayerHoverDetail>): void => {
    this.hoverId = event.detail.id;
  };

  private readonly onLayerHide = (event: CustomEvent<LayerIdDetail>): void => {
    this.toggleLayer(event.detail.id, 'hidden');
  };

  private readonly onLayerLock = (event: CustomEvent<LayerIdDetail>): void => {
    this.toggleLayer(event.detail.id, 'locked');
  };

  private readonly onCanvasSelect = (event: CustomEvent<CanvasSelectDetail>): void => {
    this.selectedId = event.detail.id;
    this.hoverId = null;
  };

  private readonly onDocumentKeydown = (event: KeyboardEvent): void => {
    const toggle = TOGGLE_KEYS[event.key.toLowerCase()];
    const selected = this.selectedId;
    const chord = (event.ctrlKey || event.metaKey) && event.shiftKey && !event.altKey;
    if (toggle === undefined || selected === null || !chord || isTyping(event) || !this.checkVisibility()) return;
    event.preventDefault();
    if (findLayer(this.layers, selected)?.kind !== 'set') this.toggleLayer(selected, toggle);
  };

  private readonly onComponentOpen =(event: CustomEvent<ComponentOpenDetail>): void => {
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
