import { expect } from 'vitest';
import type { ComponentModel, TemplateNode } from '../../../../src/modules/sync/application/component-model.js';
import type { OpenedComponent } from '../../../../src/modules/sync/application/components/component-views.js';
import type { UiGenApi } from '../../../../src/preload/ui-gen-api.js';
import { deepQueryAll, visibleText } from '../../home/shadow-dom.js';
import { PLAN_BADGE, fakeEditorApi, mountEditor, shown, type FakeEditorApi } from '../editor-fixtures.js';
import { treeItems } from '../layers/layer-fixtures.js';

type StyleRule = ComponentModel['styles'][number];
type VisualValue = StyleRule['values'][number];

export const BADGE_TEXT = `import { LitElement, css, html } from 'lit';
import { customElement, property } from 'lit/decorators.js';

// Badge du plan, une teinte par offre.
@customElement('hz-plan-badge')
export class HzPlanBadge extends LitElement {
  static styles = css\`
    .badge {
      display: flex;
      gap: var(--hz-space-2);
      border-radius: var(--hz-radius-md);
      background: var(--hz-color-surface);
    }
    .label {
      font-size: 13px;
      font-weight: var(--hz-weight-medium);
    }
    :host([variant='accent']) .badge {
      background: var(--hz-color-accent);
    }
  \`;

  @property({ reflect: true }) variant: 'neutral' | 'accent' = 'neutral';

  render() {
    return html\`
      <article class="badge">
        <h3 class="plan">Plan</h3>
        <span class="label">Pro</span>
      </article>
    \`;
  }
}
`;

export const HEADING_LINE = 28;
export const LABEL_LINE = 29;

const VARIANT_PROPERTY = "@property({ reflect: true }) variant: 'neutral' | 'accent' = 'neutral';";

export const SECTION_TITLES = ['Mise en page auto', 'Style', 'Typographie', 'Variante', 'États'] as const;

export type SectionTitle = (typeof SECTION_TITLES)[number];

const SECTION_IDS: Readonly<Record<SectionTitle, string>> = {
  'Mise en page auto': 'sec-layout',
  Style: 'sec-style',
  Typographie: 'sec-typo',
  Variante: 'sec-variant',
  États: 'sec-state',
};

const WRITABLE_CONTROLS = [
  'input',
  'select',
  'textarea',
  '[contenteditable]:not([contenteditable="false"])',
  '[role="textbox"]',
  '[role="combobox"]',
  '[role="spinbutton"]',
  '[role="slider"]',
  '[role="switch"]',
].join(', ');

export interface ShownProperty {
  readonly label: string;
  readonly value: string;
  readonly element: Element;
}

export interface RecordedEditor {
  readonly fake: FakeEditorApi;
  readonly calls: () => string[];
}

function rangeOf(snippet: string): { readonly start: number; readonly end: number } {
  const start = BADGE_TEXT.indexOf(snippet);
  return { start, end: start + snippet.length };
}

function element(id: string, tag: string, className: string, children: readonly TemplateNode[]): TemplateNode {
  const start = BADGE_TEXT.indexOf(`<${tag}`);
  const end = BADGE_TEXT.indexOf(`</${tag}>`, start) + `</${tag}>`.length;
  const attribute = `class="${className}"`;
  return {
    kind: 'element',
    id,
    tag,
    range: { start, end },
    attributes: [{ name: 'class', value: className, range: rangeOf(attribute) }],
    classes: [className],
    children,
  };
}

function visual(key: VisualValue['key'], raw: string, token: string | null): VisualValue {
  return { key, raw, token, range: rangeOf(raw) };
}

function rule(selector: string, values: readonly VisualValue[], targets: readonly string[], variant: StyleRule['variant']): StyleRule {
  const start = BADGE_TEXT.indexOf(`${selector} {`);
  return { selector, range: { start, end: BADGE_TEXT.indexOf('}', start) + 1 }, values, targets, variant, state: null };
}

function badgeModel(): ComponentModel {
  return {
    tag: 'hz-plan-badge',
    className: 'HzPlanBadge',
    version: 'v1',
    template: [
      element('0', 'article', 'badge', [element('0.0', 'h3', 'plan', []), element('0.1', 'span', 'label', [])]),
    ],
    properties: [
      {
        name: 'variant',
        decorator: 'property',
        type: "'neutral' | 'accent'",
        literalValues: ['neutral', 'accent'],
        defaultValue: "'neutral'",
        range: rangeOf(VARIANT_PROPERTY),
      },
    ],
    variantProperty: 'variant',
    variants: ['neutral', 'accent'],
    styles: [
      rule(
        '.badge',
        [
          visual('gap', 'var(--hz-space-2)', '--hz-space-2'),
          visual('radius', 'var(--hz-radius-md)', '--hz-radius-md'),
          visual('fill', 'var(--hz-color-surface)', '--hz-color-surface'),
        ],
        ['0'],
        null,
      ),
      rule(
        '.label',
        [visual('fontSize', '13px', null), visual('fontWeight', 'var(--hz-weight-medium)', '--hz-weight-medium')],
        ['0.1'],
        null,
      ),
      rule(":host([variant='accent']) .badge", [visual('fill', 'var(--hz-color-accent)', '--hz-color-accent')], ['0'], {
        property: 'variant',
        value: 'accent',
      }),
    ],
    diagnostics: [],
  };
}

export function styledBadge(): OpenedComponent {
  return { path: PLAN_BADGE.path, text: BADGE_TEXT, model: badgeModel(), highlight: [] };
}

function recording<T extends object>(target: T, namespace: string, calls: string[]): T {
  return new Proxy(target, {
    get(object, property) {
      const value: unknown = Reflect.get(object, property);
      if (typeof value !== 'function') return value;
      return (...args: unknown[]): unknown => {
        calls.push(`${namespace}.${String(property)}`);
        const result: unknown = Reflect.apply(value, object, args);
        return result;
      };
    },
  });
}

export function recordApi(api: UiGenApi): { readonly api: UiGenApi; readonly calls: () => string[] } {
  const calls: string[] = [];
  return {
    api: {
      platform: api.platform,
      recents: recording(api.recents, 'recents', calls),
      project: recording(api.project, 'project', calls),
      components: recording(api.components, 'components', calls),
    },
    calls: () => [...calls],
  };
}

export async function mountRecorded(mounted: Element[], component: OpenedComponent): Promise<RecordedEditor> {
  const fake = fakeEditorApi({ lastOpened: () => Promise.resolve(component.path), open: () => Promise.resolve(component) });
  const recorded = recordApi(fake.api);
  await mountEditor(recorded.api, mounted);
  await expect.poll(() => treeItems().length).toBeGreaterThan(0);
  return { fake, calls: recorded.calls };
}

function sectionOf(title: SectionTitle): Element | null {
  return shown(`#${SECTION_IDS[title]}`)?.closest('.ui-section, section') ?? null;
}

function byDocumentOrder(first: Element, second: Element): number {
  return first.compareDocumentPosition(second) & Node.DOCUMENT_POSITION_FOLLOWING ? -1 : 1;
}

export function shownSectionTitles(): string[] {
  return SECTION_TITLES.flatMap((title) => shown(`#${SECTION_IDS[title]}`) ?? [])
    .toSorted(byDocumentOrder)
    .map((heading) => visibleText(heading));
}

export function propertiesOf(title: SectionTitle): ShownProperty[] {
  const section = sectionOf(title);
  if (section === null) return [];
  const rows = [...section.querySelectorAll('.ui-prop')].flatMap((row) => {
    const label = row.querySelector('.ui-prop__label');
    const control = row.querySelector('.ui-prop__control');
    return label === null || control === null ? [] : [{ label: visibleText(label), value: visibleText(control), element: row }];
  });
  const pairs = [...section.querySelectorAll('dt')].flatMap((term) => {
    const detail = term.nextElementSibling;
    return detail?.localName === 'dd' ? [{ label: visibleText(term), value: visibleText(detail), element: detail }] : [];
  });
  return [...rows, ...pairs];
}

export function displayed(title: SectionTitle, label: string): string {
  const value = propertiesOf(title).find((property) => property.label === label)?.value ?? '';
  return value.replace(/\s+/g, '');
}

export function allShownValues(): string[] {
  return SECTION_TITLES.flatMap((title) => propertiesOf(title).map(({ value }) => value));
}

export function sectionText(title: SectionTitle): string {
  return visibleText(sectionOf(title)).toLowerCase();
}

export function writableControls(): Element[] {
  return SECTION_TITLES.flatMap((title) => [...(sectionOf(title)?.querySelectorAll(WRITABLE_CONTROLS) ?? [])]).filter(
    (control) =>
      control.checkVisibility() &&
      !control.matches(':disabled, [readonly]') &&
      control.getAttribute('aria-disabled') !== 'true' &&
      control.getAttribute('aria-readonly') !== 'true',
  );
}

export function shownPropertyRows(): Element[] {
  return SECTION_TITLES.flatMap((title) => propertiesOf(title).map(({ element: row }) => row));
}

export function inspectorName(): string {
  return visibleText(deepQueryAll('#insp-name')[0] ?? null);
}
