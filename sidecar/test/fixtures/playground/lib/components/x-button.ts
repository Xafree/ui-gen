import { LitElement, css, html } from 'lit';
import { customElement, state } from 'lit/decorators.js';

const loads = Number(window.name || '0') + 1;
window.name = String(loads);

@customElement('x-button')
export class XButton extends LitElement {
  static styles = css`
    :host {
      display: block;
      position: relative;
      width: 120px;
      height: 56px;
    }
    button {
      display: block;
      width: 120px;
      height: 40px;
      border: 0;
      border-radius: var(--x-radius);
      background: var(--x-color-primary);
      color: white;
      font: 16px sans-serif;
    }
    button:hover {
      box-shadow: inset 0 0 0 4px rgb(255, 255, 0);
    }
    i {
      position: absolute;
      top: 44px;
      width: 8px;
      height: 8px;
    }
    .clicks {
      left: 0;
    }
    .loads {
      left: 12px;
    }
  `;

  @state() private clicks = 0;

  private readonly onClick = () => {
    this.clicks += 1;
  };

  connectedCallback() {
    super.connectedCallback();
    this.addEventListener('click', this.onClick);
  }

  disconnectedCallback() {
    this.removeEventListener('click', this.onClick);
    super.disconnectedCallback();
  }

  render() {
    return html`<button part="control"><slot></slot></button>
      <i class="clicks" style="background: rgb(0, ${this.clicks * 50}, 0)"></i>
      <i class="loads" style="background: rgb(${loads * 50}, 0, 0)"></i>`;
  }
}
