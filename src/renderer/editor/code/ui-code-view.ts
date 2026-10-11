import { css, html, LitElement, type PropertyValues, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import { splitCodeLines, type CodeLine, type CodeSegment } from '../../../modules/editor/domain/code/code-lines.js';
import { nodeIdAtLine, touchesLine, type TextRange } from '../../../modules/editor/domain/code/code-targets.js';
import type { NodeId, TemplateNode } from '../../../modules/sync/application/component-model.js';
import type { HighlightToken } from '../../../modules/sync/ports/code-highlighter.js';
import { studioStyles } from '../../styles/studio-styles.js';

declare global {
  interface HTMLElementTagNameMap {
    'ui-code-view': UiCodeView;
  }
}

export const CODE_LINE_SELECT = 'code-line-select';
export const CODE_CURSOR = 'code-cursor';

export interface CodeLineSelectDetail {
  readonly nodeId: NodeId;
}

export interface CodeCursorDetail {
  readonly line: number;
  readonly column: number;
}

const GUTTER_WIDTH = 72;
const PAGE_LINES = 10;
const END_OF_LINE = Number.MAX_SAFE_INTEGER;

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), maximum);
}

function characterWidth(pre: HTMLElement): number {
  const context = document.createElement('canvas').getContext('2d');
  if (context === null) return 0;
  context.font = getComputedStyle(pre).font;
  return context.measureText('M').width;
}

function renderSegment(segment: CodeSegment): TemplateResult | string {
  return segment.className === null ? segment.text : html`<span class=${segment.className}>${segment.text}</span>`;
}

@customElement('ui-code-view')
export class UiCodeView extends LitElement {
  static override readonly styles = [
    studioStyles,
    css`
      :host {
        display: flex;
        flex: 1;
        flex-direction: column;
        min-height: 0;
      }

      .ui-code {
        flex: 1;
      }

      .ui-code__pre {
        position: relative;
        padding-bottom: var(--ui-space-4);
      }

      .ui-code__line {
        cursor: text;
      }
    `,
  ];

  @property({ attribute: false }) path = '';
  @property({ attribute: false }) label = '';
  @property({ attribute: false }) text = '';
  @property({ attribute: false }) highlight: readonly HighlightToken[] = [];
  @property({ attribute: false }) template: readonly TemplateNode[] = [];
  @property({ attribute: false }) targetRange: TextRange | null = null;

  @state() private line = 1;
  @state() private column = 1;

  private lines: readonly CodeLine[] = [];
  private notified = '';

  override render(): TemplateResult {
    return html`
      <figure class="ui-code" id="code-figure" aria-label=${`Code Lit : ${this.label}`}>
        <pre class="ui-code__pre" id="code-pre" tabindex="0" aria-describedby="code-help" @keydown=${this.onKeydown} @click=${this.onClick}><code id="code">${this.lines.map((line) => this.renderLine(line))}</code></pre>
        <p class="sr-only" id="code-help">Haut et bas : déplacer le curseur d'une ligne. Entrée : sélectionner sur le canvas l'élément de la ligne.</p>
      </figure>
    `;
  }

  scrollToCursor(): void {
    this.scrollLineIntoView(this.lineElement(this.line));
  }

  protected override willUpdate(changed: PropertyValues<this>): void {
    if (changed.has('text') || changed.has('highlight')) this.lines = splitCodeLines(this.text, this.highlight);
    if (changed.has('path')) {
      this.line = 1;
      this.column = 1;
    } else if (changed.has('text')) {
      this.line = clamp(this.line, 1, this.lines.length);
      this.column = clamp(this.column, 1, this.currentLength + 1);
    }
  }

  protected override updated(changed: PropertyValues<this>): void {
    const position = `${String(this.line)}:${String(this.column)}`;
    if (position !== this.notified) {
      this.notified = position;
      this.dispatchEvent(new CustomEvent<CodeCursorDetail>(CODE_CURSOR, { detail: { line: this.line, column: this.column }, bubbles: true, composed: true }));
    }
    if (changed.has('targetRange') && this.targetRange !== null) {
      this.scrollLineIntoView(this.renderRoot.querySelector<HTMLElement>('.ui-code__line.is-target'));
    }
  }

  private get currentLength(): number {
    return this.lines[this.line - 1]?.text.length ?? 0;
  }

  private renderLine(line: CodeLine): TemplateResult {
    const target = this.targetRange !== null && touchesLine(this.targetRange, line);
    const classes = `ui-code__line${target ? ' is-target' : ''}${line.number === this.line ? ' is-active' : ''}`;
    return html`<span class=${classes} data-i=${String(line.number - 1)}>${line.segments.map(renderSegment)}</span>`;
  }

  private lineElement(number: number): HTMLElement | null {
    return this.renderRoot.querySelector<HTMLElement>(`.ui-code__line[data-i="${String(number - 1)}"]`);
  }

  private scrollLineIntoView(element: HTMLElement | null): void {
    const pre = this.renderRoot.querySelector<HTMLElement>('#code-pre');
    if (element === null || !pre?.checkVisibility()) return;
    const top = element.offsetTop;
    const height = pre.clientHeight;
    if (top < pre.scrollTop + 8 || top + 28 > pre.scrollTop + height) pre.scrollTop = Math.max(0, top - height / 2 + 10);
  }

  private moveTo(line: number, column: number | null): void {
    this.line = clamp(line, 1, this.lines.length);
    const text = this.lines[this.line - 1]?.text ?? '';
    this.column = clamp(column ?? this.column, 1, text.length + 1);
    void this.updateComplete.then(() => {
      this.scrollLineIntoView(this.lineElement(this.line));
    });
  }

  private selectLine(number: number): void {
    const line = this.lines[number - 1];
    const nodeId = line === undefined ? null : nodeIdAtLine(this.template, line);
    if (nodeId === null) return;
    this.dispatchEvent(new CustomEvent<CodeLineSelectDetail>(CODE_LINE_SELECT, { detail: { nodeId }, bubbles: true, composed: true }));
  }

  private readonly onClick = (event: MouseEvent): void => {
    const element = event.target instanceof Element ? event.target.closest<HTMLElement>('.ui-code__line') : null;
    const pre = event.currentTarget;
    if (element === null || !(pre instanceof HTMLElement)) return;
    const number = Number(element.dataset['i']) + 1;
    const width = characterWidth(pre);
    const offset = event.clientX - element.getBoundingClientRect().left - GUTTER_WIDTH;
    this.moveTo(number, width > 0 ? Math.round(offset / width) + 1 : 1);
    this.selectLine(number);
  };

  private readonly onKeydown = (event: KeyboardEvent): void => {
    if (event.target !== event.currentTarget || event.altKey) return;
    const jump = event.ctrlKey || event.metaKey;
    const moves: Readonly<Record<string, () => void>> = {
      ArrowUp: () => { this.moveTo(this.line - 1, null); },
      ArrowDown: () => { this.moveTo(this.line + 1, null); },
      PageUp: () => { this.moveTo(this.line - PAGE_LINES, null); },
      PageDown: () => { this.moveTo(this.line + PAGE_LINES, null); },
      ArrowLeft: () => { this.moveTo(this.line, this.column - 1); },
      ArrowRight: () => { this.moveTo(this.line, this.column + 1); },
      Home: () => { if (jump) this.moveTo(1, 1); else this.moveTo(this.line, 1); },
      End: () => { this.moveTo(jump ? this.lines.length : this.line, END_OF_LINE); },
      Enter: () => { this.selectLine(this.line); },
    };
    const move = Object.hasOwn(moves, event.key) ? moves[event.key] : undefined;
    if (move === undefined) return;
    event.preventDefault();
    move();
  };
}
