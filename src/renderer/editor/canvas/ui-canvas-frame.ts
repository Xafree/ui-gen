import { css, html, LitElement, type PropertyValues, type TemplateResult } from 'lit';
import { customElement, property, query } from 'lit/decorators.js';
import { buildHarnessUrl, collectRenderedIds } from '../../../modules/canvas/application/harness-url.js';
import type { HarnessCommand } from '../../../modules/canvas/domain/harness-command.js';
import type { HarnessMessage, NodeRect } from '../../../modules/canvas/domain/harness-message.js';
import type { OpenedComponent } from '../../../modules/sync/application/components/component-views.js';
import { CanvasView } from './canvas-view.js';

declare global {
  interface HTMLElementTagNameMap {
    'ui-canvas-frame': UiCanvasFrame;
  }
}

export const CANVAS_READY = 'canvas-ready';
export const CANVAS_RECTS = 'canvas-rects';
export const RENDER_TIMEOUT_MS = 10_000;

export interface CanvasRectsDetail {
  readonly rects: readonly NodeRect[];
}

@customElement('ui-canvas-frame')
export class UiCanvasFrame extends LitElement {
  static override readonly styles = css`
    :host {
      display: block;
      position: relative;
      min-width: 0;
      min-height: 0;
    }

    :host([hidden]) {
      display: none;
    }

    iframe {
      position: absolute;
      inset: 0;
      width: 100%;
      height: 100%;
      border: 0;
      background: transparent;
    }

    iframe[data-pending] {
      visibility: hidden;
    }
  `;

  @property({ attribute: false }) component: OpenedComponent | null = null;
  @property({ attribute: false }) projectPath = '';
  @property({ type: Boolean }) darkPreview = false;
  @property({ attribute: false }) hiddenLayers: ReadonlySet<string> = new Set();
  @property({ attribute: false }) lockedLayers: ReadonlySet<string> = new Set();

  @query('#stage') private readonly stage!: HTMLElement;

  #front: CanvasView | null = null;
  #pending: CanvasView | null = null;
  #timeout: ReturnType<typeof setTimeout> | null = null;

  override connectedCallback(): void {
    super.connectedCallback();
    if (this.hasUpdated) this.#showVersion();
  }

  override disconnectedCallback(): void {
    this.#discard(this.#pending);
    this.#discard(this.#front);
    this.#pending = null;
    this.#front = null;
    super.disconnectedCallback();
  }

  override render(): TemplateResult {
    return html`<div id="stage"></div>`;
  }

  protected override updated(changed: PropertyValues<this>): void {
    if (changed.has('component') || changed.has('projectPath')) this.#showVersion();
    if (changed.has('darkPreview')) {
      for (const view of [this.#front, this.#pending]) {
        view?.command({ type: 'dark-preview', enabled: this.darkPreview });
      }
    }
    if (changed.has('hiddenLayers') || changed.has('lockedLayers')) this.#front?.command(this.#layerState());
  }

  #layerState(): HarnessCommand {
    return { type: 'layer-state', hidden: [...this.hiddenLayers], locked: [...this.lockedLayers] };
  }

  hitTest(x: number, y: number): Promise<string | null> {
    return this.#front?.hitTest(x, y) ?? Promise.resolve(null);
  }

  #showVersion(): void {
    const component = this.component;
    const url =
      component === null
        ? null
        : buildHarnessUrl({
            projectPath: this.projectPath,
            componentPath: component.path,
            model: component.model,
            dark: this.darkPreview,
          });
    if (component === null || url === null) {
      this.#discard(this.#pending);
      this.#discard(this.#front);
      this.#pending = null;
      this.#front = null;
      return;
    }
    if (this.#front?.url === url) {
      this.#discard(this.#pending);
      this.#pending = null;
      return;
    }
    if (this.#pending?.url === url) return;
    this.#discard(this.#pending);
    const view = new CanvasView(url, collectRenderedIds(component.model), this.#onMessage);
    if (this.#front !== null) view.iframe.dataset['pending'] = '';
    this.#pending = view;
    this.stage.append(view.iframe);
    this.#timeout = setTimeout(() => {
      console.warn('The canvas did not answer in time; the previous rendering is kept');
      this.#discard(this.#pending);
      this.#pending = null;
    }, RENDER_TIMEOUT_MS);
  }

  #discard(view: CanvasView | null): void {
    if (view === null) return;
    if (view === this.#pending && this.#timeout !== null) clearTimeout(this.#timeout);
    view.dispose();
  }

  readonly #onMessage = (view: CanvasView, message: HarnessMessage): void => {
    switch (message.type) {
      case 'ready':
        this.#promote(view);
        break;
      case 'rects':
        if (view === this.#front) {
          this.dispatchEvent(new CustomEvent<CanvasRectsDetail>(CANVAS_RECTS, { detail: { rects: message.rects } }));
        }
        break;
      case 'hit':
        view.settleHit(message.id);
        break;
    }
  };

  #promote(view: CanvasView): void {
    if (view !== this.#pending) return;
    if (this.#timeout !== null) clearTimeout(this.#timeout);
    const previous = this.#front;
    this.#front = view;
    this.#pending = null;
    delete view.iframe.dataset['pending'];
    view.command({ type: 'dark-preview', enabled: this.darkPreview });
    view.command(this.#layerState());
    this.#discard(previous);
    this.dispatchEvent(new CustomEvent(CANVAS_READY));
  }
}
