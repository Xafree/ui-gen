import { css, html, LitElement, type TemplateResult } from 'lit';
import { customElement, property, query } from 'lit/decorators.js';
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

function pixels(value: number): string {
  return `${String(value)}px`;
}

function place(element: HTMLElement, style: Readonly<Record<string, string>>): void {
  element.style.cssText = '';
  for (const [name, value] of Object.entries(style)) element.style.setProperty(name, value);
}

function boxStyle(box: Box): Readonly<Record<string, string>> {
  return { left: pixels(box.x), top: pixels(box.y), width: pixels(box.width), height: pixels(box.height) };
}

function guideStyle(guide: Guide): Readonly<Record<string, string>> {
  const length = pixels(guide.length);
  return { left: pixels(guide.x), top: pixels(guide.y), ...(guide.axis === 'horizontal' ? { width: length } : { height: length }) };
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

  @query('#hover-box') private readonly hoverBox!: HTMLElement;
  @query('#sel-box') private readonly selectionBox!: HTMLElement;

  override render(): TemplateResult {
    const { selection, hover } = this;
    return html`
      <div class="ed-overlay" id="overlay" aria-hidden="true">
        <div class="ed-hover" id="hover-box" ?hidden=${hover === null}></div>
        <div class="ed-guides" id="guides">${this.guides.map((guide) => this.renderGuide(guide))}</div>
        <div class="ed-sel ${selection !== null && isSmall(selection.box) ? 'is-small' : ''}" id="sel-box" ?hidden=${selection === null}>
          ${HANDLE_POSITIONS.map((position) => html`<span class="ui-selection__handle" data-pos=${position}></span>`)}
          <span class="ui-selection__size" id="sel-size">${selection?.size ?? '0 × 0'}</span>
        </div>
      </div>
    `;
  }

  protected override updated(): void {
    if (this.hover !== null) place(this.hoverBox, boxStyle(this.hover));
    if (this.selection !== null) place(this.selectionBox, boxStyle(this.selection.box));
    const guideElements = this.renderRoot.querySelectorAll<HTMLElement>('.ed-guide');
    this.guides.forEach((guide, index) => {
      const element = guideElements[index];
      if (element !== undefined) place(element, guideStyle(guide));
    });
  }

  private renderGuide(guide: Guide): TemplateResult {
    return html`<span class="ed-guide ed-guide--${guide.axis === 'horizontal' ? 'h' : 'v'}"><span class="ed-guide__label">${String(guide.label)}</span></span>`;
  }
}
