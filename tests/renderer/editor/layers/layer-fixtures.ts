import { expect } from 'vitest';
import { userEvent } from 'vitest/browser';
import type { ComponentModel, TemplateNode } from '../../../../src/modules/sync/application/component-model.js';
import type { ComponentChange, OpenedComponent } from '../../../../src/modules/sync/application/components/component-views.js';
import { NOTICE_REQUESTED, type NoticeRequestedDetail } from '../../../../src/renderer/studio-events.js';
import '../../../../src/renderer/editor/layers/ui-layer-tree.js';
import { deepActiveElement, deepQueryAll, shownToasts, visibleText } from '../../home/shadow-dom.js';
import { PLAN_BADGE, fakeEditorApi, mountEditor, textOf, type FakeEditorApi } from '../editor-fixtures.js';

export interface LayerEvent {
  readonly type: string;
  readonly id: unknown;
}

export interface SeenNotice {
  readonly title: string;
  readonly message: string;
}

const BADGE_SOURCE = `import { LitElement, css, html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

// Badge du plan, une teinte par offre.
@customElement('hz-plan-badge')
export class HzPlanBadge extends LitElement {
  static styles = css\`.badge { display: inline-flex; }\`;

  @property({ reflect: true }) variant: 'neutral' | 'accent' = 'neutral';

  render() {
    return html\`<article class="badge"><h3 class="plan"></h3><span class="label"></span></article>\`;
  }
}
`;

function element(id: string, tag: string, start: number, children: readonly TemplateNode[] = []): TemplateNode {
  return { kind: 'element', id, tag, range: { start, end: start + 10 }, attributes: [], classes: [], children };
}

export const BADGE_TEMPLATE: readonly TemplateNode[] = [
  element('0', 'article', 340, [element('0.0', 'h3', 366), element('0.1', 'span', 387)]),
];

export function badgeModel(version: string, template: readonly TemplateNode[] = BADGE_TEMPLATE): ComponentModel {
  return {
    tag: 'hz-plan-badge',
    className: 'HzPlanBadge',
    version,
    template,
    properties: [
      {
        name: 'variant',
        decorator: 'property',
        type: "'neutral' | 'accent'",
        literalValues: ['neutral', 'accent'],
        defaultValue: "'neutral'",
        range: { start: 230, end: 290 },
      },
    ],
    variantProperty: 'variant',
    variants: ['neutral', 'accent'],
    styles: [],
    diagnostics: [],
  };
}

export function badgeComponent(model: ComponentModel = badgeModel('v1')): OpenedComponent {
  return { path: PLAN_BADGE.path, text: BADGE_SOURCE, model, highlight: [] };
}

export function badgeChange(model: ComponentModel): ComponentChange {
  return { ...badgeComponent(model), diagnostics: [] };
}

export async function mountWithLayers(mounted: Element[], component: OpenedComponent = badgeComponent()): Promise<FakeEditorApi> {
  const fake = fakeEditorApi({
    lastOpened: () => Promise.resolve(PLAN_BADGE.path),
    open: () => Promise.resolve(component),
  });
  await mountEditor(fake.api, mounted);
  await expect.poll(() => textOf('#frame-name')).toBe(component.model.tag);
  await expect.poll(() => treeItems().length).toBeGreaterThan(0);
  return fake;
}

export function layerTree(): Element {
  const element = deepQueryAll('ui-layer-tree')[0];
  if (element === undefined) throw new Error('No <ui-layer-tree> is shown');
  return element;
}

export function tree(): HTMLElement {
  const element = deepQueryAll('[role="tree"]')[0];
  if (!(element instanceof HTMLElement)) throw new Error('No layer tree is shown');
  return element;
}

export function treeItems(): HTMLElement[] {
  return deepQueryAll('[role="tree"] [role="treeitem"]').filter((item): item is HTMLElement => item instanceof HTMLElement);
}

export function treeItem(id: string): HTMLElement {
  const item = treeItems().find((candidate) => candidate.dataset['id'] === id);
  if (item === undefined) throw new Error(`No tree item for layer ${id}`);
  return item;
}

export function itemIds(): string[] {
  return treeItems().map((item) => item.dataset['id'] ?? '');
}

export function ownRow(id: string): HTMLElement {
  const row = treeItem(id).querySelector<HTMLElement>(':scope > .ui-tree__row');
  if (row === null) throw new Error(`The tree item ${id} has no row`);
  return row;
}

export function layerName(id: string): string {
  return visibleText(ownRow(id).querySelector('.ui-tree__name'));
}

export function layerMeta(id: string): string {
  return visibleText(ownRow(id).querySelector('.ui-tree__meta'));
}

export function actionButton(id: string, action: 'hide' | 'lock'): HTMLElement {
  const button = ownRow(id).querySelector<HTMLElement>(`.ui-tree__actions [data-act="${action}"]`);
  if (button === null) throw new Error(`The layer ${id} has no ${action} button`);
  return button;
}

export function focusedLayerId(): string | null {
  const active = deepActiveElement();
  return active instanceof HTMLElement && active.getAttribute('role') === 'treeitem' ? (active.dataset['id'] ?? null) : null;
}

export function selectedLayerIds(): string[] {
  return treeItems()
    .filter((item) => item.getAttribute('aria-selected') === 'true')
    .map((item) => item.dataset['id'] ?? '');
}

export function tabStops(): string[] {
  return treeItems()
    .filter((item) => item.tabIndex === 0)
    .map((item) => item.dataset['id'] ?? '');
}

export async function expand(id: string): Promise<void> {
  const item = treeItem(id);
  if (item.getAttribute('aria-expanded') !== 'false') return;
  const toggle = item.querySelector<HTMLElement>(':scope > .ui-tree__row .ui-tree__toggle');
  if (toggle === null) throw new Error(`The layer ${id} has no toggle`);
  toggle.click();
  await expect.poll(() => treeItem(id).getAttribute('aria-expanded')).toBe('true');
}

export async function reveal(id: string): Promise<HTMLElement> {
  const ancestors: string[] = [];
  for (let parent = treeItem(id).parentElement?.closest('[role="treeitem"]'); parent instanceof HTMLElement; parent = parent.parentElement?.closest('[role="treeitem"]')) {
    ancestors.unshift(parent.dataset['id'] ?? '');
  }
  for (const ancestor of ancestors) await expand(ancestor);
  await expect.poll(() => treeItem(id).checkVisibility()).toBe(true);
  return treeItem(id);
}

export async function clickLayer(id: string): Promise<void> {
  await reveal(id);
  const name = ownRow(id).querySelector('.ui-tree__name');
  if (!(name instanceof HTMLElement)) throw new Error(`The layer ${id} has no name`);
  await userEvent.click(name);
}

export function recordLayerEvents(): LayerEvent[] {
  const events: LayerEvent[] = [];
  const target = layerTree();
  for (const type of ['layer-select', 'layer-hover']) {
    target.addEventListener(type, (event) => {
      const detail: unknown = event instanceof CustomEvent ? event.detail : null;
      const id: unknown = typeof detail === 'object' && detail !== null ? Reflect.get(detail, 'id') : undefined;
      events.push({ type, id });
    });
  }
  return events;
}

export function recordNotices(): { readonly seen: () => SeenNotice[]; readonly stop: () => void } {
  const requested: SeenNotice[] = [];
  const listener = (event: Event): void => {
    if (!(event instanceof CustomEvent)) return;
    const { notice } = event.detail as NoticeRequestedDetail;
    requested.push({ title: notice.title, message: notice.message });
  };
  document.addEventListener(NOTICE_REQUESTED, listener);
  return {
    seen: () => [...requested, ...shownToasts().map(({ title, message }) => ({ title, message }))],
    stop: () => {
      document.removeEventListener(NOTICE_REQUESTED, listener);
    },
  };
}
