import { html, LitElement, nothing, type TemplateResult } from 'lit';
import { customElement, property, query, state } from 'lit/decorators.js';
import { repeat } from 'lit/directives/repeat.js';
import type { OpenedProject, OpenProjectOutcome } from '../../modules/project/application/open/project-outcomes.js';
import type { RecentProjectView } from '../../modules/project/application/recents/recent-projects-use-cases.js';
import type { UiGenApi } from '../../preload/ui-gen-api.js';
import { icon, studioMark } from '../icons/icon.js';
import { enter, swap } from '../motion/motion.js';
import { showToast, type ToastRequest } from '../motion/toast.js';
import { hostContents, studioStyles } from '../styles/studio-styles.js';
import {
  PROJECT_OPENED,
  type NoticeRequestedDetail,
  type ProjectAdoptedDetail,
  type ProjectCreatedDetail,
  type ProjectOpenedDetail,
} from '../studio-events.js';
import { REQUEST_REFUSED } from '../studio-notices.js';
import { ThemeController } from '../theme/theme-controller.js';
import { themeToggleView, toggleTheme } from '../theme/theme.js';
import './dialogs/ui-adopt-dialog.js';
import type { UiAdoptDialog } from './dialogs/ui-adopt-dialog.js';
import './dialogs/ui-create-dialog.js';
import type { UiCreateDialog } from './dialogs/ui-create-dialog.js';
import { homeStyles } from './home-styles.js';
import { openFailureToast } from './open-messages.js';
import { formatOpenedAt, formatRelativeTime } from './recent-time.js';

declare global {
  interface HTMLElementTagNameMap {
    'ui-home': UiHome;
  }
}

const RECENTS_UNREADABLE: ToastRequest = {
  tone: 'warning',
  title: 'Projets récents illisibles',
  message: "La liste n'a pas pu être lue. Aucun dossier n'a été modifié.",
};

type OpenTrigger = 'btn-open' | 'empty-open';

const READING_FOLDER = 'Lecture du dossier…';
const OPEN_PROJECT = 'Ouvrir un projet';

function triggerOf(event: Event): HTMLElement | null {
  return event.currentTarget instanceof HTMLElement ? event.currentTarget : null;
}

@customElement('ui-home')
export class UiHome extends LitElement {
  static override styles = [studioStyles, homeStyles, hostContents];

  @property({ attribute: false }) api!: UiGenApi;
  @property({ attribute: false }) notice: ToastRequest | null = null;

  @state() private recents: readonly RecentProjectView[] = [];
  @state() private loaded = false;
  @state() private opening: OpenTrigger | null = null;

  @query('#recent-body') private recentBody!: HTMLElement;
  @query('.ui-toast-region') private toastRegion!: HTMLOListElement;
  @query('#main') private main!: HTMLElement;
  @query('ui-create-dialog') private createDialog!: UiCreateDialog;
  @query('ui-adopt-dialog') private adoptDialog!: UiAdoptDialog;

  private readonly theme = new ThemeController(this);
  private openPending = false;

  override render(): TemplateResult {
    const toggle = themeToggleView(this.theme.theme);
    const empty = this.recents.length === 0;
    return html`
      <div class="ui-page">
        <a class="ui-skip" href="#main" @click=${this.skipToMain}>Aller au contenu principal</a>
        <header class="ui-topbar ac-topbar">
          <span class="ui-topbar__brand">
            <span class="ui-topbar__mark">${studioMark()}</span>
            <span class="ui-topbar__project">UI-GEN</span>
            <span class="ui-topbar__meta">Aucun projet ouvert</span>
          </span>
          <div class="ui-topbar__actions">
            <button type="button" class="ui-icon-btn" data-ui-theme-toggle data-tip-pos="end" aria-label=${toggle.label} data-tip=${toggle.tip} @click=${toggleTheme}>${icon(toggle.icon)}</button>
            <span class="ui-avatar" role="img" aria-label="Compte local" data-tip="Compte local" data-tip-pos="end">${icon('utilisateur')}</span>
          </div>
        </header>

        <main id="main" class="ui-page__main ac-main" tabindex="-1">
          <div class="ac-layout">
            <section class="ac-start" aria-labelledby="ac-title">
              <span class="ac-start__mark" aria-hidden="true" data-enter>${studioMark()}</span>
              <div class="ac-start__head" data-enter>
                <h1 id="ac-title" class="ui-title-2">UI-GEN</h1>
                <p class="ac-start__lead">Créez une bibliothèque de composants Lit, ou ouvrez une bibliothèque existante pour la dessiner, la tester et l'exporter.</p>
              </div>
              <div class="ac-actions" data-enter>
                <button class="ui-btn ui-btn--primary ui-btn--lg" type="button" id="btn-create" @click=${this.onCreateClick}>${icon('plus')}Créer un projet</button>
                <button class="ui-btn ui-btn--secondary ui-btn--lg" type="button" id="btn-open" aria-busy=${this.opening === 'btn-open' ? 'true' : nothing} @click=${this.onOpenClick}>${icon('dossier')}${this.opening === 'btn-open' ? READING_FOLDER : OPEN_PROJECT}</button>
              </div>
              <p class="ac-note" data-enter>${icon('info')}<span>Un projet est le dossier d'une bibliothèque Lit. UI-GEN y range ses réglages dans un fichier <code>uigen.json</code>.</span></p>
            </section>

            <section class="ui-card ac-recent" aria-labelledby="recent-title" data-enter>
              <div class="ac-recent__head">
                <h2 class="ac-recent__title" id="recent-title">Projets récents</h2>
                <span class="ui-count" id="recent-count" ?hidden=${empty}>${String(this.recents.length)}</span>
                <span class="ui-spacer"></span>
                <button class="ui-btn ui-btn--ghost ui-btn--sm" type="button" id="recent-clear" ?hidden=${empty}>${icon('corbeille', 'icon--sm')}Effacer la liste</button>
              </div>
              <div class="ac-recent__body" id="recent-body">
                <ul class="ui-list ac-recent__list" id="recent-list" aria-labelledby="recent-title" ?hidden=${empty}>
                  ${repeat(this.recents, (project) => project.path, (project) => this.renderRecent(project))}
                </ul>
                <div class="ui-empty" id="recent-empty" ?hidden=${!this.loaded || !empty}>
                  <span class="ui-empty__icon">${icon('historique')}</span>
                  <p class="ui-empty__title">Aucun projet récent</p>
                  <p class="ui-empty__text">Les projets que vous créez ou ouvrez s'affichent ici, du plus récent au plus ancien.</p>
                  <div class="ui-empty__actions">
                    <button class="ui-btn ui-btn--secondary ui-btn--sm" type="button" id="empty-create" @click=${this.onCreateClick}>Créer un projet</button>
                    <button class="ui-btn ui-btn--ghost ui-btn--sm" type="button" id="empty-open" aria-busy=${this.opening === 'empty-open' ? 'true' : nothing} @click=${this.onOpenClick}>${this.opening === 'empty-open' ? READING_FOLDER : OPEN_PROJECT}</button>
                  </div>
                </div>
              </div>
              <p class="ac-recent__foot">Les 10 derniers projets ouverts. Effacer la liste ne modifie aucun dossier.</p>
            </section>
          </div>
        </main>
        <ui-create-dialog .api=${this.api} @project-created=${this.onProjectCreated} @notice-requested=${this.onNoticeRequested}></ui-create-dialog>
        <ui-adopt-dialog .api=${this.api} @project-adopted=${this.onProjectAdopted}></ui-adopt-dialog>
        <ol class="ui-toast-region" aria-live="polite" aria-label="Notifications"></ol>
      </div>
    `;
  }

  protected override firstUpdated(): void {
    document.title = 'Accueil, UI-GEN';
    enter([...this.renderRoot.querySelectorAll('[data-enter]')], { stagger: 0.03 });
    if (this.notice !== null) this.toast(this.notice);
    void this.loadRecents();
  }

  private async loadRecents(): Promise<void> {
    try {
      const listing = await this.api.recents.list();
      this.recents = listing.projects;
      if (listing.unreadable) this.toast(RECENTS_UNREADABLE);
    } catch (error) {
      console.warn('Recent projects unavailable', error);
      this.toast(RECENTS_UNREADABLE);
    }
    this.loaded = true;
  }

  private renderRecent(project: RecentProjectView): TemplateResult {
    const body = html`<span class="ui-list-item__body"><span class="ui-list-item__title">${project.name}</span><span class="ui-list-item__meta">${project.path}</span></span>`;
    if (!project.available) {
      return html`<li class="ui-list-item ac-row is-missing"><span class="ac-row__icon">${icon('alerte')}</span>${body}<span class="ac-row__state"><span class="ui-badge ui-badge--warning" title="Dossier déplacé ou supprimé">Introuvable</span><button class="ui-btn ui-btn--ghost ui-btn--sm" type="button" @click=${(event: MouseEvent) => void this.removeRecent(event, project)}>Retirer de la liste<span class="sr-only"> : ${project.name}</span></button></span></li>`;
    }
    const openedAt = new Date(project.openedAt);
    return html`<li><a class="ui-list-item ac-row" href="#main" @click=${(event: MouseEvent) => void this.openRecent(event, project)}><span class="ac-row__icon">${icon('dossier')}</span>${body}<time class="ac-row__when" datetime=${openedAt.toISOString()} title=${formatOpenedAt(openedAt)}>${formatRelativeTime(openedAt, new Date())}</time>${icon('fleche-droite', 'ac-row__go')}</a></li>`;
  }

  private async openRecent(event: MouseEvent, project: RecentProjectView): Promise<void> {
    event.preventDefault();
    if (this.openPending) return;
    const link = triggerOf(event);
    this.openPending = true;
    link?.setAttribute('aria-busy', 'true');
    try {
      await this.handleOpenOutcome(await this.api.recents.open(project.path), link);
    } catch (error) {
      this.refuse(error, link);
    } finally {
      link?.removeAttribute('aria-busy');
      this.openPending = false;
    }
  }

  private async createProject(trigger: Element | null): Promise<void> {
    if (this.openPending) return;
    this.openPending = true;
    try {
      await this.createDialog.open(trigger);
    } finally {
      this.openPending = false;
    }
  }

  private async openFolder(event: MouseEvent): Promise<void> {
    const button = triggerOf(event);
    if (this.openPending || button === null) return;
    this.openPending = true;
    this.opening = button.id === 'empty-open' ? 'empty-open' : 'btn-open';
    try {
      const outcome = await this.api.project.openFolder();
      this.opening = null;
      if (outcome.kind !== 'canceled') await this.handleOpenOutcome(outcome, button);
    } catch (error) {
      this.refuse(error, button);
    } finally {
      this.opening = null;
      this.openPending = false;
    }
  }

  private async handleOpenOutcome(outcome: OpenProjectOutcome, trigger: HTMLElement | null): Promise<void> {
    switch (outcome.kind) {
      case 'opened':
        this.projectOpened(outcome.project, null);
        return;
      case 'needs-adoption':
        this.adoptDialog.open(outcome.proposal, trigger).catch((error: unknown) => {
          this.refuse(error, trigger);
        });
        return;
      case 'missing':
        await this.reloadRecents();
        this.toast(openFailureToast(outcome));
        return;
      default:
        this.toast(openFailureToast(outcome));
        trigger?.focus();
    }
  }

  private async removeRecent(event: MouseEvent, project: RecentProjectView): Promise<void> {
    const button = triggerOf(event);
    let remaining: RecentProjectView[];
    try {
      remaining = await this.api.recents.remove(project.path);
    } catch (error) {
      this.refuse(error, button);
      return;
    }
    await swap(this.recentBody, () => {
      this.recents = remaining;
    });
    await this.updateComplete;
    const next =
      this.renderRoot.querySelector<HTMLElement>('#recent-list a, #recent-list button') ??
      this.renderRoot.querySelector<HTMLElement>('#btn-open');
    next?.focus();
    this.toast({
      tone: 'success',
      title: `${project.name} retiré de la liste`,
      message: `Le dossier ${project.path} n'a pas été modifié.`,
    });
  }

  private async reloadRecents(): Promise<void> {
    const { projects } = await this.api.recents.list();
    await swap(this.recentBody, () => {
      this.recents = projects;
    });
  }

  private projectOpened(project: OpenedProject, notice: ToastRequest | null): void {
    this.dispatchEvent(new CustomEvent<ProjectOpenedDetail>(PROJECT_OPENED, { detail: { project, notice } }));
  }

  private refuse(error: unknown, trigger: HTMLElement | null): void {
    console.warn('Request refused', error);
    this.toast(REQUEST_REFUSED);
    trigger?.focus();
  }

  private toast(request: ToastRequest): void {
    showToast(this.toastRegion, request);
  }

  private readonly skipToMain = (event: MouseEvent): void => {
    event.preventDefault();
    this.main.focus();
  };

  private readonly onCreateClick = (event: MouseEvent): void => {
    void this.createProject(event.currentTarget instanceof Element ? event.currentTarget : null);
  };

  private readonly onOpenClick = (event: MouseEvent): void => {
    void this.openFolder(event);
  };

  private readonly onProjectCreated = (event: CustomEvent<ProjectCreatedDetail>): void => {
    const { project } = event.detail;
    this.projectOpened(project, { tone: 'success', title: 'Projet créé', message: `${project.name} est prêt dans ${project.path}.` });
  };

  private readonly onProjectAdopted = (event: CustomEvent<ProjectAdoptedDetail>): void => {
    const { project } = event.detail;
    this.projectOpened(project, {
      tone: 'success',
      title: `${project.name} ouvert avec UI-GEN`,
      message: `uigen.json ajouté à ${project.path} avec le préfixe ${project.settings.tagPrefix}.`,
    });
  };

  private readonly onNoticeRequested = (event: CustomEvent<NoticeRequestedDetail>): void => {
    this.toast(event.detail.notice);
  };
}
