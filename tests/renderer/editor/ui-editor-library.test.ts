import { page } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ComponentSummary } from '../../../src/modules/sync/domain/library/component-summary.js';
import { deepActiveElement } from '../home/shadow-dom.js';
import {
  FEATURE_LIST,
  PLAN_BADGE,
  PRICING_CARD,
  fakeEditorApi,
  isShown,
  isTextShown,
  mountEditor,
  openedMarks,
  optionFor,
  optionNames,
  press,
  shownCanvasComponentPath,
  textOf,
} from './editor-fixtures.js';

const mounted: Element[] = [];

beforeEach(() => {
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
  for (const element of mounted.splice(0)) element.remove();
});

async function showComponentsTab(): Promise<void> {
  await page.getByRole('tab', { name: /^Composants/ }).click();
  await expect.poll(() => isShown('#comp-list')).toBe(true);
}

async function expectOpenedTag(tag: string): Promise<void> {
  await expect.poll(() => textOf('#frame-name')).toBe(tag);
  expect(textOf('#page-title')).toBe(`Éditeur du composant ${tag}`);
  expect(textOf('#layers-file')).toBe(`${tag}.ts`);
}

describe('<ui-editor> arrival', () => {
  // covers: FCT-0026
  it('opens the last component opened in this project', async () => {
    const fake = fakeEditorApi({ lastOpened: () => Promise.resolve(PLAN_BADGE.path) });

    await mountEditor(fake.api, mounted);

    await expectOpenedTag('hz-plan-badge');
    expect(fake.opened).toEqual([PLAN_BADGE.path]);
    await expect.poll(() => shownCanvasComponentPath()).toBe(PLAN_BADGE.path);
    await showComponentsTab();
    expect(openedMarks()).toEqual(['PlanBadge']);
  });

  // covers: FCT-0026
  it('opens the first component by tag order when none was opened before', async () => {
    const fake = fakeEditorApi({ lastOpened: () => Promise.resolve(null) });

    await mountEditor(fake.api, mounted);

    await expectOpenedTag('hz-feature-list');
    expect(fake.opened).toEqual([FEATURE_LIST.path]);
    await showComponentsTab();
    expect(openedMarks()).toEqual(['FeatureList']);
  });

  it('does not show the empty library state while the library is still loading', async () => {
    const fake = fakeEditorApi({ lastOpened: () => new Promise<string | null>(() => undefined) });

    await mountEditor(fake.api, mounted);

    expect(isShown('#layers-empty')).toBe(false);
    expect(isShown('#canvas-empty')).toBe(false);
    expect(fake.opened).toEqual([]);
  });
});

describe('<ui-editor> components tab', () => {
  // covers: FCT-0028
  it('lists each component by its class name without the project prefix, with its tag, and counts them', async () => {
    const fake = fakeEditorApi({ lastOpened: () => Promise.resolve(PRICING_CARD.path) });
    await mountEditor(fake.api, mounted);
    await expectOpenedTag('hz-pricing-card');

    await showComponentsTab();

    expect(optionNames()).toEqual(['FeatureList', 'PlanBadge', 'PricingCard']);
    expect(textOf('#comp-count')).toBe('3');
    expect(optionFor('PricingCard').querySelector('.ed-comp__tag')?.textContent.trim()).toBe('<hz-pricing-card>');
    expect(openedMarks()).toEqual(['PricingCard']);
  });

  // covers: FCT-0028
  it('opens a component clicked in the list and marks it « Ouvert »', async () => {
    const fake = fakeEditorApi({ lastOpened: () => Promise.resolve(PRICING_CARD.path) });
    await mountEditor(fake.api, mounted);
    await expectOpenedTag('hz-pricing-card');
    await showComponentsTab();

    optionFor('FeatureList').click();

    await expectOpenedTag('hz-feature-list');
    expect(fake.opened).toEqual([PRICING_CARD.path, FEATURE_LIST.path]);
    await expect.poll(() => openedMarks()).toEqual(['FeatureList']);
    await expect.poll(() => shownCanvasComponentPath()).toBe(FEATURE_LIST.path);
  });

  // covers: FCT-0028
  it('opens the component focused in the list with Enter', async () => {
    const fake = fakeEditorApi({ lastOpened: () => Promise.resolve(PRICING_CARD.path) });
    await mountEditor(fake.api, mounted);
    await expectOpenedTag('hz-pricing-card');
    await showComponentsTab();

    optionFor('PlanBadge').focus();
    await press('{Enter}');

    await expectOpenedTag('hz-plan-badge');
    expect(fake.opened).toEqual([PRICING_CARD.path, PLAN_BADGE.path]);
    await expect.poll(() => openedMarks()).toEqual(['PlanBadge']);
  });

  // covers: FCT-0028
  it('only moves the choice with the arrow keys, without opening anything', async () => {
    const fake = fakeEditorApi({ lastOpened: () => Promise.resolve(PRICING_CARD.path) });
    await mountEditor(fake.api, mounted);
    await expectOpenedTag('hz-pricing-card');
    await showComponentsTab();

    optionFor('FeatureList').focus();
    await press('{ArrowDown}');

    await expect.poll(() => deepActiveElement()).toBe(optionFor('PlanBadge'));
    expect(optionFor('PlanBadge').getAttribute('aria-selected')).toBe('true');
    expect(fake.opened).toEqual([PRICING_CARD.path]);
    expect(openedMarks()).toEqual(['PricingCard']);
    expect(textOf('#frame-name')).toBe('hz-pricing-card');
  });
});

describe('<ui-editor> empty library', () => {
  const noComponent: readonly ComponentSummary[] = [];

  // covers: FCT-0027
  it('shows « Aucun composant dans cette bibliothèque » with « Importer un composant » in the layers and on the canvas', async () => {
    const fake = fakeEditorApi({ library: noComponent, lastOpened: () => Promise.resolve(null) });

    await mountEditor(fake.api, mounted);

    await expect.poll(() => isShown('#layers-empty')).toBe(true);
    expect(textOf('#layers-empty .ui-empty__title')).toBe('Aucun composant dans cette bibliothèque');
    expect(isShown('#layers-empty a')).toBe(true);
    expect(textOf('#layers-empty a')).toBe('Importer un composant');
    expect(isShown('#canvas-empty')).toBe(true);
    expect(textOf('#canvas-empty .ui-empty__title')).toBe('Aucun composant dans cette bibliothèque');
    expect(textOf('#canvas-empty a')).toBe('Importer un composant');
    await expect.element(page.getByRole('link', { name: 'Importer un composant' }).first()).toBeVisible();
    expect(fake.opened).toEqual([]);
    expect(isShown('ui-canvas-frame')).toBe(false);
    expect(isShown('#toolbar')).toBe(false);
    expect(isShown('#frame')).toBe(false);
  });

  // covers: FCT-0027
  it('counts zero components and gives the empty library state in the components tab, without « Effacer la recherche »', async () => {
    const fake = fakeEditorApi({ library: noComponent, lastOpened: () => Promise.resolve(null) });
    await mountEditor(fake.api, mounted);
    await expect.poll(() => isShown('#layers-empty')).toBe(true);

    await page.getByRole('tab', { name: /^Composants/ }).click();

    await expect.poll(() => isShown('#comp-empty')).toBe(true);
    expect(textOf('#comp-count')).toBe('0');
    expect(textOf('#comp-empty-title')).toBe('Aucun composant dans cette bibliothèque');
    expect(isShown('#comp-clear')).toBe(false);
    expect(optionNames()).toEqual([]);
  });
});

describe('<ui-editor> analysis status', () => {
  it('says the code analysis is restarting, then stops saying it once ready', async () => {
    const fake = fakeEditorApi({ lastOpened: () => Promise.resolve(PRICING_CARD.path) });
    await mountEditor(fake.api, mounted);
    await expectOpenedTag('hz-pricing-card');
    expect(isTextShown('Analyse du code relancée…')).toBe(false);

    fake.sendStatus('restarting');

    await expect.poll(() => isTextShown('Analyse du code relancée…')).toBe(true);

    fake.sendStatus('ready');

    await expect.poll(() => isTextShown('Analyse du code relancée…')).toBe(false);
  });

  it('stops following the component changes and the analysis status once removed', async () => {
    const fake = fakeEditorApi({ lastOpened: () => Promise.resolve(PRICING_CARD.path) });
    const editor = await mountEditor(fake.api, mounted);
    await expectOpenedTag('hz-pricing-card');

    editor.remove();

    expect(fake.unsubscribed.toSorted()).toEqual(['changes', 'status']);
  });
});
