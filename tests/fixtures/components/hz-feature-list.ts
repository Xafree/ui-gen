import { LitElement, html } from 'lit';
import { customElement, property } from 'lit/decorators.js';
import { repeat } from 'lit/directives/repeat.js';
import { formatPrice } from './format-price.js';

@customElement('hz-feature-list')
export class HzFeatureList extends LitElement {
  @property({ type: Array }) features: string[] = [];
  @property({ type: Number }) price = 0;
  @property({ type: Boolean }) compact = false;
  @property() heading = 'Inclus';

  /* Pied de liste, rendu à part. */
  private renderFooter() {
    return html`<footer>Sans engagement</footer>`;
  }

  render() {
    return html`
      <h4>${this.heading}</h4>
      <ul>
        ${this.features.map((feature) => html`<li>${feature}</li>`)}
      </ul>
      <ol>
        ${repeat(
          this.features,
          (feature) => feature,
          (feature, index) => html`<li>${index + 1}. ${feature}</li>`,
        )}
      </ol>
      <p>${formatPrice(this.price)}</p>
      <p>${this.compact ? 'Compact' : 'Complet'}</p>
      <p>${this.ariaLabel}</p>
      ${this.renderFooter()}
    `;
  }
}
