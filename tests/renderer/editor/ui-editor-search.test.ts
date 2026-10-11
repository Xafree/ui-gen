import { page, type Locator } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { deepActiveElement } from '../home/shadow-dom.js';
import {
  PRICING_CARD,
  fakeEditorApi,
  htmlElement,
  isShown,
  mountEditor,
  openedMarks,
  optionNames,
  textOf,
} from './editor-fixtures.js';

const mounted: Element[] = [];

beforeEach(async () => {
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
  const fake = fakeEditorApi({ lastOpened: () => Promise.resolve(PRICING_CARD.path) });
  await mountEditor(fake.api, mounted);
  await expect.poll(() => textOf('#frame-name')).toBe('hz-pricing-card');
  await page.getByRole('tab', { name: /^Composants/ }).click();
  await expect.poll(() => optionNames()).toEqual(['FeatureList', 'PlanBadge', 'PricingCard']);
});

afterEach(() => {
  vi.restoreAllMocks();
  for (const element of mounted.splice(0)) element.remove();
});

function searchBox(): Locator {
  return page.getByRole('searchbox', { name: 'Rechercher un composant' });
}

describe('<ui-editor> component search', () => {
  // covers: FCT-0029
  it('keeps only the components whose name contains the search, whatever its case', async () => {
    await searchBox().fill('PRICING');

    await expect.poll(() => optionNames()).toEqual(['PricingCard']);
    expect(openedMarks()).toEqual(['PricingCard']);
    expect(isShown('#comp-empty')).toBe(false);

    await searchBox().fill('badge');

    await expect.poll(() => optionNames()).toEqual(['PlanBadge']);
  });

  // covers: FCT-0029
  it('keeps the components whose tag contains the search', async () => {
    await searchBox().fill('hz-feat');

    await expect.poll(() => optionNames()).toEqual(['FeatureList']);

    await searchBox().fill('HZ-P');

    await expect.poll(() => optionNames()).toEqual(['PlanBadge', 'PricingCard']);
  });

  // covers: FCT-0029
  it('says no component contains the search and offers « Effacer la recherche »', async () => {
    await searchBox().fill('Switch');

    await expect.poll(() => isShown('#comp-empty')).toBe(true);
    expect(optionNames()).toEqual([]);
    expect(textOf('#comp-empty-title')).toBe('Aucun composant ne contient « Switch »');
    await expect.element(page.getByRole('button', { name: 'Effacer la recherche' })).toBeVisible();
    expect(textOf('#comp-count')).toBe('3');
  });

  // covers: FCT-0029
  it('lists every component again and gives the focus back to the search once cleared', async () => {
    await searchBox().fill('Switch');
    await expect.poll(() => isShown('#comp-empty')).toBe(true);

    await page.getByRole('button', { name: 'Effacer la recherche' }).click();

    await expect.poll(() => optionNames()).toEqual(['FeatureList', 'PlanBadge', 'PricingCard']);
    expect(isShown('#comp-empty')).toBe(false);
    await expect.element(searchBox()).toHaveValue('');
    expect(deepActiveElement()).toBe(htmlElement('#comp-search'));
  });
});
