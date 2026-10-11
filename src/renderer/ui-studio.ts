import { html, LitElement, type TemplateResult } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import type { OpenedProject } from '../modules/project/application/open/project-outcomes.js';
import type { UiGenApi } from '../preload/ui-gen-api.js';
import './home/ui-home.js';
import type { ToastRequest } from './motion/toast.js';
import { hostContents } from './styles/studio-styles.js';
import type { ProjectClosedDetail, ProjectOpenedDetail } from './studio-events.js';

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
  static override styles = [hostContents];

  @property({ attribute: false }) api: UiGenApi = window.uiGen;

  @state() private view: StudioView = { kind: 'home', notice: null };

  override render(): TemplateResult {
    if (this.view.kind === 'home') {
      return html`<ui-home .api=${this.api} .notice=${this.view.notice} @project-opened=${this.onProjectOpened}></ui-home>`;
    }
    return html`<ui-shell .api=${this.api} .project=${this.view.project} .notice=${this.view.notice} @project-closed=${this.onProjectClosed}></ui-shell>`;
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
    void this.api.project.close().then(() => {
      this.view = { kind: 'home', notice };
    });
  };
}
