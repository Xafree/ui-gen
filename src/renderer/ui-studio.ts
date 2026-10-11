import { LitElement, html } from 'lit';
import { customElement } from 'lit/decorators.js';

declare global {
  interface HTMLElementTagNameMap {
    'ui-studio': UiStudio;
  }
}

@customElement('ui-studio')
export class UiStudio extends LitElement {
  override render() {
    return html``;
  }
}
