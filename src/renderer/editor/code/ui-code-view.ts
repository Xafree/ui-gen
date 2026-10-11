import { css, html, LitElement, type PropertyValues, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import { markLines, type Diagnostic, type LineDiagnostics } from '../../../modules/editor/domain/code/code-diagnostics.js';
import { splitCodeLines, type CodeLine, type CodeSegment } from '../../../modules/editor/domain/code/code-lines.js';
import { nodeIdAtLine, touchesLine, type TextRange } from '../../../modules/editor/domain/code/code-targets.js';
import type { TemplateNode } from '../../../modules/sync/application/component-model.js';
import { diagnosticMeta, diagnosticTitle } from '../../../modules/editor/domain/code/problem-summary.js';
import type { HighlightToken } from '../../../modules/sync/ports/code-highlighter.js';
import { icon } from '../../icons/icon.js';
import { studioStyles } from '../../styles/studio-styles.js';

declare global {
  interface HTMLElementTagNameMap {
    'ui-code-view': UiCodeView;
  }
}

export const CODE_LINE_SELECT = 'code-line-select';
export const CODE_CURSOR = 'code-cursor';

export interface CodeLineSelectDetail {
  readonly nodeId: string;
}

export interface CodeCursorDetail {
  readonly line: number;
  readonly column: number;
}

const GUTTER_WIDTH = 72;
const PAGE_LINES = 10;
const END_OF_LINE = Number.MAX_SAFE_INTEGER;
const CARD_MARGIN = 16;
const CARD_GAP = 6;
const CARD_BOTTOM_ROOM = 32;

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

      .ed-diag-card {
        position: fixed;
        z-index: var(--ui-z-popover);
        width: min(420px, calc(100vw - 32px));
      }
    `,
  ];

  @property({ attribute: false }) path = '';
  @property({ attribute: false }) label = '';
  @property({ attribute: false }) text = '';
  @property({ attribute: false }) highlight: readonly HighlightToken[] = [];
  @property({ attribute: false }) template: readonly TemplateNode[] = [];
  @property({ attribute: false }) targetRange: TextRange | null = null;
  @property({ attribute: false }) diagnostics: readonly Diagnostic[] = [];

  @state() private line = 1;
  @state() private column = 1;
  @state() private shown: number | null = null;

  private lines: readonly CodeLine[] = [];
  private marks: readonly (LineDiagnostics | null)[] = [];
  private anchor: DOMRect | null = null;
  private notified = '';
  private shownScrollTop = 0;

  override render(): TemplateResult {
    return html`
      <figure class="ui-code" id="code-figure" aria-label=${`Code Lit : ${this.label}`}>
        <pre class="ui-code__pre" id="code-pre" tabindex="0" aria-describedby="code-help" @keydown=${this.onKeydown} @click=${this.onClick} @mouseover=${this.onMouseOver} @mouseout=${this.onMouseOut} @focusin=${this.onFocusIn} @focusout=${this.onFocusOut} @scroll=${this.onScroll}><code id="code">${this.lines.map((line) => this.renderLine(line))}</code></pre>
        <p class="sr-only" id="code-help">Haut et bas : déplacer le curseur d'une ligne. Entrée : sélectionner sur le canvas l'élément de la ligne.</p>
      </figure>
      ${this.renderDiagnosticCard()}
    `;
  }

  focusDiagnostic(index: number): void {
    const diagnostic = this.diagnostics[index];
    if (diagnostic === undefined) return;
    this.moveTo(diagnostic.line, diagnostic.column);
    this.renderRoot.querySelector<HTMLElement>(`[data-diag="${String(index)}"]`)?.focus({ preventScroll: true });
  }

  scrollToCursor(): void {
    this.scrollLineIntoView(this.lineElement(this.line));
  }

  protected override willUpdate(changed: PropertyValues<this>): void {
    if (changed.has('text') || changed.has('highlight')) this.lines = splitCodeLines(this.text, this.highlight);
    if (changed.has('text') || changed.has('highlight') || changed.has('diagnostics')) {
      this.marks = markLines(this.lines, this.diagnostics);
      this.shown = null;
    }
    if (changed.has('path')) {
      this.line = 1;
      this.column = 1;
    } else if (changed.has('text')) {
      this.line = clamp(this.line, 1, this.lines.length);
      this.column = clamp(this.column, 1, this.currentLength + 1);
    }
    if (changed.has('targetRange')) this.followTarget();
  }

  protected override updated(changed: PropertyValues<this>): void {
    const position = `${String(this.line)}:${String(this.column)}`;
    if (position !== this.notified) {
      this.notified = position;
      this.dispatchEvent(new CustomEvent<CodeCursorDetail>(CODE_CURSOR, { detail: { line: this.line, column: this.column }, bubbles: true, composed: true }));
    }
    if (this.anchor !== null) this.placeCard();
    if (changed.has('targetRange') && this.targetRange !== null) {
      this.scrollLineIntoView(this.renderRoot.querySelector<HTMLElement>('.ui-code__line.is-target'));
    }
  }

  private followTarget(): void {
    const range = this.targetRange;
    const current = this.lines[this.line - 1];
    if (range === null || (current !== undefined && touchesLine(range, current))) return;
    const opening = this.lines.find((line) => line.start <= range.start && range.start <= line.end);
    if (opening === undefined) return;
    this.line = opening.number;
    this.column = range.start - opening.start + 1;
  }

  private get currentLength(): number {
    return this.lines[this.line - 1]?.text.length ?? 0;
  }

  private renderLine(line: CodeLine): TemplateResult {
    const target = this.targetRange !== null && touchesLine(this.targetRange, line);
    const classes = `ui-code__line${target ? ' is-target' : ''}${line.number === this.line ? ' is-active' : ''}`;
    const marks = this.marks[line.number - 1] ?? null;
    const marked = marks === null ? '' : ` has-${marks.severity}`;
    return html`<span class=${classes + marked} data-i=${String(line.number - 1)}>${marks === null ? line.segments.map(renderSegment) : this.renderMarked(line, marks)}</span>`;
  }

  private renderMarked(line: CodeLine, marks: LineDiagnostics): TemplateResult {
    const lensId = `diag-lens-${String(line.number)}`;
    const fragments = marks.fragments.map((fragment) => {
      const segments = fragment.segments.map(renderSegment);
      if (fragment.diagnostic === null) return segments;
      const warning = this.diagnostics[fragment.diagnostic]?.severity !== 'error';
      return html`<span class=${warning ? 'ui-diag ui-diag--warning' : 'ui-diag'} tabindex="0" aria-describedby=${lensId} data-diag=${String(fragment.diagnostic)}>${segments}</span>`;
    });
    return html`${fragments}<span class=${marks.severity === 'warning' ? 'ui-code__lens ui-code__lens--warning' : 'ui-code__lens'} id=${lensId}>${marks.message}</span>`;
  }

  private renderDiagnosticCard(): TemplateResult {
    const diagnostic = this.shown === null ? undefined : this.diagnostics[this.shown];
    const warning = diagnostic?.severity !== 'error';
    return html`
      <div class=${warning ? 'ui-diag-card ui-diag-card--warning ed-diag-card' : 'ui-diag-card ed-diag-card'} id="diag-card" role="tooltip" ?hidden=${diagnostic === undefined}>
        <p class="ui-diag-card__title">${icon(warning ? 'alerte' : 'erreur')}<span id="diag-title">${diagnostic === undefined ? '' : diagnosticTitle(diagnostic)}</span></p>
        <p class="ui-diag-card__text" id="diag-text">${diagnostic?.message ?? ''}</p>
        <p class="ui-diag-card__meta" id="diag-meta">${diagnostic === undefined ? '' : diagnosticMeta(this.label, diagnostic)}</p>
      </div>
    `;
  }

  private placeCard(): void {
    const card = this.renderRoot.querySelector<HTMLElement>('#diag-card');
    const anchor = this.anchor;
    this.anchor = null;
    if (card === null || anchor === null) return;
    const left = clamp(anchor.left, CARD_MARGIN, window.innerWidth - card.offsetWidth - CARD_MARGIN);
    const below = anchor.bottom + CARD_GAP;
    const top = below + card.offsetHeight > window.innerHeight - CARD_BOTTOM_ROOM ? anchor.top - card.offsetHeight - CARD_GAP : below;
    card.style.left = `${String(left)}px`;
    card.style.top = `${String(Math.max(8, top))}px`;
  }

  private showDiagnostic(target: EventTarget | null): void {
    const mark = target instanceof Element ? target.closest<HTMLElement>('[data-diag]') : null;
    const index = Number(mark?.dataset['diag']);
    if (mark === null || Number.isNaN(index)) return;
    this.anchor = mark.getBoundingClientRect();
    this.shownScrollTop = mark.closest('pre')?.scrollTop ?? 0;
    this.shown = index;
  }

  private readonly hideDiagnostic = (): void => {
    this.shown = null;
  };

  private readonly onScroll = (event: Event): void => {
    const pre = event.currentTarget;
    if (pre instanceof HTMLElement && pre.scrollTop !== this.shownScrollTop) this.hideDiagnostic();
  };

  private readonly onMouseOver = (event: MouseEvent): void => {
    this.showDiagnostic(event.target);
  };

  private readonly onMouseOut = (event: MouseEvent): void => {
    const mark = event.target instanceof Element ? event.target.closest('[data-diag]') : null;
    if (mark !== null && !(event.relatedTarget instanceof Node && mark.contains(event.relatedTarget))) this.hideDiagnostic();
  };

  private readonly onFocusIn = (event: FocusEvent): void => {
    this.showDiagnostic(event.target);
  };

  private readonly onFocusOut = (event: FocusEvent): void => {
    if (event.target instanceof Element && event.target.closest('[data-diag]') !== null) this.hideDiagnostic();
  };

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
    if (event.key === 'Escape') this.hideDiagnostic();
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
