import { css, html, LitElement, nothing, type PropertyValues, type TemplateResult } from 'lit';
import { customElement, property, query, state } from 'lit/decorators.js';
import { styleMap } from 'lit/directives/style-map.js';
import { fitView } from '../../../modules/editor/domain/viewport/fit.js';
import { rulerFor, type Ruler, type RulerMark } from '../../../modules/editor/domain/viewport/ruler.js';
import {
  DEFAULT_SCREEN_SIZE,
  SCREEN_SIZES,
  formatPixels,
  screenSizeById,
  type ScreenSizeId,
} from '../../../modules/editor/domain/viewport/screen-size.js';
import {
  nextZoomStep,
  panBy,
  zoomAfterWheel,
  zoomAround,
  zoomPercent,
  type CanvasView,
} from '../../../modules/editor/domain/viewport/zoom.js';
import { nearestDrawnAncestor, neighboursOf } from '../../../modules/editor/domain/layers/layer-lookup.js';
import type { LayerNode } from '../../../modules/editor/domain/layers/layer-tree.js';
import {
  selectionGuides,
  sizeLabel,
  toViewportBox,
  type Box,
  type Point,
} from '../../../modules/editor/domain/selection/selection-overlay.js';
import type { NodeRect } from '../../../modules/canvas/domain/harness-message.js';
import type { OpenedComponent } from '../../../modules/sync/application/components/component-views.js';
import { icon } from '../../icons/icon.js';
import type { ToastRequest } from '../../motion/toast.js';
import { NOTICE_REQUESTED, type NoticeRequestedDetail } from '../../studio-events.js';
import { editorStyles } from '../editor-styles.js';
import { EMPTY_LIBRARY_TITLE, IMPORT_HINT, IMPORT_SCREEN } from '../library/ui-component-library.js';
import '../selection/ui-selection-overlay.js';
import type { UiSelectionOverlay } from '../selection/ui-selection-overlay.js';
import type { CanvasFailedDetail, CanvasRectsDetail, UiCanvasFrame } from './ui-canvas-frame.js';
import './ui-canvas-frame.js';

declare global {
  interface HTMLElementTagNameMap {
    'ui-editor-canvas': UiEditorCanvas;
  }
}

export type CanvasPhase = 'loading' | 'empty' | 'ready';

export const CANVAS_SELECT = 'canvas-select';

export interface CanvasSelectDetail {
  readonly id: string | null;
}

type Tool = 'select' | 'frame' | 'text' | 'hand';

const NO_BREAK_SPACE = String.fromCodePoint(0xa0);
const DEFAULT_FRAME_HEIGHT = 320;
const FRAME_BOTTOM_MARGIN = 16;
const MAX_FRAME_HEIGHT = 4000;
const RENDER_FAILED_TITLE = "Ce composant ne s'affiche pas";
const RENDER_FAILED_TEXT = "Son code a levé une erreur au rendu. Le fichier n'a pas été modifié.";
const ARROW_STEP = 24;
const ARROW_STEP_FAST = 80;
const LINE_HEIGHT_PX = 16;
const TOOL_KEYS: Readonly<Record<string, Tool>> = { v: 'select', f: 'frame', t: 'text', h: 'hand' };
const TOOL_HINTS: Readonly<Partial<Record<Tool, string>>> = {
  frame: "Glissez pour dessiner un cadre. Il reste hors du code jusqu'à son intégration.",
  text: 'Cliquez pour placer un texte, puis saisissez-le.',
  hand: 'Glissez pour déplacer la vue. Astuce : maintenez Espace avec un autre outil.',
};
const INTERACTIONS_PENDING: ToastRequest = {
  tone: 'info',
  title: 'Disponible dans un prochain lot',
  message: 'Les interactions arrivent avec le lot 4.',
};
const TYPING = 'input, textarea, select, [contenteditable="true"]';
const OWN_KEYBOARD = '[role="tree"], [role="listbox"], [role="radiogroup"], [role="tablist"], #code-panel';

interface PanDrag {
  readonly pointerId: number;
  readonly startX: number;
  readonly startY: number;
  readonly from: CanvasView;
}

function isTyping(event: KeyboardEvent): boolean {
  const origin = event.composedPath()[0];
  return origin instanceof Element && origin.closest(TYPING) !== null;
}

function hasOwnKeyboard(event: KeyboardEvent): boolean {
  return event.composedPath().some((target) => target instanceof Element && target.matches(OWN_KEYBOARD));
}

function variantsLabel(count: number): string {
  return `${String(count)} ${count > 1 ? 'variantes' : 'variante'}`;
}

@customElement('ui-editor-canvas')
export class UiEditorCanvas extends LitElement {
  static override readonly styles = [
    ...editorStyles,
    css`
      :host {
        display: block;
        min-width: 0;
        min-height: 0;
      }

      #canvas-area {
        height: 100%;
      }

      ui-canvas-frame {
        width: 100%;
        height: 100%;
        pointer-events: none;
      }
    `,
  ];

  @property({ attribute: false }) phase: CanvasPhase = 'loading';
  @property({ attribute: false }) component: OpenedComponent | null = null;
  @property({ attribute: false }) tag = '';
  @property({ attribute: false }) projectPath = '';
  @property({ attribute: false }) layers: readonly LayerNode[] = [];
  @property({ attribute: false }) selectedId: string | null = null;
  @property({ attribute: false }) hoverId: string | null = null;
  @property({ attribute: false }) hiddenLayers: ReadonlySet<string> = new Set();
  @property({ attribute: false }) lockedLayers: ReadonlySet<string> = new Set();

  @state() private rects: ReadonlyMap<string, NodeRect> = new Map();
  @state() private view: CanvasView = { x: 0, y: 0, zoom: 1 };
  @state() private screenSize: ScreenSizeId = DEFAULT_SCREEN_SIZE;
  @state() private darkPreview = false;
  @state() private tool: Tool = 'select';
  @state() private frameHeight = DEFAULT_FRAME_HEIGHT;
  @state() private renderFailed = false;
  @state() private viewportWidth = 0;
  @state() private viewportHeight = 0;
  @state() private spaceDown = false;
  @state() private panning = false;

  @query('#viewport') private readonly viewport!: HTMLElement | null;
  @query('ui-canvas-frame') private readonly frame!: UiCanvasFrame | null;
  @query('ui-selection-overlay') private readonly overlay!: UiSelectionOverlay | null;

  private autoFit = true;
  private drag: PanDrag | null = null;
  private observed: Element | null = null;
  private readonly resizeObserver = new ResizeObserver(() => {
    this.measureViewport();
  });

  override connectedCallback(): void {
    super.connectedCallback();
    document.addEventListener('keydown', this.onDocumentKeydown);
    document.addEventListener('keyup', this.onDocumentKeyup);
    window.addEventListener('blur', this.onWindowBlur);
  }

  override disconnectedCallback(): void {
    document.removeEventListener('keydown', this.onDocumentKeydown);
    document.removeEventListener('keyup', this.onDocumentKeyup);
    window.removeEventListener('blur', this.onWindowBlur);
    this.resizeObserver.disconnect();
    this.observed = null;
    super.disconnectedCallback();
  }

  override render(): TemplateResult {
    return html`<div class="ui-canvas ed-canvas" id="canvas-area">${this.phase === 'ready' ? this.renderReady() : this.renderPhase()}</div>`;
  }

  protected override willUpdate(changed: PropertyValues<this>): void {
    if (changed.has('component')) {
      this.frameHeight = DEFAULT_FRAME_HEIGHT;
      if (changed.get('component')?.path !== this.component?.path) this.renderFailed = false;
    }
  }

  protected override updated(changed: PropertyValues): void {
    this.observeViewport();
    this.drawOverlay();
    if (this.autoFit && (changed.has('screenSize') || changed.has('frameHeight'))) this.fit();
  }

  private renderPhase(): TemplateResult | typeof nothing {
    if (this.phase === 'loading') return nothing;
    return html`
      <div class="ed-canvas-empty" id="canvas-empty">
        <div class="ui-empty">
          <span class="ui-empty__icon">${icon('composants')}</span>
          <p class="ui-empty__title">${EMPTY_LIBRARY_TITLE}</p>
          <p class="ui-empty__text">${IMPORT_HINT}</p>
          <div class="ui-empty__actions">
            <a class="ui-btn ui-btn--secondary ui-btn--sm" href=${IMPORT_SCREEN}>Importer un composant</a>
          </div>
        </div>
      </div>
    `;
  }

  private renderRenderFailed(): TemplateResult {
    return html`
      <div class="ed-canvas-empty" id="render-failed" role="alert">
        <div class="ui-empty">
          <span class="ui-empty__icon">${icon('erreur')}</span>
          <p class="ui-empty__title">${RENDER_FAILED_TITLE}</p>
          <p class="ui-empty__text">${RENDER_FAILED_TEXT}</p>
        </div>
      </div>
    `;
  }

  private renderReady(): TemplateResult {
    const hint = TOOL_HINTS[this.tool];
    return html`
      ${this.renderFailed ? this.renderRenderFailed() : nothing}
      <div class="ed-ruler-corner" aria-hidden="true"></div>
      ${this.renderRuler('x', this.viewportWidth, this.view.x)}${this.renderRuler('y', this.viewportHeight, this.view.y)}
      <div class="ed-viewport ${this.spaceDown ? 'is-space' : ''} ${this.panning ? 'is-panning' : ''}" id="viewport" tabindex="0" role="group" aria-roledescription="canvas" aria-label=${this.viewportLabel()} aria-describedby="viewport-help" data-tool=${this.tool} @wheel=${{ handleEvent: this.onWheel, passive: false }} @keydown=${this.onViewportKeydown} @pointerdown=${this.onPointerDown} @pointermove=${this.onPointerMove} @pointerup=${this.endDrag} @pointercancel=${this.endDrag} @click=${this.onViewportClick}>
        <p class="sr-only" id="viewport-help">Flèches : déplacer la vue. Plus et moins : zoomer. Sélectionnez les éléments au clavier dans le panneau Calques.</p>
        <div class="ed-world" id="world" style=${styleMap({ transform: `translate(${String(this.view.x)}px, ${String(this.view.y)}px) scale(${String(this.view.zoom)})`, '--z': String(this.view.zoom) })}>
          ${this.component === null ? nothing : this.renderFrame(this.component)}
        </div>
        <ui-selection-overlay></ui-selection-overlay>
      </div>
      ${this.renderCanvasBar()}
      <span class="ui-tooltip ed-tool-hint" id="tool-hint" role="status" ?hidden=${hint === undefined}>${hint ?? ''}</span>
      ${this.renderToolbar()}
    `;
  }

  private renderRuler(axis: 'x' | 'y', length: number, offset: number): TemplateResult {
    const ruler: Ruler = rulerFor(length, offset, this.view.zoom);
    const style = styleMap({ '--ed-minor': `${String(ruler.minor)}px`, '--ed-off': `${String(ruler.offset)}px` });
    return html`<div class="ed-ruler ed-ruler--${axis}" id="ruler-${axis}" aria-hidden="true" style=${style}>${ruler.marks.map((mark) => this.renderRulerMark(axis, mark))}</div>`;
  }

  private renderRulerMark(axis: 'x' | 'y', mark: RulerMark): TemplateResult {
    const position = `${String(mark.position)}px`;
    if (axis === 'x') return html`<span class="ed-ruler__mark" style=${styleMap({ left: position })}>${mark.label}</span>`;
    return html`<span class="ed-ruler__mark" style=${styleMap({ top: position })}><span>${mark.label}</span></span>`;
  }

  private renderFrame(component: OpenedComponent): TemplateResult {
    const size = screenSizeById(this.screenSize);
    const variants = Math.max(component.model.variants.length, 1);
    return html`
      <div class="ui-frame ed-frame" id="frame">
        <div class="ui-frame__label"><span id="frame-name">${this.tag}</span><span class="ui-mono" id="frame-meta">${variantsLabel(variants)} · ${size.label}</span></div>
        <div class="ui-frame__body ed-frame__body" id="frame-body" data-screen=${size.id} aria-hidden="true" style=${styleMap({ width: `${String(size.width)}px`, height: `${String(this.frameHeight)}px` })}>
          <ui-canvas-frame .component=${component} .projectPath=${this.projectPath} ?darkPreview=${this.darkPreview} .hiddenLayers=${this.hiddenLayers} .lockedLayers=${this.lockedLayers} @canvas-rects=${this.onRects} @canvas-ready=${this.onRendered} @canvas-failed=${this.onRenderFailed}></ui-canvas-frame>
        </div>
      </div>
    `;
  }

  private renderCanvasBar(): TemplateResult {
    const size = screenSizeById(this.screenSize);
    return html`
      <div class="ed-canvas-bar">
        <div class="ui-segmented" role="radiogroup" aria-label="Taille d'écran du cadre" id="screen-seg">
          ${SCREEN_SIZES.map((option) => html`<label><input type="radio" name="ecran" value=${option.id} .checked=${option.id === this.screenSize} @change=${this.onScreenSize}><span>${icon(option.id, 'icon--sm')}<span class="ed-seg-label">${option.label}</span></span></label>`)}
        </div>
        <span class="ed-frame-chip" id="frame-chip">${formatPixels(size.width)}</span>
        <span class="ui-spacer"></span>
        <button class="ui-btn ui-btn--secondary ui-btn--sm" type="button" id="hz-dark" aria-pressed=${String(this.darkPreview)} @click=${this.toggleDarkPreview}>${icon('lune', 'icon--sm')}<span class="ed-btn-label">Aperçu sombre</span></button>
        <button class="ui-btn ui-btn--secondary ui-btn--sm" type="button" id="preview-toggle" aria-pressed="false" @click=${this.showInteractionsPending}>${icon('play', 'icon--sm')}<span class="ed-btn-label">Tester les interactions</span></button>
      </div>
    `;
  }

  private renderToolbar(): TemplateResult {
    const percent = zoomPercent(this.view.zoom);
    return html`
      <div class="ui-toolbar ed-toolbar" role="toolbar" aria-label="Outils du canvas" id="toolbar">
        ${this.renderToolButton('select', 'curseur', 'Sélection (V)', 'Sélection (V)')}${this.renderToolButton('frame', 'cadre', 'Cadre (F)', 'Cadre (F)')}${this.renderToolButton('text', 'texte', 'Texte (T)', 'Texte (T)')}${this.renderToolButton('hand', 'main', 'Main : déplacer la vue (H)', 'Déplacer la vue (H)')}
        <span class="ui-toolbar__sep" aria-hidden="true"></span>
        <button class="ui-icon-btn" type="button" id="zoom-out" aria-label="Zoom arrière" data-tip="Zoom arrière (Ctrl −)" data-tip-pos="top" @click=${this.zoomOut}>${icon('dezoom')}</button>
        <button class="ui-toolbar__value" type="button" id="zoom-value" aria-label=${`Zoom : ${String(percent)} %. Revenir à 100 %`} data-tip="Revenir à 100 % (Maj 0)" data-tip-pos="top" @click=${this.resetZoom}>${String(percent)}${NO_BREAK_SPACE}%</button>
        <button class="ui-icon-btn" type="button" id="zoom-in" aria-label="Zoom avant" data-tip="Zoom avant (Ctrl +)" data-tip-pos="top" @click=${this.zoomIn}>${icon('zoom')}</button>
        <button class="ui-icon-btn ed-tool-extra" type="button" id="zoom-fit" aria-label="Ajuster le cadre à la vue" data-tip="Ajuster (Maj 1)" data-tip-pos="top" @click=${this.fit}>${icon('agrandir')}</button>
      </div>
    `;
  }

  private renderToolButton(tool: Tool, iconName: 'curseur' | 'cadre' | 'texte' | 'main', label: string, tip: string): TemplateResult {
    return html`<button class="ui-icon-btn" type="button" data-tool-btn=${tool} aria-pressed=${String(this.tool === tool)} aria-label=${label} data-tip=${tip} data-tip-pos="top" @click=${() => { this.tool = tool; }}>${icon(iconName)}</button>`;
  }

  private viewportLabel(): string {
    const variants = Math.max(this.component?.model.variants.length ?? 1, 1);
    return `Canvas de ${this.tag}, ${variantsLabel(variants)}`;
  }

  private observeViewport(): void {
    const viewport = this.viewport;
    if (viewport === this.observed) return;
    this.resizeObserver.disconnect();
    this.observed = viewport;
    if (viewport !== null) this.resizeObserver.observe(viewport);
  }

  private measureViewport(): void {
    const viewport = this.viewport;
    if (viewport === null) return;
    this.viewportWidth = viewport.clientWidth;
    this.viewportHeight = viewport.clientHeight;
    if (this.autoFit) this.fit();
  }

  private readonly fit = (): void => {
    const viewport = this.viewport;
    if (viewport === null) return;
    const fitted = fitView({
      viewportWidth: viewport.clientWidth,
      viewportHeight: viewport.clientHeight,
      frameWidth: screenSizeById(this.screenSize).width,
      frameHeight: this.frameHeight,
    });
    if (fitted === null) return;
    this.view = fitted;
    this.autoFit = true;
  };

  private moveView(view: CanvasView): void {
    this.view = view;
    this.autoFit = false;
  }

  private zoomTo(zoom: number): void {
    const viewport = this.viewport;
    if (viewport === null) return;
    this.moveView(zoomAround(this.view, zoom, viewport.clientWidth / 2, viewport.clientHeight / 2));
  }

  private zoomStep(direction: 1 | -1): void {
    this.zoomTo(nextZoomStep(this.view.zoom, direction));
  }

  private readonly zoomIn = (): void => {
    this.zoomStep(1);
  };

  private readonly zoomOut = (): void => {
    this.zoomStep(-1);
  };

  private readonly resetZoom = (): void => {
    this.zoomTo(1);
  };

  private readonly showInteractionsPending = (): void => {
    this.dispatchEvent(new CustomEvent<NoticeRequestedDetail>(NOTICE_REQUESTED, { detail: { notice: INTERACTIONS_PENDING }, bubbles: true, composed: true }));
  };

  private readonly toggleDarkPreview = (): void => {
    this.darkPreview = !this.darkPreview;
  };

  private readonly onScreenSize = (event: Event): void => {
    const target = event.target;
    if (!(target instanceof HTMLInputElement)) return;
    const chosen = SCREEN_SIZES.find((option) => option.id === target.value);
    if (chosen === undefined) return;
    this.screenSize = chosen.id;
  };

  private readonly onRects = (event: CustomEvent<CanvasRectsDetail>): void => {
    const { rects } = event.detail;
    this.rects = new Map(rects.map((rect) => [rect.id, rect]));
    const bottom = Math.max(0, ...rects.map((rect) => rect.y + rect.height));
    if (bottom > 0) this.frameHeight = Math.min(Math.ceil(bottom) + FRAME_BOTTOM_MARGIN, MAX_FRAME_HEIGHT);
  };

  private readonly onRendered = (): void => {
    this.renderFailed = false;
  };

  private readonly onRenderFailed = (event: CustomEvent<CanvasFailedDetail>): void => {
    this.renderFailed = !event.detail.kept;
  };

  private frameOrigin(): Point | null {
    const frame = this.frame;
    const viewport = this.viewport;
    if (frame === null || viewport === null) return null;
    const frameBox = frame.getBoundingClientRect();
    const viewportBox = viewport.getBoundingClientRect();
    return { x: frameBox.left - viewportBox.left, y: frameBox.top - viewportBox.top };
  }

  private viewportBoxOf(id: string | null, origin: Point): Box | null {
    const rect = id === null ? undefined : this.rects.get(id);
    return rect === undefined ? null : toViewportBox(rect, origin, this.view.zoom);
  }

  private drawOverlay(): void {
    const overlay = this.overlay;
    if (overlay === null) return;
    const origin = this.frameOrigin();
    const selected = origin === null ? null : this.viewportBoxOf(this.selectedId, origin);
    const rect = this.selectedId === null ? undefined : this.rects.get(this.selectedId);
    if (origin === null || selected === null || rect === undefined || this.selectedId === null) {
      overlay.selection = null;
      overlay.guides = [];
    } else {
      overlay.selection = { box: selected, size: sizeLabel(rect) };
      overlay.guides = this.guidesOf(this.selectedId, selected, origin);
    }
    overlay.hover = this.hoverId === this.selectedId || origin === null ? null : this.viewportBoxOf(this.hoverId, origin);
  }

  private guidesOf(id: string, selected: Box, origin: Point): ReturnType<typeof selectionGuides> {
    const isDrawn = (layerId: string): boolean => this.rects.has(layerId);
    const parentId = nearestDrawnAncestor(this.layers, id, isDrawn);
    const { previousId } = neighboursOf(this.layers, id, isDrawn);
    const parent = this.viewportBoxOf(parentId, origin);
    const previous = parent === null ? null : this.viewportBoxOf(previousId, origin);
    return selectionGuides(selected, parent, previous, this.view.zoom);
  }

  private readonly onViewportClick = (event: MouseEvent): void => {
    const frame = this.frame;
    if (frame === null || event.button !== 0 || this.tool !== 'select' || this.spaceDown || this.component === null) return;
    const box = frame.getBoundingClientRect();
    const x = (event.clientX - box.left) / this.view.zoom;
    const y = (event.clientY - box.top) / this.view.zoom;
    void frame.hitTest(x, y).then((id) => {
      this.dispatchEvent(new CustomEvent<CanvasSelectDetail>(CANVAS_SELECT, { detail: { id }, bubbles: true, composed: true }));
    });
  };

  private readonly onWheel = (event: WheelEvent): void => {
    event.preventDefault();
    const viewport = this.viewport;
    if (viewport === null) return;
    if (event.ctrlKey || event.metaKey) {
      const box = viewport.getBoundingClientRect();
      this.moveView(zoomAround(this.view, zoomAfterWheel(this.view.zoom, event.deltaY, event.deltaMode), event.clientX - box.left, event.clientY - box.top));
      return;
    }
    const unit = event.deltaMode === 1 ? LINE_HEIGHT_PX : 1;
    const horizontal = event.shiftKey ? event.deltaY : event.deltaX;
    const vertical = event.shiftKey ? 0 : event.deltaY;
    this.moveView(panBy(this.view, -horizontal * unit, -vertical * unit));
  };

  private readonly onViewportKeydown = (event: KeyboardEvent): void => {
    if (event.target !== this.viewport || event.ctrlKey || event.metaKey) return;
    const step = event.shiftKey ? ARROW_STEP_FAST : ARROW_STEP;
    const moves: Readonly<Record<string, readonly [number, number]>> = {
      ArrowLeft: [step, 0],
      ArrowRight: [-step, 0],
      ArrowUp: [0, step],
      ArrowDown: [0, -step],
    };
    const move = moves[event.key];
    if (move !== undefined) {
      event.preventDefault();
      this.moveView(panBy(this.view, move[0], move[1]));
    } else if (event.key === '+' || event.key === '=') {
      event.preventDefault();
      this.zoomStep(1);
    } else if (event.key === '-') {
      event.preventDefault();
      this.zoomStep(-1);
    }
  };

  private readonly onPointerDown = (event: PointerEvent): void => {
    const viewport = this.viewport;
    const panWithPrimary = this.tool === 'hand' || this.spaceDown;
    if (viewport === null || !(event.button === 1 || (event.button === 0 && panWithPrimary))) return;
    event.preventDefault();
    viewport.setPointerCapture(event.pointerId);
    this.drag = { pointerId: event.pointerId, startX: event.clientX, startY: event.clientY, from: this.view };
    this.panning = true;
  };

  private readonly onPointerMove = (event: PointerEvent): void => {
    const drag = this.drag;
    if (drag?.pointerId !== event.pointerId) return;
    this.moveView({ ...drag.from, x: drag.from.x + event.clientX - drag.startX, y: drag.from.y + event.clientY - drag.startY });
  };

  private readonly endDrag = (event: PointerEvent): void => {
    if (this.drag?.pointerId !== event.pointerId) return;
    this.drag = null;
    this.panning = false;
    this.viewport?.releasePointerCapture(event.pointerId);
  };

  private readonly onDocumentKeydown = (event: KeyboardEvent): void => {
    if (!this.checkVisibility() || this.phase !== 'ready' || isTyping(event)) return;
    if ((event.ctrlKey || event.metaKey) && !event.altKey) {
      this.onZoomKey(event);
      return;
    }
    if (event.shiftKey && event.code === 'Digit0') this.shortcut(event, this.resetZoom);
    else if (event.shiftKey && event.code === 'Digit1') this.shortcut(event, this.fit);
    else if (event.key === 'Escape') this.tool = 'select';
    else if (event.key === ' ' && this.isViewportTarget(event)) this.shortcut(event, () => { this.spaceDown = true; });
    else this.onToolKey(event);
  };

  private onZoomKey(event: KeyboardEvent): void {
    if (event.key === '+' || event.key === '=') this.shortcut(event, () => { this.zoomStep(1); });
    else if (event.key === '-') this.shortcut(event, () => { this.zoomStep(-1); });
  }

  private onToolKey(event: KeyboardEvent): void {
    if (event.altKey || event.repeat || hasOwnKeyboard(event)) return;
    const tool = TOOL_KEYS[event.key.toLowerCase()];
    if (tool !== undefined) this.tool = tool;
  }

  private isViewportTarget(event: KeyboardEvent): boolean {
    const origin = event.composedPath()[0];
    return origin === this.viewport || origin === document.body;
  }

  private shortcut(event: KeyboardEvent, run: () => void): void {
    event.preventDefault();
    run();
  }

  private readonly onDocumentKeyup = (event: KeyboardEvent): void => {
    if (event.key === ' ') this.spaceDown = false;
  };

  private readonly onWindowBlur = (): void => {
    this.spaceDown = false;
  };
}
