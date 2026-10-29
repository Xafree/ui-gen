export class XChip extends HTMLElement {
  connectedCallback() {
    this.textContent = this.getAttribute('label') ?? 'chip';
  }
}

customElements.define('x-chip', XChip);
