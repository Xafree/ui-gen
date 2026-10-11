import { LitElement } from 'lit';
import { userEvent } from 'vitest/browser';
import type { ComponentModel } from '../../../src/modules/sync/application/component-model.js';
import type { ComponentChange, OpenedComponent } from '../../../src/modules/sync/application/components/component-views.js';
import type { ComponentSummary } from '../../../src/modules/sync/domain/library/component-summary.js';
import type { AnalyzerStatus } from '../../../src/modules/sync/ports/component-analyzer.js';
import type { UiGenApi } from '../../../src/preload/ui-gen-api.js';
import '../../../src/renderer/editor/ui-editor.js';
import { makeUiGenApi } from '../home/home-fixtures.js';
import { deepQueryAll, visibleText } from '../home/shadow-dom.js';

const COMPONENTS_DIR = 'E:\\projets\\kit-horizon\\src\\components';

export const FEATURE_LIST: ComponentSummary = {
  path: `${COMPONENTS_DIR}\\hz-feature-list.ts`,
  tag: 'hz-feature-list',
  name: 'Feature List',
};

export const PLAN_BADGE: ComponentSummary = {
  path: `${COMPONENTS_DIR}\\badge\\hz-plan-badge.ts`,
  tag: 'hz-plan-badge',
  name: 'Plan Badge',
};

export const PRICING_CARD: ComponentSummary = {
  path: `${COMPONENTS_DIR}\\hz-pricing-card.ts`,
  tag: 'hz-pricing-card',
  name: 'Pricing Card',
};

export const KIT_HORIZON_LIBRARY: readonly ComponentSummary[] = [FEATURE_LIST, PLAN_BADGE, PRICING_CARD];

const CLASS_NAMES: Readonly<Record<string, string>> = {
  'hz-feature-list': 'HzFeatureList',
  'hz-plan-badge': 'HzPlanBadge',
  'hz-pricing-card': 'HzPricingCard',
};

const VARIANTS: Readonly<Record<string, readonly string[]>> = {
  'hz-plan-badge': ['neutral', 'accent'],
  'hz-pricing-card': ['starter', 'pro', 'equipe'],
};

function sourceOf(tag: string, className: string): string {
  return `import { LitElement, html } from 'lit';
import { customElement } from 'lit/decorators.js';

// Écrit à la main pour le Kit Horizon.
@customElement('${tag}')
export class ${className} extends LitElement {
  render() {
    return html\`<article class="card">
      <h3 class="plan">Horizon</h3>
    </article>\`;
  }
}
`;
}

function modelOf(tag: string, version: string): ComponentModel {
  const className = CLASS_NAMES[tag] ?? 'HzUnknown';
  const variants = VARIANTS[tag] ?? [];
  return {
    tag,
    className,
    version,
    template: [
      {
        kind: 'element',
        id: '0',
        tag: 'article',
        range: { start: 214, end: 279 },
        attributes: [{ name: 'class', value: 'card', range: { start: 223, end: 235 } }],
        classes: ['card'],
        children: [
          {
            kind: 'element',
            id: '0.0',
            tag: 'h3',
            range: { start: 242, end: 268 },
            attributes: [{ name: 'class', value: 'plan', range: { start: 246, end: 258 } }],
            classes: ['plan'],
            children: [{ kind: 'text', id: '0.0.0', range: { start: 259, end: 266 }, text: 'Horizon' }],
          },
        ],
      },
    ],
    properties:
      variants.length === 0
        ? []
        : [
            {
              name: 'variant',
              decorator: 'property',
              type: variants.map((value) => `'${value}'`).join(' | '),
              literalValues: variants,
              defaultValue: `'${variants[0] ?? ''}'`,
              range: { start: 180, end: 200 },
            },
          ],
    variantProperty: variants.length === 0 ? null : 'variant',
    variants,
    styles: [],
    diagnostics: [],
  };
}

export function openedComponent(summary: ComponentSummary, version = 'v1'): OpenedComponent {
  const className = CLASS_NAMES[summary.tag] ?? 'HzUnknown';
  return {
    path: summary.path,
    text: sourceOf(summary.tag, className),
    model: modelOf(summary.tag, version),
    highlight: [],
  };
}

export interface EditorApiOptions {
  readonly library?: readonly ComponentSummary[];
  readonly lastOpened?: () => Promise<string | null>;
  readonly list?: () => Promise<ComponentSummary[]>;
  readonly open?: (path: string) => Promise<OpenedComponent>;
}

export interface FakeEditorApi {
  readonly api: UiGenApi;
  readonly opened: string[];
  readonly unsubscribed: string[];
  sendStatus(status: AnalyzerStatus): void;
  sendChange(change: ComponentChange): void;
}

function openFrom(library: readonly ComponentSummary[]): (path: string) => Promise<OpenedComponent> {
  return (path) => {
    const summary = library.find((candidate) => candidate.path === path);
    return summary === undefined ? Promise.reject(new Error(`Unknown component ${path}`)) : Promise.resolve(openedComponent(summary));
  };
}

export function fakeEditorApi(options: EditorApiOptions): FakeEditorApi {
  const library = options.library ?? KIT_HORIZON_LIBRARY;
  const opened: string[] = [];
  const unsubscribed: string[] = [];
  const statusListeners: ((status: AnalyzerStatus) => void)[] = [];
  const changeListeners: ((change: ComponentChange) => void)[] = [];
  const open = options.open ?? openFrom(library);
  const base = makeUiGenApi({});
  const api: UiGenApi = {
    ...base,
    components: {
      list: options.list ?? (() => Promise.resolve([...library])),
      lastOpened: options.lastOpened ?? (() => Promise.resolve(null)),
      open: (path) => {
        opened.push(path);
        return open(path);
      },
      onChanged: (listener) => {
        changeListeners.push(listener);
        return () => {
          unsubscribed.push('changes');
        };
      },
      onStatus: (listener) => {
        statusListeners.push(listener);
        return () => {
          unsubscribed.push('status');
        };
      },
    },
  };
  return {
    api,
    opened,
    unsubscribed,
    sendStatus: (status) => {
      for (const listener of statusListeners) listener(status);
    },
    sendChange: (change) => {
      for (const listener of changeListeners) listener(change);
    },
  };
}

export async function mountEditor(api: UiGenApi, mounted: Element[]): Promise<LitElement> {
  const editor = document.createElement('ui-editor');
  Object.assign(editor, { api });
  editor.style.position = 'fixed';
  editor.style.inset = '0';
  document.body.append(editor);
  mounted.push(editor);
  if (!(editor instanceof LitElement)) throw new Error('<ui-editor> is not a Lit element');
  await editor.updateComplete;
  return editor;
}

export function shown(selector: string): Element | undefined {
  return deepQueryAll(selector).find((element) => element.checkVisibility());
}

export function isShown(selector: string): boolean {
  return shown(selector) !== undefined;
}

export function isTextShown(text: string): boolean {
  return deepQueryAll('*').some((element) => visibleText(element) === text && element.checkVisibility());
}

export function textOf(selector: string): string {
  return visibleText(deepQueryAll(selector)[0] ?? null);
}

export function htmlElement(selector: string): HTMLElement {
  const element = deepQueryAll(selector)[0];
  if (!(element instanceof HTMLElement)) throw new Error(`Expected an HTML element for ${selector}`);
  return element;
}

export function componentOptions(): Element[] {
  return deepQueryAll('#comp-list [role="option"]').filter((option) => option.checkVisibility());
}

export function optionNames(): string[] {
  return componentOptions().map((option) => visibleText(option.querySelector('.ed-comp__name')));
}

export function optionFor(name: string): HTMLElement {
  const option = componentOptions().find((candidate) => visibleText(candidate.querySelector('.ed-comp__name')) === name);
  if (!(option instanceof HTMLElement)) throw new Error(`No component option named ${name}`);
  return option;
}

export function openedMarks(): string[] {
  return deepQueryAll('#comp-list [role="option"]')
    .filter((option) => visibleText(option.querySelector('.ed-comp__mark')) === 'Ouvert')
    .map((option) => visibleText(option.querySelector('.ed-comp__name')));
}

export function shownCanvasComponentPath(): unknown {
  const frame = deepQueryAll('ui-canvas-frame')[0];
  if (frame === undefined) return undefined;
  const component: unknown = Reflect.get(frame, 'component');
  return typeof component === 'object' && component !== null ? Reflect.get(component, 'path') : null;
}

export async function press(key: string): Promise<void> {
  await userEvent.keyboard(key);
}
