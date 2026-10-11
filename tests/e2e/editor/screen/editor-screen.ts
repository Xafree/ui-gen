import { expect, type Locator, type Page } from '@playwright/test';
import { writeFiles, writeRecents } from '../../project/studio-app';

const TSCONFIG = `{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "experimentalDecorators": true,
    "useDefineForClassFields": false,
    "strict": true
  },
  "include": ["src"]
}
`;

const FORMAT_PRICE = `export function formatPrice(amount: number, locale = 'fr-FR'): string {
  return new Intl.NumberFormat(locale, { style: 'currency', currency: 'EUR', maximumFractionDigits: 0 }).format(amount);
}
`;

const FORMAT_PRICE_REEXPORT = `export { formatPrice } from '../utils/format-price.js';
`;

const BADGE = `import { LitElement, css, html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

@customElement('hz-badge')
export class HzBadge extends LitElement {
  static styles = css\`
    :host {
      display: inline-flex;
      padding: 0 var(--hz-space-2);
      border-radius: var(--hz-radius-full);
      background: var(--hz-color-accent-soft);
      color: var(--hz-color-accent);
      font-size: var(--hz-text-xs);
    }
  \`;

  @property({ reflect: true }) tone: 'neutral' | 'accent' = 'neutral';

  render() {
    return html\`<slot></slot>\`;
  }
}
`;

const BUTTON = `import { LitElement, css, html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

@customElement('hz-button')
export class HzButton extends LitElement {
  static styles = css\`
    button {
      display: inline-flex;
      flex-direction: var(--hz-button-direction, row);
      gap: var(--hz-button-gap, var(--hz-space-2));
      padding: var(--hz-button-padding, 0 var(--hz-space-4));
      border: var(--hz-button-border, 1px solid var(--hz-color-border));
      border-radius: var(--hz-button-radius, var(--hz-radius-md));
      background: var(--hz-button-bg, var(--hz-color-bg));
      box-shadow: var(--hz-button-shadow, none);
      font-size: var(--hz-button-font-size, var(--hz-text-md));
      font-weight: var(--hz-button-font-weight, var(--hz-weight-medium));
      width: 100%;
    }
  \`;

  @property({ reflect: true }) variant: 'primary' | 'secondary' = 'primary';
  @property({ type: Boolean, reflect: true }) disabled = false;

  render() {
    return html\`<button type="button" ?disabled=\${this.disabled}><slot></slot><slot name="end"></slot></button>\`;
  }
}
`;

const TOOLTIP = `import { LitElement, css, html } from 'lit';
import { customElement } from 'lit/decorators.js';

@customElement('hz-tooltip')
export class HzTooltip extends LitElement {
  static styles = css\`
    :host {
      display: block;
      padding: var(--hz-space-1) var(--hz-space-2);
      border-radius: var(--hz-radius-sm);
      background: var(--hz-color-text);
      color: var(--hz-color-bg);
      font-size: var(--hz-text-xs);
    }
  \`;

  render() {
    return html\`<span role="tooltip"><slot></slot></span>\`;
  }
}
`;

function reexport(primitive: string): string {
  return `export * from '../primitives/${primitive}.js';\n`;
}

export async function writeTsconfig(project: string): Promise<void> {
  await writeFiles(project, { 'tsconfig.json': TSCONFIG });
}

export async function writeKitHorizonSupport(project: string): Promise<void> {
  await writeFiles(project, {
    'src/utils/format-price.ts': FORMAT_PRICE,
    'src/components/format-price.ts': FORMAT_PRICE_REEXPORT,
    'src/primitives/hz-badge.ts': BADGE,
    'src/primitives/hz-button.ts': BUTTON,
    'src/primitives/hz-tooltip.ts': TOOLTIP,
    'src/components/hz-badge.ts': reexport('hz-badge'),
    'src/components/hz-button.ts': reexport('hz-button'),
    'src/components/hz-tooltip.ts': reexport('hz-tooltip'),
  });
}

export async function rememberRecentProject(userData: string, project: string): Promise<void> {
  await writeRecents(userData, [{ path: project, name: 'Kit Horizon', openedAt: new Date().toISOString() }]);
}

export function frameName(window: Page): Locator {
  return window.locator('#frame-name');
}

export function componentsTab(window: Page): Locator {
  return window.getByRole('tab', { name: /^Composants/ });
}

export function componentOption(window: Page, name: string): Locator {
  return window.locator('#comp-list [role="option"]').filter({ has: window.locator('.ed-comp__name', { hasText: name }) });
}

export function openedMark(window: Page): Locator {
  return window.locator('#comp-list [role="option"]').filter({ has: window.locator('.ed-comp__mark', { hasText: 'Ouvert' }) });
}

export async function expectEditorOn(window: Page, tag: string): Promise<void> {
  await expect(window.locator('ui-editor')).toBeVisible();
  await expect(frameName(window)).toHaveText(tag);
  await expect(window.locator('#page-title')).toHaveText(`Éditeur du composant ${tag}`);
}
