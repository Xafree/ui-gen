import { html, LitElement, nothing, type PropertyValues, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import { findLayer } from '../../../modules/editor/domain/layers/layer-lookup.js';
import type { LayerKind, LayerNode } from '../../../modules/editor/domain/layers/layer-tree.js';
import { icon } from '../../icons/icon.js';
import type { IconName } from '../../icons/icon-paths.js';
import { editorStyles } from '../editor-styles.js';

declare global {
  interface HTMLElementTagNameMap {
    'ui-layer-tree': UiLayerTree;
  }
}

export const LAYER_SELECT = 'layer-select';
export const LAYER_HOVER = 'layer-hover';
export const LAYER_HIDE = 'layer-hide';
export const LAYER_LOCK = 'layer-lock';

export interface LayerIdDetail {
  readonly id: string;
}

export interface LayerHoverDetail {
  readonly id: string | null;
}

interface KeyboardPosition {
  readonly id: string;
  readonly index: number;
  readonly visible: readonly string[];
  readonly layer: LayerNode | undefined;
}

const KIND_ICON: Readonly<Record<LayerKind, IconName>> = {
  set: 'variantes',
  variant: 'cadre',
  container: 'empiler',
  text: 'texte',
  instance: 'composants',
  binding: 'code',
  conditional: 'code',
  opaque: 'code',
};

const ROOT_LEVEL = 1;

function layerLabel(name: string, hidden: boolean, locked: boolean): string {
  return `${name}${hidden ? ', masqué' : ''}${locked ? ', verrouillé' : ''}`;
}

function itemOf(target: EventTarget | null): HTMLElement | null {
  return target instanceof Element ? target.closest<HTMLElement>('[role="treeitem"]') : null;
}

@customElement('ui-layer-tree')
export class UiLayerTree extends LitElement {
  static override readonly styles = editorStyles;

  @property({ attribute: false }) layers: readonly LayerNode[] = [];
  @property({ attribute: false }) tag = '';
  @property({ attribute: false }) selectedId: string | null = null;
  @property({ attribute: false }) hiddenLayers: ReadonlySet<string> = new Set();
  @property({ attribute: false }) lockedLayers: ReadonlySet<string> = new Set();

  @state() private collapsed: ReadonlySet<string> = new Set();
  @state() private tabStopId: string | null = null;

  private hoveredId: string | null = null;

  override render(): TemplateResult {
    const tabStop = this.currentTabStop();
    return html`
      <ul class="ui-tree" role="tree" id="tree" aria-label=${`Calques de ${this.tag}`} @click=${this.onClick} @keydown=${this.onKeydown} @mouseover=${this.onMouseOver} @mouseleave=${this.onMouseLeave}>
        ${this.layers.map((layer) => this.renderLayer(layer, ROOT_LEVEL, tabStop))}
      </ul>
    `;
  }

  protected override willUpdate(changed: PropertyValues<this>): void {
    if (changed.has('selectedId') && this.selectedId !== null) this.expandToSelection(this.selectedId);
  }

  protected override updated(changed: PropertyValues<this>): void {
    if (changed.has('selectedId') && this.selectedId !== null) this.revealSelection(this.selectedId);
  }

  private renderLayer(layer: LayerNode, level: number, tabStop: string | null): TemplateResult {
    const hidden = this.hiddenLayers.has(layer.id);
    const locked = this.lockedLayers.has(layer.id);
    const expandable = layer.children.length > 0;
    const expanded = !this.collapsed.has(layer.id);
    return html`
      <li role="treeitem" data-id=${layer.id} aria-label=${layerLabel(layer.name, hidden, locked)} aria-selected=${String(layer.id === this.selectedId)} aria-expanded=${expandable ? String(expanded) : nothing} tabindex=${layer.id === tabStop ? '0' : '-1'}>
        <div class="ui-tree__row ${hidden ? 'is-muted' : ''}" style=${`--level:${String(level)}`}>
          ${expandable
            ? html`<button class="ui-tree__toggle" type="button" tabindex="-1" aria-hidden="true">${icon('chevron-droite')}</button>`
            : html`<span class="ui-tree__spacer"></span>`}
          ${icon(KIND_ICON[layer.kind], 'ui-tree__icon')}
          <span class="ui-tree__name">${layer.name}</span>
          ${layer.meta === null ? nothing : html`<span class="ui-tree__meta">${layer.meta}</span>`}
          ${layer.kind === 'set' ? nothing : this.renderActions(layer, hidden, locked)}
        </div>
        ${expandable ? html`<ul role="group">${layer.children.map((child) => this.renderLayer(child, level + 1, tabStop))}</ul>` : nothing}
      </li>
    `;
  }

  private renderActions(layer: LayerNode, hidden: boolean, locked: boolean): TemplateResult {
    const hideVerb = hidden ? 'Afficher' : 'Masquer';
    const lockVerb = locked ? 'Déverrouiller' : 'Verrouiller';
    return html`
      <span class="ui-tree__actions ${hidden || locked ? 'is-pinned' : ''}">
        <button class="ui-icon-btn" type="button" tabindex="-1" data-act="hide" aria-label=${`${hideVerb} le calque ${layer.name}`} data-tip=${`${hideVerb} (Ctrl Maj H)`} data-tip-pos="end">${icon(hidden ? 'oeil-barre' : 'oeil')}</button>
        <button class="ui-icon-btn" type="button" tabindex="-1" data-act="lock" aria-label=${`${lockVerb} le calque ${layer.name}`} data-tip=${`${lockVerb} (Ctrl Maj L)`} data-tip-pos="end">${icon(locked ? 'verrou' : 'verrou-ouvert')}</button>
      </span>
    `;
  }

  private visibleIds(): string[] {
    const visible: string[] = [];
    const walk = (layers: readonly LayerNode[]): void => {
      for (const layer of layers) {
        visible.push(layer.id);
        if (!this.collapsed.has(layer.id)) walk(layer.children);
      }
    };
    walk(this.layers);
    return visible;
  }

  private currentTabStop(): string | null {
    const visible = this.visibleIds();
    const wanted = [this.tabStopId, this.selectedId].find((id) => id !== null && visible.includes(id));
    return wanted ?? visible[0] ?? null;
  }

  private expandToSelection(id: string): void {
    const ancestors = this.ancestorsOf(id);
    if (ancestors.some((ancestor) => this.collapsed.has(ancestor))) {
      this.collapsed = new Set([...this.collapsed].filter((collapsedId) => !ancestors.includes(collapsedId)));
    }
    if (this.renderRoot.querySelector(':focus-within') === null) this.tabStopId = id;
  }

  private ancestorsOf(id: string): string[] {
    const path: string[] = [];
    const search = (layers: readonly LayerNode[], trail: readonly string[]): boolean =>
      layers.some((layer) => {
        if (layer.id === id) {
          path.push(...trail);
          return true;
        }
        return search(layer.children, [...trail, layer.id]);
      });
    search(this.layers, []);
    return path;
  }

  private revealSelection(id: string): void {
    const item = this.renderRoot.querySelector<HTMLElement>(`[role="treeitem"][data-id="${CSS.escape(id)}"]`);
    item?.scrollIntoView({ block: 'nearest' });
  }

  private focusLayer(id: string): void {
    this.tabStopId = id;
    void this.updateComplete.then(() => {
      const items = [...this.renderRoot.querySelectorAll<HTMLElement>('[role="treeitem"]')];
      items.find((item) => item.dataset['id'] === id)?.focus();
    });
  }

  private setCollapsed(id: string, collapse: boolean): void {
    const next = new Set(this.collapsed);
    if (collapse) next.add(id);
    else next.delete(id);
    this.collapsed = next;
  }

  private emit(type: string, detail: LayerHoverDetail): void {
    this.dispatchEvent(new CustomEvent<LayerHoverDetail>(type, { detail, bubbles: true, composed: true }));
  }

  private select(id: string): void {
    this.emit(LAYER_SELECT, { id });
  }

  private focusAt(id: string | undefined): void {
    if (id !== undefined) this.focusLayer(id);
  }

  private readonly onClick = (event: MouseEvent): void => {
    const item = itemOf(event.target);
    const id = item?.dataset['id'];
    if (id === undefined || !(event.target instanceof Element)) return;
    const action = event.target.closest<HTMLElement>('[data-act]');
    if (action !== null) {
      this.emit(action.dataset['act'] === 'hide' ? LAYER_HIDE : LAYER_LOCK, { id });
    } else if (event.target.closest('.ui-tree__toggle') === null) {
      this.select(id);
      this.focusLayer(id);
    } else {
      this.setCollapsed(id, !this.collapsed.has(id));
      this.focusLayer(id);
    }
  };

  private readonly onMouseOver = (event: MouseEvent): void => {
    const id = itemOf(event.target)?.dataset['id'] ?? null;
    if (id === this.hoveredId) return;
    this.hoveredId = id;
    this.emit(LAYER_HOVER, { id });
  };

  private readonly onMouseLeave = (): void => {
    if (this.hoveredId === null) return;
    this.hoveredId = null;
    this.emit(LAYER_HOVER, { id: null });
  };

  private readonly onKeydown = (event: KeyboardEvent): void => {
    const item = itemOf(event.target);
    const id = item?.dataset['id'];
    if (id === undefined || event.target !== item || event.ctrlKey || event.metaKey || event.altKey) return;
    const visible = this.visibleIds();
    const position: KeyboardPosition = { id, index: visible.indexOf(id), visible, layer: findLayer(this.layers, id) };
    if (this.navigate(event.key, position)) event.preventDefault();
  };

  private navigate(key: string, at: KeyboardPosition): boolean {
    const expandable = (at.layer?.children.length ?? 0) > 0;
    const expanded = !this.collapsed.has(at.id);
    switch (key) {
      case 'ArrowDown':
        this.focusAt(at.visible[at.index + 1]);
        return true;
      case 'ArrowUp':
        this.focusAt(at.visible[at.index - 1]);
        return true;
      case 'Home':
        this.focusAt(at.visible[0]);
        return true;
      case 'End':
        this.focusAt(at.visible.at(-1));
        return true;
      case 'ArrowRight':
        if (expandable && !expanded) this.setCollapsed(at.id, false);
        else if (expandable) this.focusAt(at.layer?.children[0]?.id);
        return true;
      case 'ArrowLeft':
        if (expandable && expanded) this.setCollapsed(at.id, true);
        else this.focusAt(this.ancestorsOf(at.id).at(-1));
        return true;
      case 'Enter':
      case ' ':
        this.select(at.id);
        return true;
      default:
        return false;
    }
  }
}
