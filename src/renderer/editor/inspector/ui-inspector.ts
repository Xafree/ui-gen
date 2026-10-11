import { css, html, LitElement, nothing, type TemplateResult } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import { variantOf } from '../../../modules/editor/domain/code/code-targets.js';
import { inspectorValues, type InspectorKey, type InspectorValues } from '../../../modules/editor/domain/inspector/inspector-values.js';
import type { LayerNode } from '../../../modules/editor/domain/layers/layer-tree.js';
import type { ComponentModel } from '../../../modules/sync/application/component-model.js';
import { icon } from '../../icons/icon.js';
import { editorStyles } from '../editor-styles.js';

declare global {
  interface HTMLElementTagNameMap {
    'ui-inspector': UiInspector;
  }
}

interface PropertyRow {
  readonly id: string;
  readonly label: string;
  readonly key: InspectorKey;
  readonly swatch?: true;
}

interface PropertySection {
  readonly id: string;
  readonly title: string;
  readonly rows: readonly PropertyRow[];
}

const NO_VALUE = '—';
const SWATCH = '[data-swatch]';

const LAYOUT_ROWS: readonly PropertyRow[] = [
  { id: 'width', label: 'Largeur', key: 'width' },
  { id: 'height', label: 'Hauteur', key: 'height' },
  { id: 'dir', label: 'Direction', key: 'direction' },
  { id: 'gap', label: 'Espacement', key: 'gap' },
];

const STYLE_ROWS: readonly PropertyRow[] = [
  { id: 'fill', label: 'Remplissage', key: 'fill', swatch: true },
  { id: 'radius', label: 'Rayon', key: 'radius' },
  { id: 'border', label: 'Bordure', key: 'border' },
  { id: 'shadow', label: 'Ombre', key: 'shadow' },
];

const TYPO_ROWS: readonly PropertyRow[] = [
  { id: 'font', label: 'Police', key: 'fontFamily' },
  { id: 'size', label: 'Taille', key: 'fontSize' },
  { id: 'weight', label: 'Graisse', key: 'fontWeight' },
];

const PADDING: readonly { readonly label: string; readonly key: InspectorKey }[] = [
  { label: 'Vertical', key: 'paddingBlock' },
  { label: 'Horizontal', key: 'paddingInline' },
];

const STATES: readonly { readonly value: string; readonly label: string }[] = [
  { value: 'default', label: 'Par défaut' },
  { value: 'hover', label: 'Survol' },
  { value: 'focus', label: 'Focus' },
  { value: 'disabled', label: 'Désactivé' },
];

const SECTIONS: readonly PropertySection[] = [
  { id: 'sec-layout', title: 'Mise en page auto', rows: LAYOUT_ROWS },
  { id: 'sec-style', title: 'Style', rows: STYLE_ROWS },
  { id: 'sec-typo', title: 'Typographie', rows: TYPO_ROWS },
];

function swatchColor(value: string): string {
  return value.startsWith('--') ? `var(${value})` : value;
}

@customElement('ui-inspector')
export class UiInspector extends LitElement {
  static override readonly styles = [
    ...editorStyles,
    css`
      :host {
        display: flex;
        flex: 1;
        flex-direction: column;
        min-height: 0;
      }
    `,
  ];

  @property({ attribute: false }) model: ComponentModel | null = null;
  @property({ attribute: false }) layer: LayerNode | null = null;

  protected override updated(): void {
    for (const swatch of this.renderRoot.querySelectorAll<HTMLElement>(SWATCH)) {
      swatch.style.setProperty('--swatch', swatch.dataset['swatch'] ?? '');
    }
  }

  override render(): TemplateResult {
    const { layer, model } = this;
    if (layer === null || model === null) return this.renderEmpty();
    const variant = variantOf(layer);
    const values = layer.nodeId === null ? {} : inspectorValues(model, layer.nodeId, variant);
    return html`
      <div class="ed-insp-head" id="insp-head">
        <p class="ed-insp-head__name">${icon('composants')}<span id="insp-name">${layer.name}</span></p>
        <p class="ed-insp-head__sub" id="insp-sub">${variant === null ? '' : `Variante ${variant} · `}${model.tag}.ts</p>
      </div>
      <div class="ui-panel__body">
        ${SECTIONS.map((section) => this.renderSection(section, values))}
        ${this.renderVariants(model, variant)}${this.renderStates()}
      </div>
    `;
  }

  private renderEmpty(): TemplateResult {
    return html`
      <div class="ui-panel__body">
        <div class="ui-empty" id="props-empty">
          <span class="ui-empty__icon">${icon('curseur')}</span>
          <p class="ui-empty__title">Aucun élément sélectionné</p>
        </div>
      </div>
    `;
  }

  private renderSection(section: PropertySection, values: InspectorValues): TemplateResult {
    return html`
      <section class="ui-section" aria-labelledby=${section.id}>
        <div class="ui-section__header"><h3 class="ui-section__title" id=${section.id}>${section.title}</h3></div>
        <div class="ui-props">
          ${section.rows.map((row) => this.renderRow(row, values))}
          ${section.id === 'sec-layout' ? this.renderPadding(values) : nothing}
        </div>
      </section>
    `;
  }

  private renderRow(row: PropertyRow, values: InspectorValues): TemplateResult {
    const value = values[row.key];
    const label = `lbl-${row.id}`;
    return html`
      <div class="ui-prop">
        <span class="ui-prop__label" id=${label}>${row.label}</span>
        <div class="ui-prop__control">
          <span class="ed-readonly" aria-labelledby=${label}>
            ${row.swatch === true && value !== undefined ? html`<span class="ui-swatch" data-swatch=${swatchColor(value)}></span>` : nothing}${value ?? NO_VALUE}
          </span>
        </div>
      </div>
    `;
  }

  private renderPadding(values: InspectorValues): TemplateResult {
    return html`
      <div class="ui-field">
        <span class="ui-label">Padding</span>
        <div class="ed-pad-grid">
          ${PADDING.map(
            ({ label, key }) => html`
              <div class="ui-field">
                <span class="ui-label" id=${`lbl-pad-${key}`}>${label}</span>
                <span class="ed-readonly" aria-labelledby=${`lbl-pad-${key}`}>${values[key] ?? NO_VALUE}</span>
              </div>`,
          )}
        </div>
      </div>
    `;
  }

  private renderVariants(model: ComponentModel, variant: string | null): TemplateResult {
    return html`
      <section class="ui-section" aria-labelledby="sec-variant">
        <div class="ui-section__header">
          <h3 class="ui-section__title" id="sec-variant">Variante</h3>
          ${model.variantProperty === null ? nothing : html`<code class="ed-note">${model.variantProperty}</code>`}
        </div>
        ${model.variants.length === 0
          ? html`<p class="ed-note">Ce composant n'a pas de variante.</p>`
          : html`
              <div class="ui-segmented ui-segmented--block" role="radiogroup" aria-labelledby="sec-variant">
                ${model.variants.map((name) => html`<label><input type="radio" name="p-variant" value=${name} disabled ?checked=${name === variant}><span>${name}</span></label>`)}
              </div>`}
      </section>
    `;
  }

  private renderStates(): TemplateResult {
    return html`
      <section class="ui-section" aria-labelledby="sec-state">
        <div class="ui-section__header">
          <h3 class="ui-section__title" id="sec-state">États</h3>
          <span class="ed-note">Aperçu du canvas</span>
        </div>
        <div class="ed-states" role="radiogroup" aria-labelledby="sec-state">
          ${STATES.map(({ value, label }) => html`<label><input type="radio" name="p-state" value=${value} disabled ?checked=${value === 'default'}><span>${label}</span></label>`)}
        </div>
      </section>
    `;
  }
}
