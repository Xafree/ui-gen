import { html, LitElement, nothing, unsafeCSS, type TemplateResult } from 'lit';
import { customElement, property, query, state } from 'lit/decorators.js';
import type { OpenedProject } from '../../modules/project/application/open/project-outcomes.js';
import type { UiGenApi } from '../../preload/ui-gen-api.js';
import { icon, studioMark } from '../icons/icon.js';
import { closePanel, enter, openPanel, swap } from '../motion/motion.js';
import type { ToastRequest } from '../motion/toast.js';
import { hostContents, studioStyles } from '../styles/studio-styles.js';
import {
  NOTICE_REQUESTED,
  PROJECT_CLOSED,
  type NoticeRequestedDetail,
  type ProjectClosedDetail,
} from '../studio-events.js';
import { ThemeController } from '../theme/theme-controller.js';
import { themeToggleView, toggleTheme } from '../theme/theme.js';
import { SCREENS, screenById, type ScreenId, type StudioScreen } from './screens.js';
import shellCss from './shell.css?inline';

declare global {
  interface HTMLElementTagNameMap {
    'ui-shell': UiShell;
  }
}

type MenuId = 'ui-project-menu' | 'ui-screens-menu';

const CHAT_PENDING: ToastRequest = {
  tone: 'info',
  title: 'Disponible dans un prochain lot',
  message: 'Le Chat IA arrive avec le lot 6.',
};

function windowTitle(id: ScreenId): string {
  return `${screenById(id).label}, UI-GEN`;
}

function isChatShortcut(event: KeyboardEvent): boolean {
  return (event.ctrlKey || event.metaKey) && !event.altKey && !event.shiftKey && event.key.toLowerCase() === 'i';
}

function isSpace(event: KeyboardEvent): boolean {
  return event.key === ' ';
}

@customElement('ui-shell')
export class UiShell extends LitElement {
  static override styles = [studioStyles, unsafeCSS(shellCss), hostContents];

  @property({ attribute: false }) api!: UiGenApi;
  @property({ attribute: false }) project!: OpenedProject;
  @property({ attribute: false }) notice: ToastRequest | null = null;

  @state() private active: ScreenId = 'editor';
  @state() private shown: ScreenId = 'editor';
  @state() private openMenu: MenuId | null = null;

  @query('#main') private main!: HTMLElement;
  @query('#av-empty') private emptyState!: HTMLElement;
  @query('.ui-topbar__project-menu') private projectMenu!: HTMLElement;

  private readonly theme = new ThemeController(this);

  override connectedCallback(): void {
    super.connectedCallback();
    document.addEventListener('click', this.onDocumentClick);
    document.addEventListener('keydown', this.onDocumentKeydown);
  }

  override disconnectedCallback(): void {
    document.removeEventListener('click', this.onDocumentClick);
    document.removeEventListener('keydown', this.onDocumentKeydown);
    super.disconnectedCallback();
  }

  override render(): TemplateResult {
    const shown = screenById(this.shown);
    return html`
      <div class="ui-app">
        <a class="ui-skip" href="#main" @click=${this.skipToMain}>Aller au contenu principal</a>
        <header class="ui-topbar" data-shell data-active=${this.active}>
          ${this.renderBrandGroup()}${this.renderScreensMenu()}${this.renderNav()}${this.renderActions()}
        </header>
        <main class="ui-stage av-stage" id="main" tabindex="-1">
          <h1 class="sr-only" id="av-heading">${shown.label}</h1>
          <div class="ui-empty" id="av-empty">
            <span class="ui-empty__icon">${icon(shown.icon)}</span>
            <p class="ui-empty__title">Disponible dans un prochain lot</p>
            <p class="ui-empty__text" id="av-text">L'écran ${shown.label} arrive avec le lot ${String(shown.lot)}.</p>
          </div>
        </main>
      </div>
    `;
  }

  protected override firstUpdated(): void {
    document.title = windowTitle(this.shown);
    enter([...this.emptyState.children], { stagger: 0.03 });
    if (this.notice !== null) this.requestNotice(this.notice);
  }

  private renderBrandGroup(): TemplateResult {
    return html`
      <div class="ui-topbar__brand-group">
        <span class="ui-topbar__brand"><span class="ui-topbar__mark">${studioMark()}</span></span>
        <div class="ui-topbar__project-menu" @focusout=${this.onProjectMenuFocusOut}>
          <button type="button" class="ui-topbar__project-btn" data-ui-popover="ui-project-menu" aria-haspopup="true" aria-controls="ui-project-menu" aria-expanded=${String(this.openMenu === 'ui-project-menu')} @click=${this.toggleProjectMenu}><span class="sr-only">Projet :</span><span class="ui-topbar__project">${this.project.name}</span><span class="ui-topbar__meta">${`${this.project.packageName} ${this.project.version}`}</span>${icon('chevron-bas', 'icon--sm ui-topbar__project-chevron')}</button>
          <div class="ui-popover" id="ui-project-menu" hidden>
            <p class="ui-menu-label ui-topbar__project-path">${this.project.path}</p>
            <ul class="ui-menu">
              <li><button class="ui-menu-item" type="button" @click=${this.switchProject}>${icon('dossier')}Changer de projet</button></li>
              <li class="ui-menu-sep" role="separator"></li>
              <li><button class="ui-menu-item" type="button" @click=${this.closeCurrentProject}>${icon('fermer')}Fermer le projet</button></li>
            </ul>
          </div>
        </div>
      </div>
    `;
  }

  private renderScreensMenu(): TemplateResult {
    const screen = screenById(this.active);
    return html`
      <div class="ui-topbar__screens">
        <button type="button" class="ui-btn ui-btn--ghost" data-ui-popover="ui-screens-menu" aria-controls="ui-screens-menu" aria-expanded=${String(this.openMenu === 'ui-screens-menu')} @click=${this.toggleScreensMenu}><span class="sr-only">Écran actuel : </span>${screen.short}${icon('chevron-bas', 'icon--sm')}</button>
        <div class="ui-popover" id="ui-screens-menu" hidden>
          <p class="ui-menu-label">Écrans du studio</p>
          <ul class="ui-menu">${SCREENS.map((item) => this.renderMenuItem(item))}</ul>
        </div>
      </div>
    `;
  }

  private renderMenuItem(item: StudioScreen): TemplateResult {
    return html`<li><a class="ui-menu-item" href="#main" aria-current=${item.id === this.active ? 'page' : nothing} @click=${(event: MouseEvent) => { this.activateFromMenu(item.id, event); }} @keydown=${(event: KeyboardEvent) => { if (isSpace(event)) this.activateFromMenu(item.id, event); }}>${icon(item.icon)}${item.label}</a></li>`;
  }

  private renderNav(): TemplateResult {
    return html`<nav class="ui-topbar__nav" aria-label="Écrans du studio"><ul class="ui-nav">${SCREENS.map((item) => this.renderNavItem(item))}</ul></nav>`;
  }

  private renderNavItem(item: StudioScreen): TemplateResult {
    return html`<li><a href="#main" aria-current=${item.id === this.active ? 'page' : nothing} @click=${(event: MouseEvent) => { this.activate(item.id, event); }} @keydown=${(event: KeyboardEvent) => { if (isSpace(event)) this.activate(item.id, event); }}>${icon(item.icon)}<span class="ui-nav__long">${item.label}</span><span class="ui-nav__short">${item.short}</span></a></li>`;
  }

  private renderActions(): TemplateResult {
    const toggle = themeToggleView(this.theme.theme);
    return html`
      <div class="ui-topbar__actions">
        <div class="ui-topbar__slot"><button class="ui-btn ui-btn--ghost ui-btn--sm ed-topbar-btn" type="button" id="chat-toggle" @click=${this.showChatPending}>${icon('etincelle')}<span class="ed-hide-sm">Chat IA</span> <span class="ui-btn__kbd ed-hide-md">Ctrl&nbsp;I</span></button><a class="ui-btn ui-btn--primary ui-btn--sm ed-topbar-btn ed-hide-xs" href="#main" id="export-link" @click=${this.exportScreen} @keydown=${this.exportScreenOnSpace}>${icon('package')}<span class="ed-hide-sm">Exporter</span></a></div>
        <span class="ui-topbar__divider" aria-hidden="true"></span>
        <button type="button" class="ui-icon-btn" data-ui-theme-toggle data-tip-pos="end" aria-label=${toggle.label} data-tip=${toggle.tip} @click=${toggleTheme}>${icon(toggle.icon)}</button>
        <span class="ui-avatar" role="img" aria-label="Compte local" data-tip="Compte local" data-tip-pos="end">${icon('utilisateur')}</span>
      </div>
    `;
  }

  private activate(id: ScreenId, event: Event): void {
    event.preventDefault();
    this.showScreen(id);
  }

  private activateFromMenu(id: ScreenId, event: Event): void {
    event.preventDefault();
    this.closeMenu(true);
    this.showScreen(id);
  }

  private showScreen(id: ScreenId): void {
    this.active = id;
    void swap(this.emptyState, () => {
      this.shown = id;
      document.title = windowTitle(id);
    });
  }

  private element(selector: string): HTMLElement {
    const found = this.renderRoot.querySelector<HTMLElement>(selector);
    if (found === null) throw new Error(`Missing shell element: ${selector}`);
    return found;
  }

  private toggleMenu(menu: MenuId, event: MouseEvent): void {
    if (this.openMenu === menu) {
      this.closeMenu(false);
      return;
    }
    this.closeMenu(false);
    this.openMenu = menu;
    const popover = this.element(`#${menu}`);
    popover.inert = false;
    void openPanel(popover);
    if (event.detail === 0) popover.querySelector<HTMLElement>('.ui-menu-item')?.focus();
  }

  private closeMenu(returnFocus: boolean): void {
    const menu = this.openMenu;
    if (menu === null) return;
    this.openMenu = null;
    const popover = this.element(`#${menu}`);
    popover.inert = true;
    void closePanel(popover);
    if (returnFocus) this.element(`[data-ui-popover="${menu}"]`).focus();
  }

  private leaveProject(reason: ProjectClosedDetail['reason']): void {
    this.closeMenu(true);
    this.dispatchEvent(
      new CustomEvent<ProjectClosedDetail>(PROJECT_CLOSED, { detail: { project: this.project, reason } }),
    );
  }

  private requestNotice(notice: ToastRequest): void {
    this.dispatchEvent(new CustomEvent<NoticeRequestedDetail>(NOTICE_REQUESTED, { detail: { notice } }));
  }

  private readonly toggleProjectMenu = (event: MouseEvent): void => {
    this.toggleMenu('ui-project-menu', event);
  };

  private readonly toggleScreensMenu = (event: MouseEvent): void => {
    this.toggleMenu('ui-screens-menu', event);
  };

  private readonly switchProject = (): void => {
    this.leaveProject('switch');
  };

  private readonly closeCurrentProject = (): void => {
    this.leaveProject('close');
  };

  private readonly exportScreen = (event: MouseEvent): void => {
    this.activate('export', event);
  };

  private readonly exportScreenOnSpace = (event: KeyboardEvent): void => {
    if (isSpace(event)) this.activate('export', event);
  };

  private readonly onDocumentClick = (event: MouseEvent): void => {
    const menu = this.openMenu;
    if (menu === null) return;
    const path = event.composedPath();
    if (!path.includes(this.element(`#${menu}`)) && !path.includes(this.element(`[data-ui-popover="${menu}"]`))) {
      this.closeMenu(false);
    }
  };

  private readonly onDocumentKeydown = (event: KeyboardEvent): void => {
    if (isChatShortcut(event)) {
      event.preventDefault();
      this.showChatPending();
      return;
    }
    const menu = this.openMenu;
    if (menu === null) return;
    if (event.key === 'Escape') {
      this.closeMenu(true);
    } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      this.moveInMenu(menu, event);
    }
  };

  private moveInMenu(menu: MenuId, event: KeyboardEvent): void {
    const popover = this.element(`#${menu}`);
    const focused = this.shadowRoot?.activeElement ?? null;
    if (focused === null || !popover.contains(focused)) return;
    const items = [...popover.querySelectorAll<HTMLElement>('.ui-menu-item')];
    if (items.length === 0) return;
    event.preventDefault();
    const index = items.findIndex((item) => item === focused);
    const step = event.key === 'ArrowDown' ? 1 : -1;
    items[(index + step + items.length) % items.length]?.focus();
  }

  private readonly onProjectMenuFocusOut = (event: FocusEvent): void => {
    const next = event.relatedTarget;
    if (this.openMenu !== 'ui-project-menu' || !(next instanceof Node) || this.projectMenu.contains(next)) return;
    this.closeMenu(false);
  };

  private readonly showChatPending = (): void => {
    this.requestNotice(CHAT_PENDING);
  };

  private readonly skipToMain = (event: MouseEvent): void => {
    event.preventDefault();
    this.main.focus();
  };
}
