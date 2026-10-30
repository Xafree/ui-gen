export class XSlotted extends HTMLElement {
  constructor() {
    super();
    this.attachShadow({ mode: 'open' }).innerHTML = '<span><slot></slot></span>';
  }
}

customElements.define('x-slotted', XSlotted);
