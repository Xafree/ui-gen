import { LitElement, css, html, nothing } from 'lit';
import { customElement, property, state } from 'lit/decorators.js';
import { formatPrice } from '../utils/format-price.js';
import './hz-badge.js';
import './hz-button.js';
import './hz-tooltip.js';

export type Plan = 'starter' | 'pro' | 'equipe';

/** Variante du bouton d'action pour chaque offre. */
const CTA_VARIANT: Record<Plan, 'primary' | 'secondary'> = {
  starter: 'secondary',
  pro: 'primary',
  equipe: 'secondary',
};

/**
 * Carte d'offre tarifaire du Kit Horizon.
 *
 * @fires hz-select - Choix de l'offre. detail : { plan }
 * @slot features - Avantages de l'offre, un <li> par ligne.
 */
@customElement('hz-pricing-card')
export class HzPricingCard extends LitElement {
  static styles = css`
    :host {
      position: relative;
      display: flex;
      flex-direction: column;
      gap: var(--hz-space-4);
      padding: var(--hz-space-6);
      border: 1px solid var(--hz-color-border);
      border-radius: var(--hz-radius-lg);
      background: var(--hz-color-bg);
      color: var(--hz-color-text);
      font-family: var(--hz-font-sans);
    }

    :host([variant='pro']) {
      gap: var(--hz-space-5);
      padding: var(--hz-space-8);
      border-color: var(--hz-color-accent);
      box-shadow: var(--hz-shadow-md);
    }

    header {
      display: grid;
      gap: var(--hz-space-1);
      justify-items: start;
    }

    .plan {
      margin: 0;
      font-size: var(--hz-text-lg);
      font-weight: var(--hz-weight-bold);
    }

    .description {
      margin: 0;
      color: var(--hz-color-muted);
      font-size: var(--hz-text-sm);
    }

    .price {
      margin: var(--hz-space-2) 0 0;
      font-size: var(--hz-text-2xl);
      font-weight: var(--hz-weight-bold);
      font-variant-numeric: tabular-nums;
    }

    .period {
      color: var(--hz-color-muted);
      font-size: var(--hz-text-sm);
    }

    .features {
      display: grid;
      gap: var(--hz-space-2);
      margin: 0;
      padding: 0;
      list-style: none;
    }

    ::slotted(li) {
      display: flex;
      gap: var(--hz-space-2);
      font-size: var(--hz-text-sm);
    }

    .cta {
      margin-top: auto;
      width: 100%;
    }

    :host([variant='pro']) .cta {
      width: 100%;
      --hz-button-direction: row;
      --hz-button-gap: var(--hz-space-2);
      --hz-button-padding: 0 var(--hz-space-4);
      --hz-button-bg: var(--hz-color-accent);
      --hz-button-radius: var(--hz-radius-md);
      --hz-button-border: none;
      --hz-button-shadow: none;
      --hz-button-font-size: var(--hz-text-md);
      --hz-button-font-weight: var(--hz-weight-medium);
    }

    .hint {
      position: absolute;
      bottom: calc(100% + var(--hz-space-2));
      left: 50%;
      translate: -50% 0;
    }
  `;

  @property({ reflect: true }) variant: Plan = 'starter';
  @property() plan = '';
  @property() description = '';
  @property({ type: Number }) price = 0;
  @property() locale = 'fr-FR';
  @property() period = 'par mois';
  @property() badge = '';
  @property() cta = '';
  @property({ type: Boolean }) disabled = false;
  @state() private hint = '';

  private emit(name: string) {
    this.dispatchEvent(
      new CustomEvent(name, {
        detail: { plan: this.variant },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private showHint(text: string) {
    this.hint = text;
  }

  private hideHint() {
    this.hint = '';
  }

  render() {
    return html`
      <header>
        ${this.badge ? html`<hz-badge tone="accent">${this.badge}</hz-badge>` : nothing}
        <h3 class="plan">${this.plan}</h3>
        <p class="description">${this.description}</p>
        <p class="price">
          ${formatPrice(this.price, this.locale)}
          <span class="period">${this.period}</span>
        </p>
      </header>
      <ul class="features">
        <slot name="features"></slot>
      </ul>
      <hz-button
        class="cta"
        variant=${CTA_VARIANT[this.variant]}
        ?disabled=${this.disabled}
        @click=${() => this.emit('hz-select')}
        @pointerenter=${() => this.showHint('Essai gratuit de 14 jours')}
        @pointerleave=${this.hideHint}
      >
        ${this.cta}
        ${this.variant === 'pro' ? html`<svg slot="end" aria-hidden="true"><use href="#arrow-right"></use></svg>` : nothing}
      </hz-button>
      ${this.hint ? html`<hz-tooltip class="hint">${this.hint}</hz-tooltip>` : nothing}
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'hz-pricing-card': HzPricingCard;
  }
}
