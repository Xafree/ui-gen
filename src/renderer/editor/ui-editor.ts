import { css, html, LitElement, nothing, type PropertyValues, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import { displayedDiagnostics, type Diagnostic } from '../../modules/editor/domain/code/code-diagnostics.js';
import { layerIdForNode, nodeRange, type TextRange } from '../../modules/editor/domain/code/code-targets.js';
import { problemCounter, syncStateOf } from '../../modules/editor/domain/code/problem-summary.js';
import { layerSubtitle, selectionLabel } from '../../modules/editor/domain/layers/layer-label.js';
import { findLayer, layerIds } from '../../modules/editor/domain/layers/layer-lookup.js';
import { buildLayerTree, countLayers, type LayerNode } from '../../modules/editor/domain/layers/layer-tree.js';
import { samePath } from '../../modules/sync/domain/library/component-summary.js';
import type { ComponentSummary } from '../../modules/sync/domain/library/component-summary.js';
import type { ComponentChange, OpenedComponent } from '../../modules/sync/application/components/component-views.js';
import type { AnalyzerStatus } from '../../modules/sync/ports/component-analyzer.js';
import type { UiGenApi } from '../../preload/ui-gen-api.js';
import { icon } from '../icons/icon.js';
import { placeTabIndicator } from '../motion/motion.js';
import type { ToastRequest } from '../motion/toast.js';
import { NOTICE_REQUESTED, type NoticeRequestedDetail } from '../studio-events.js';
import type { CanvasPhase, CanvasSelectDetail } from './canvas/ui-editor-canvas.js';
import './canvas/ui-editor-canvas.js';
import type { CodeCursorDetail, CodeLineSelectDetail } from './code/ui-code-view.js';
import './code/ui-code-panel.js';
import { editorStyles } from './editor-styles.js';
import './inspector/ui-inspector.js';
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
type RightTab = 'properties' | 'interactions';
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
  @state() private libraryFailed = false;
  @state() private opened: OpenedComponent | null = null;
  @state() private restarting = false;
  @state() private leftTab: LeftTab = 'layers';
  @state() private rightTab: RightTab = 'properties';
  @state() private layers: readonly LayerNode[] = [];
  @state() private selectedId: string | null = null;
  @state() private hoverId: string | null = null;
  @state() private hiddenLayers: ReadonlySet<string> = new Set();
  @state() private lockedLayers: ReadonlySet<string> = new Set();
  @state() private cursor: CodeCursorDetail = { line: 1, column: 1 };
  @state() private diagnostics: readonly Diagnostic[] = [];

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
    if (changed.has('opened')) {
      this.rebuildLayers();
      this.diagnostics = displayedDiagnostics(this.opened);
    }
  }

  protected override updated(): void {
    for (const list of this.renderRoot.querySelectorAll<HTMLElement>('.ui-tabs')) placeTabIndicator(list);
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
    if (this.libraryFailed) return 'error';
    if (this.library === null) return 'loading';
    return this.library.length === 0 ? 'empty' : 'ready';
  }

  private get targetRange(): TextRange | null {
    const nodeId = this.selectedId === null ? null : (findLayer(this.layers, this.selectedId)?.nodeId ?? null);
    return this.opened === null || nodeId === null ? null : nodeRange(this.opened.model.template, nodeId);
  }

  private get selectedLayer(): LayerNode | null {
    return this.selectedId === null ? null : (findLayer(this.layers, this.selectedId) ?? null);
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
    return html`
      <main class="ui-stage ed-stage" id="main" tabindex="-1">
        <h1 class="sr-only" id="page-title">${this.opened === null ? 'Éditeur' : `Éditeur du composant ${this.tag}`}</h1>
        <ui-editor-canvas .phase=${this.phase} .component=${this.opened} .tag=${this.tag} .projectPath=${this.projectPath} .layers=${this.layers} .selectedId=${this.selectedId} .hoverId=${this.hoverId} .hiddenLayers=${this.hiddenLayers} .lockedLayers=${this.lockedLayers} @canvas-select=${this.onCanvasSelect} @library-retry=${this.onLibraryRetry}></ui-editor-canvas>
        <ui-code-panel .component=${this.opened} .displayPath=${this.opened === null ? '' : projectRelative(this.projectPath, this.opened.path)} .restarting=${this.restarting} .diagnostics=${this.diagnostics} .targetRange=${this.targetRange} @code-line-select=${this.onCodeLineSelect} @code-cursor=${this.onCodeCursor}></ui-code-panel>
      </main>
    `;
  }

  private renderRightPanel(): TemplateResult {
    return html`
      <aside class="ui-panel ui-panel--right" id="panneau-droit" aria-label="Propriétés et interactions">
        <div class="ui-tabs" role="tablist" aria-label="Panneau de droite" @keydown=${this.onTabKeydown}>
          <button class="ui-tab" type="button" role="tab" id="tab-props" aria-controls="tp-props" aria-selected=${String(this.rightTab === 'properties')} tabindex=${this.rightTab === 'properties' ? '0' : '-1'} @click=${() => { this.rightTab = 'properties'; }}>Propriétés</button>
          ${this.renderInteractionsTab()}
        </div>
        ${this.renderInspectorHead()}
        <div class="ed-tabpanel" role="tabpanel" id="tp-props" aria-labelledby="tab-props" ?hidden=${this.rightTab !== 'properties'}>
          <ui-inspector .model=${this.opened?.model ?? null} .layer=${this.selectedLayer}></ui-inspector>
        </div>
        ${this.opened === null ? nothing : this.renderInteractions()}
      </aside>
    `;
  }

  private renderInteractionsTab(): TemplateResult | typeof nothing {
    if (this.opened === null) return nothing;
    const active = this.rightTab === 'interactions';
    return html`<button class="ui-tab" type="button" role="tab" id="tab-inter" aria-controls="tp-inter" aria-selected=${String(active)} tabindex=${active ? '0' : '-1'} @click=${() => { this.rightTab = 'interactions'; }}>Interactions <span class="ui-count" id="rule-count">0</span></button>`;
  }

  private renderInspectorHead(): TemplateResult | typeof nothing {
    const layer = this.selectedLayer;
    const model = this.opened?.model;
    if (layer === null || model === undefined) return nothing;
    return html`
      <div class="ed-insp-head" id="insp-head">
        <p class="ed-insp-head__name">${icon('composants')}<span id="insp-name">${layer.name}</span></p>
        <p class="ed-insp-head__sub" id="insp-sub">${layerSubtitle(layer, model.tag, Math.max(model.variants.length, 1))}</p>
      </div>
    `;
  }

  private renderInteractions(): TemplateResult {
    return html`
      <div class="ed-tabpanel" role="tabpanel" id="tp-inter" aria-labelledby="tab-inter" ?hidden=${this.rightTab !== 'interactions'}>
        <div class="ui-panel__body">
          <div class="ui-empty" id="rules-empty">
            <span class="ui-empty__icon">${icon('eclair')}</span>
            <p class="ui-empty__title" id="rules-empty-title">Aucune interaction sur ce calque</p>
            <p class="ui-empty__text">Les interactions se règlent sur les éléments qui réagissent au clic ou au clavier, comme hz-button.</p>
            <div class="ui-empty__actions">
              <button class="ui-btn ui-btn--secondary ui-btn--sm" type="button" disabled>Sélectionner le bouton de la carte Pro</button>
            </div>
          </div>
        </div>
      </div>
    `;
  }

  private renderStatusBar(): TemplateResult {
    const sync = syncStateOf(this.diagnostics, this.restarting);
    return html`
      <footer class="ui-statusbar" aria-label="Barre d'état">
        <span class="ui-statusbar__item">${icon('code')}<span><span class="ed-status-long">Cible : </span>Lit<span class="ed-status-long"> + TypeScript</span></span></span>
        <span class="ui-statusbar__item" id="status-sync" role="status"><span class="ui-status-dot ui-status-dot--${sync.tone}" aria-hidden="true"></span><span id="status-sync-label">${sync.status}</span></span>
        ${this.renderProblemsButton()}
        <span class="ui-statusbar__spacer"></span>
        ${this.renderSelectionStatus()}
        <span class="ui-statusbar__item ed-status-opt">Espaces : 2</span>
        <span class="ui-statusbar__item ed-status-opt">UTF-8</span>
      </footer>
    `;
  }

  private renderProblemsButton(): TemplateResult | typeof nothing {
    const counter = problemCounter(this.diagnostics);
    if (counter === null) return nothing;
    const errorClass = counter.hasError ? ' is-error' : '';
    const word = ` ${counter.word}`;
    return html`<button class="ed-status-btn ed-status-warn${errorClass}" type="button" id="status-warn" aria-label=${counter.name} @click=${this.onProblemsClick}>${icon('alerte')}<span id="status-warn-label">${String(counter.count)}<span class="ed-status-long">${word}</span></span></button>`;
  }

  private renderSelectionStatus(): TemplateResult | typeof nothing {
    if (this.opened === null) return nothing;
    const selection = this.selectedLayer === null ? this.tag : selectionLabel(this.selectedLayer);
    return html`<span class="ui-statusbar__item ed-status-opt" id="status-sel">${selection}</span><span class="ui-statusbar__item ed-status-num" id="status-cursor">Ligne ${String(this.cursor.line)}, col. ${String(this.cursor.column)}</span>`;
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
      this.libraryFailed = true;
    }
  }

  private readonly onLibraryRetry = (): void => {
    this.libraryFailed = false;
    this.library = null;
    void this.load();
  };

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

  private readonly onCodeLineSelect = (event: CustomEvent<CodeLineSelectDetail>): void => {
    if (this.opened === null) return;
    const selected = this.selectedId === null ? undefined : findLayer(this.layers, this.selectedId);
    this.selectedId = layerIdForNode(this.opened.model, selected, event.detail.nodeId);
    this.hoverId = null;
  };

  private readonly onProblemsClick = (): void => {
    this.renderRoot.querySelector('ui-code-panel')?.showProblems();
  };

  private readonly onCodeCursor =(event: CustomEvent<CodeCursorDetail>): void => {
    this.cursor = event.detail;
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
    const list = event.currentTarget;
    if (!(list instanceof HTMLElement)) return;
    const tabs = [...list.querySelectorAll<HTMLElement>('[role="tab"]')];
    const index = tabs.findIndex((tab) => tab.getAttribute('aria-selected') === 'true');
    const last = tabs.length - 1;
    const targets: Readonly<Record<string, number>> = {
      ArrowRight: Math.min(last, index + 1),
      ArrowLeft: Math.max(0, index - 1),
      Home: 0,
      End: last,
    };
    const target = tabs[targets[event.key] ?? index];
    if (target === undefined || index < 0 || target === tabs[index]) return;
    event.preventDefault();
    target.click();
    target.focus();
  };
}
