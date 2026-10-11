import { css, html, LitElement, nothing, type TemplateResult } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import { styleMap } from 'lit/directives/style-map.js';
import { isSmall, type Box, type Guide } from '../../../modules/editor/domain/selection/selection-overlay.js';
import { editorStyles } from '../editor-styles.js';

declare global {
  interface HTMLElementTagNameMap {
    'ui-selection-overlay': UiSelectionOverlay;
  }
}

export interface SelectionOutline {
  readonly box: Box;
  readonly size: string;
}

const HANDLE_POSITIONS = ['tl', 't', 'tr', 'r', 'br', 'b', 'bl', 'l'] as const;

function boxStyle(box: Box): Readonly<Record<string, string>> {
  return {
    left: `${String(box.x)}px`,
    top: `${String(box.y)}px`,
    width: `${String(box.width)}px`,
    height: `${String(box.height)}px`,
  };
}

function guideStyle(guide: Guide): Readonly<Record<string, string>> {
  const length = `${String(guide.length)}px`;
  return {
    left: `${String(guide.x)}px`,
    top: `${String(guide.y)}px`,
    ...(guide.axis === 'horizontal' ? { width: length } : { height: length }),
  };
}

@customElement('ui-selection-overlay')
export class UiSelectionOverlay extends LitElement {
  static override readonly styles = [
    ...editorStyles,
    css`
      :host {
        display: block;
        position: absolute;
        inset: 0;
        z-index: var(--ui-z-canvas-overlay);
        pointer-events: none;
      }
    `,
  ];

  @property({ attribute: false }) selection: SelectionOutline | null = null;
  @property({ attribute: false }) hover: Box | null = null;
  @property({ attribute: false }) guides: readonly Guide[] = [];

  override render(): TemplateResult {
    const { selection, hover } = this;
    return html`
      <div class="ed-overlay" id="overlay" aria-hidden="true">
        <div class="ed-hover" id="hover-box" style=${hover === null ? nothing : styleMap(boxStyle(hover))} ?hidden=${hover === null}></div>
        <div class="ed-guides" id="guides">${this.guides.map((guide) => this.renderGuide(guide))}</div>
        <div class="ed-sel ${selection !== null && isSmall(selection.box) ? 'is-small' : ''}" id="sel-box" style=${selection === null ? nothing : styleMap(boxStyle(selection.box))} ?hidden=${selection === null}>
          ${HANDLE_POSITIONS.map((position) => html`<span class="ui-selection__handle" data-pos=${position}></span>`)}
          <span class="ui-selection__size" id="sel-size">${selection?.size ?? '0 × 0'}</span>
        </div>
      </div>
    `;
  }

  private renderGuide(guide: Guide): TemplateResult {
    return html`<span class="ed-guide ed-guide--${guide.axis === 'horizontal' ? 'h' : 'v'}" style=${styleMap(guideStyle(guide))}><span class="ed-guide__label">${String(guide.label)}</span></span>`;
  }
}
