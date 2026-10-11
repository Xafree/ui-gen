import { html, LitElement, type TemplateResult } from 'lit';
import { customElement, property, query, state } from 'lit/decorators.js';
import type { OpenedProject } from '../modules/project/application/open/project-outcomes.js';
import type { UiGenApi } from '../preload/ui-gen-api.js';
import './home/ui-home.js';
import { showToast, type ToastRequest } from './motion/toast.js';
import { hostContents, studioStyles } from './styles/studio-styles.js';
import type { ProjectClosedDetail, ProjectOpenedDetail } from './studio-events.js';
import { REQUEST_REFUSED } from './studio-notices.js';

declare global {
  interface HTMLElementTagNameMap {
    'ui-studio': UiStudio;
  }
}

type StudioView =
  | { readonly kind: 'home'; readonly notice: ToastRequest | null }
  | { readonly kind: 'project'; readonly project: OpenedProject; readonly notice: ToastRequest | null };

@customElement('ui-studio')
export class UiStudio extends LitElement {
  static override styles = [studioStyles, hostContents];

  @property({ attribute: false }) api: UiGenApi = window.uiGen;

  @state() private view: StudioView = { kind: 'home', notice: null };

  @query('.ui-toast-region') private toastRegion!: HTMLOListElement;

  override render(): TemplateResult {
    return html`${this.renderView()}<ol class="ui-toast-region" aria-live="polite" aria-label="Notifications"></ol>`;
  }

  private renderView(): TemplateResult {
    if (this.view.kind === 'home') {
      return html`<ui-home .api=${this.api} .notice=${this.view.notice} @project-opened=${this.onProjectOpened}></ui-home>`;
    }
    return html`<ui-shell .api=${this.api} .project=${this.view.project} .notice=${this.view.notice} @project-closed=${this.onProjectClosed}></ui-shell>`;
  }

  private async closeProject(notice: ToastRequest | null): Promise<void> {
    try {
      await this.api.project.close();
    } catch (error) {
      console.warn('Project close refused', error);
      showToast(this.toastRegion, REQUEST_REFUSED);
      return;
    }
    this.toastRegion.replaceChildren();
    this.view = { kind: 'home', notice };
  }

  private readonly onProjectOpened = (event: CustomEvent<ProjectOpenedDetail>): void => {
    this.view = { kind: 'project', project: event.detail.project, notice: event.detail.notice };
  };

  private readonly onProjectClosed = (event: CustomEvent<ProjectClosedDetail>): void => {
    const { project, reason } = event.detail;
    const notice: ToastRequest | null =
      reason === 'close'
        ? { tone: 'info', title: `${project.name} fermé`, message: 'Il reste dans les projets récents.' }
        : null;
    void this.closeProject(notice);
  };
}
