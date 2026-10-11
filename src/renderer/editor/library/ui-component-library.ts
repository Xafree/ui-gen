import { html, LitElement, nothing, type PropertyValues, type TemplateResult } from 'lit';
import { customElement, property, query, state } from 'lit/decorators.js';
import { componentLabel, filterComponents } from '../../../modules/editor/domain/library/component-label.js';
import { samePath, type ComponentSummary } from '../../../modules/sync/domain/library/component-summary.js';
import { icon } from '../../icons/icon.js';
import { editorStyles } from '../editor-styles.js';

declare global {
  interface HTMLElementTagNameMap {
    'ui-component-library': UiComponentLibrary;
  }
}

export const COMPONENT_OPEN = 'component-open';

export interface ComponentOpenDetail {
  readonly path: string;
}

export const EMPTY_LIBRARY_TITLE = 'Aucun composant dans cette bibliothèque';
export const IMPORT_HINT = "Importez un composant depuis MUI, Material ou shadcn, ou demandez-en un à l'agent.";
export const IMPORT_SCREEN = 'import-librairie.html';

function neighbour(options: readonly HTMLElement[], index: number, key: string): HTMLElement | undefined {
  switch (key) {
    case 'ArrowDown':
      return options[Math.min(options.length - 1, index + 1)];
    case 'ArrowUp':
      return options[Math.max(0, index - 1)];
    case 'Home':
      return options[0];
    case 'End':
      return options.at(-1);
    default:
      return undefined;
  }
}

function pathOf(target: EventTarget | null): string | undefined {
  return target instanceof Element ? target.closest<HTMLElement>('[role="option"]')?.dataset['path'] : undefined;
}

@customElement('ui-component-library')
export class UiComponentLibrary extends LitElement {
  static override readonly styles = editorStyles;

  @property({ attribute: false }) components: readonly ComponentSummary[] = [];
  @property({ attribute: false }) openedPath: string | null = null;

  @state() private search = '';
  @state() private chosenPath: string | null = null;

  @query('#comp-search') private readonly searchInput!: HTMLInputElement;

  override render(): TemplateResult {
    const shown = filterComponents(this.components, this.search);
    const chosen = this.chosenComponent();
    return html`
      <div class="ed-comp-search">
        <div class="ui-input-group">
          <span class="ui-input-group__prefix">${icon('recherche')}</span>
          <input class="ui-input" type="search" id="comp-search" placeholder="Rechercher, par exemple Switch…" aria-label="Rechercher un composant" autocomplete="off" spellcheck="false" .value=${this.search} @input=${this.onSearchInput} @keydown=${this.onSearchKeydown}>
        </div>
      </div>
      <div class="ui-panel__body">
        <ul class="ed-comp-list" role="listbox" id="comp-list" aria-label="Composants du projet" ?hidden=${shown.length === 0} @click=${this.onListClick} @keydown=${this.onListKeydown}>
          ${shown.map((component) => this.renderOption(component, shown))}
        </ul>
        ${shown.length === 0 ? this.renderEmpty() : nothing}
      </div>
      ${chosen === undefined ? nothing : this.renderDetail(chosen)}
    `;
  }

  protected override willUpdate(changed: PropertyValues<this>): void {
    if (changed.has('openedPath')) this.chosenPath = this.openedPath;
  }

  private isChosen(component: ComponentSummary): boolean {
    const path = this.chosenPath ?? this.openedPath;
    return path !== null && samePath(component.path, path);
  }

  private isOpened(component: ComponentSummary): boolean {
    return this.openedPath !== null && samePath(component.path, this.openedPath);
  }

  private chosenComponent(): ComponentSummary | undefined {
    return this.components.find((component) => this.isChosen(component));
  }

  private renderOption(component: ComponentSummary, shown: readonly ComponentSummary[]): TemplateResult {
    const chosen = this.isChosen(component);
    const anchor = shown.some((candidate) => this.isChosen(candidate)) ? chosen : component === shown[0];
    return html`<li class="ed-comp" role="option" data-path=${component.path} aria-selected=${String(chosen)} tabindex=${anchor ? '0' : '-1'}>${icon('composants')}<span class="ed-comp__body"><span class="ed-comp__name">${componentLabel(component)}</span><span class="ed-comp__tag">&lt;${component.tag}&gt;</span></span>${this.isOpened(component) ? html`<span class="ed-comp__mark">Ouvert</span>` : nothing}</li>`;
  }

  private renderEmpty(): TemplateResult {
    const library = this.components.length === 0;
    return html`
      <div class="ui-empty" id="comp-empty">
        <span class="ui-empty__icon">${icon('composants')}</span>
        <p class="ui-empty__title" id="comp-empty-title">${library ? EMPTY_LIBRARY_TITLE : `Aucun composant ne contient « ${this.search.trim()} »`}</p>
        <p class="ui-empty__text" id="comp-empty-text">${library ? IMPORT_HINT : 'Effacez la recherche ou importez un composant depuis MUI, Material ou shadcn.'}</p>
        <div class="ui-empty__actions">
          ${library ? nothing : html`<button class="ui-btn ui-btn--secondary ui-btn--sm" type="button" id="comp-clear" @click=${this.clearSearch}>Effacer la recherche</button>`}
          <a class="ui-btn ${library ? 'ui-btn--secondary' : 'ui-btn--ghost'} ui-btn--sm" href=${IMPORT_SCREEN}>Importer un composant</a>
        </div>
      </div>
    `;
  }

  private renderDetail(component: ComponentSummary): TemplateResult {
    const opened = this.isOpened(component);
    return html`
      <div class="ed-comp-detail" id="comp-detail" aria-live="polite">
        <p class="ed-comp-detail__title">${icon('composants')}${componentLabel(component)}<span class="ui-badge ${opened ? 'ui-badge--info' : 'ui-badge--success'}">${opened ? 'Ouvert' : 'Prêt'}</span></p>
        <p class="ed-comp-detail__path">${component.path}</p>
      </div>
    `;
  }

  private options(): HTMLElement[] {
    return [...this.renderRoot.querySelectorAll<HTMLElement>('#comp-list [role="option"]')];
  }

  private choose(path: string): void {
    this.chosenPath = path;
    void this.updateComplete.then(() => {
      this.options().find((option) => option.dataset['path'] === path)?.focus();
    });
  }

  private open(path: string): void {
    this.dispatchEvent(new CustomEvent<ComponentOpenDetail>(COMPONENT_OPEN, { detail: { path }, bubbles: true, composed: true }));
  }

  private readonly onListClick = (event: MouseEvent): void => {
    const path = pathOf(event.target);
    if (path === undefined) return;
    this.choose(path);
    this.open(path);
  };

  private readonly onListKeydown = (event: KeyboardEvent): void => {
    const current = pathOf(event.target);
    const options = this.options();
    const index = options.findIndex((option) => option.dataset['path'] === current);
    if (current === undefined || index < 0) return;
    if (event.key === 'Enter') {
      event.preventDefault();
      this.open(current);
      return;
    }
    const next = neighbour(options, index, event.key)?.dataset['path'];
    if (next === undefined) return;
    event.preventDefault();
    this.choose(next);
  };

  private readonly onSearchInput = (event: Event): void => {
    if (event.target instanceof HTMLInputElement) this.search = event.target.value;
  };

  private readonly onSearchKeydown = (event: KeyboardEvent): void => {
    if (event.key === 'Escape' && this.search !== '') {
      event.stopPropagation();
      this.search = '';
    } else if (event.key === 'ArrowDown') {
      const first = this.options()[0]?.dataset['path'];
      if (first === undefined) return;
      event.preventDefault();
      this.choose(first);
    }
  };

  private readonly clearSearch = (): void => {
    this.search = '';
    this.searchInput.focus();
  };
}
