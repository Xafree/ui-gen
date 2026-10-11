import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, type Frame, type FrameLocator, type Locator, type Page } from '@playwright/test';
import { writeFiles } from '../../project/studio-app';
import { listedPath, openComponent, openProject, withStudio } from '../editor-project';

export const OFFER_CARD_FILE = 'src/components/hz-offer-card.ts';
export const PROBE_CARD_FILE = 'src/components/hz-probe-card.ts';
export const PROJECT_TOKENS_FILE = 'tokens/tokens.css';
export const OUTSIDE_SECRET_FILE = 'outside-secret.js';

export const LIGHT_BG = 'rgb(250, 240, 230)';
export const DARK_BG = 'rgb(15, 25, 35)';

const PACKAGE_JSON = `{
  "name": "@kit-horizon/canvas",
  "version": "0.2.0",
  "type": "module",
  "dependencies": {
    "lit": "^3.3.0"
  }
}
`;

const SETTINGS = `{
  "formatVersion": 1,
  "tagPrefix": "hz-",
  "componentsDir": "src/components"
}
`;

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

const TOKENS_CSS = `:root {
  --hz-color-bg: rgb(250, 240, 230);
  --hz-color-text: rgb(20, 30, 40);
  --hz-space-4: 16px;
}

[data-hz-theme='dark'] {
  --hz-color-bg: rgb(15, 25, 35);
  --hz-color-text: rgb(235, 225, 215);
}
`;

const OFFER_CARD = `import { LitElement, css, html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

export type Plan = 'starter' | 'pro' | 'equipe';

/** Carte d'offre du kit, rendue par le canvas pour chaque offre. */
@customElement('hz-offer-card')
export class HzOfferCard extends LitElement {
  static styles = css\`
    :host {
      display: block;
    }

    .card {
      padding: var(--hz-space-4);
      background: var(--hz-color-bg);
      color: var(--hz-color-text);
    }
  \`;

  // Offre affichée, reflétée pour le style.
  @property({ reflect: true }) variant: Plan = 'starter';
  @property() plan = 'Horizon';

  render() {
    return html\`<article class="card">
      <h3 class="plan">\${this.plan}</h3>
      <p class="variant">Offre \${this.variant}</p>
    </article>\`;
  }
}
`;

const PROBE_CARD = `import { LitElement, html } from 'lit';
import { customElement } from 'lit/decorators.js';

async function attempt(run) {
  try {
    await run();
    return 'succeeded';
  } catch {
    return 'failed';
  }
}

function attemptNow(run) {
  try {
    run();
    return 'succeeded';
  } catch {
    return 'failed';
  }
}

// Essaie tout ce qu'un composant ne doit pas pouvoir faire depuis le canvas.
@customElement('hz-probe-card')
export class HzProbeCard extends LitElement {
  connectedCallback() {
    super.connectedCallback();
    void this.probe();
  }

  async probe() {
    const slashTraversal = new URL('..%2f..%2f..%2f${OUTSIDE_SECRET_FILE}', import.meta.url).href;
    const backslashTraversal = new URL('..%5c..%5c..%5c${OUTSIDE_SECRET_FILE}', import.meta.url).href;
    const results = {
      bridge: typeof globalThis.uiGen,
      process: typeof globalThis.process,
      require: typeof globalThis.require,
      network: await attempt(() => fetch('https://example.com/')),
      ownFile: await attempt(() => fetch(import.meta.url)),
      outsideFetch: await attempt(() => fetch(slashTraversal)),
      outsideImport: await attempt(() => import(slashTraversal)),
      outsideBackslashImport: await attempt(() => import(backslashTraversal)),
      parentDocument: attemptNow(() => window.parent.document.title),
      popup: window.open('https://example.com/') === null ? 'blocked' : 'opened',
    };
    attemptNow(() => {
      window.top.location.href = 'https://example.com/';
    });
    this.setAttribute('data-probe', JSON.stringify(results));
  }

  render() {
    return html\`<p class="probe">Sonde</p>\`;
  }
}
`;

const OUTSIDE_SECRET = `export const secret = 'leaked';
globalThis.leakedSecret = secret;
`;

export async function writeOfferCardProject(project: string): Promise<void> {
  await writeFiles(project, {
    'package.json': PACKAGE_JSON,
    'uigen.json': SETTINGS,
    'tsconfig.json': TSCONFIG,
    [PROJECT_TOKENS_FILE]: TOKENS_CSS,
    [OFFER_CARD_FILE]: OFFER_CARD,
  });
}

export async function writeProbeCardProject(project: string): Promise<void> {
  await writeFiles(project, {
    'package.json': PACKAGE_JSON,
    'uigen.json': SETTINGS,
    'tsconfig.json': TSCONFIG,
    [PROJECT_TOKENS_FILE]: TOKENS_CSS,
    [PROBE_CARD_FILE]: PROBE_CARD,
  });
  await writeFiles(join(project, '..'), { [OUTSIDE_SECRET_FILE]: OUTSIDE_SECRET });
}

export async function fileBytes(project: string, file: string): Promise<Buffer> {
  return readFile(join(project, file));
}

export async function rememberOpenedComponent(userData: string, project: string, tag: string): Promise<void> {
  await withStudio(userData, async (studio) => {
    await openProject(studio, project);
    await openComponent(studio.window, await listedPath(studio.window, tag));
  });
}

export async function openRecentProject(window: Page): Promise<void> {
  const link = window.getByRole('region', { name: 'Projets récents' }).getByRole('link').first();
  await expect(link).toBeVisible();
  await link.click();
  await expect(window.locator('ui-shell')).toHaveCount(1);
}

export function canvasIframe(window: Page): Locator {
  return window.locator('ui-canvas-frame iframe').last();
}

export function canvasContent(window: Page): FrameLocator {
  return canvasIframe(window).contentFrame();
}

export async function canvasFrame(window: Page): Promise<Frame> {
  const handle = await canvasIframe(window).elementHandle();
  const frame = await handle.contentFrame();
  if (frame === null) throw new Error('The canvas iframe has no frame');
  return frame;
}

export async function setDarkPreview(window: Page, enabled: boolean): Promise<void> {
  await window.locator('ui-canvas-frame').evaluate((element, value) => {
    Reflect.set(element, 'darkPreview', value);
  }, enabled);
}

export async function studioCustomProperties(frame: Frame): Promise<string[]> {
  const names: unknown = await frame.evaluate(`(() => {
    const elements = [document.documentElement, document.body, ...document.querySelectorAll('*')];
    const inShadow = [...document.querySelectorAll('*')]
      .flatMap((host) => (host.shadowRoot === null ? [] : [...host.shadowRoot.querySelectorAll('*')]));
    const names = new Set();
    for (const element of [...elements, ...inShadow]) {
      for (const name of getComputedStyle(element)) if (name.startsWith('--ui-')) names.add(name);
      for (const name of ['--ui-accent', '--ui-border', '--ui-canvas-dot']) {
        if (getComputedStyle(element).getPropertyValue(name) !== '') names.add(name);
      }
    }
    return [...names];
  })()`);
  if (!Array.isArray(names)) throw new Error('Expected a list of custom property names');
  return names.map(String);
}

export async function rootCustomProperty(frame: Frame, name: string): Promise<string> {
  const value: unknown = await frame.evaluate(
    `getComputedStyle(document.documentElement).getPropertyValue(${JSON.stringify(name)}).trim()`,
  );
  return String(value);
}
