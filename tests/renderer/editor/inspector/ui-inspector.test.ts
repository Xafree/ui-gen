import { userEvent } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cardComponent, codeLines } from '../code/code-fixtures.js';
import { isTextShown, press } from '../editor-fixtures.js';
import { clickLayer, layerName, selectedLayerIds } from '../layers/layer-fixtures.js';
import {
  LABEL_LINE,
  allShownValues,
  displayed,
  inspectorName,
  mountRecorded,
  sectionText,
  shownPropertyRows,
  shownSectionTitles,
  styledBadge,
  writableControls,
} from './inspector-fixtures.js';

const mounted: Element[] = [];

function codeLineAt(lineNumber: number): HTMLElement {
  const line = codeLines()[lineNumber - 1];
  if (line === undefined) throw new Error(`There is no code line ${String(lineNumber)}`);
  return line;
}

function codeTexts(): string[] {
  return codeLines().map((line) => line.textContent);
}

beforeEach(() => {
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
  for (const element of mounted.splice(0)) element.remove();
});

describe('the properties panel', () => {
  // covers: FCT-0041
  it('says no element is selected and shows none of its sections until an element is selected', async () => {
    await mountRecorded(mounted, styledBadge());

    expect(isTextShown('Aucun élément sélectionné')).toBe(true);
    expect(shownSectionTitles()).toEqual([]);

    await clickLayer('neutral:0');

    await expect.poll(() => isTextShown('Aucun élément sélectionné')).toBe(false);
    await expect.poll(() => shownSectionTitles()).toEqual(['Mise en page auto', 'Style', 'Typographie', 'Variante', 'États']);
  });

  // covers: FCT-0041
  it('is titled with the name of the selected layer', async () => {
    await mountRecorded(mounted, styledBadge());

    await clickLayer('neutral:0.1');

    await expect.poll(() => inspectorName()).toBe(layerName('neutral:0.1'));
    expect(inspectorName()).not.toBe('');

    await clickLayer('neutral:0');

    await expect.poll(() => inspectorName()).toBe(layerName('neutral:0'));
  });

  // covers: FCT-0041
  it('shows the values read from the code of the selected element, with the name of the token when the value is one', async () => {
    await mountRecorded(mounted, styledBadge());

    await clickLayer('neutral:0');

    await expect.poll(() => displayed('Mise en page auto', 'Espacement')).toBe('--hz-space-2');
    expect(displayed('Style', 'Rayon')).toBe('--hz-radius-md');
    expect(displayed('Style', 'Remplissage')).toBe('--hz-color-surface');
  });

  // covers: FCT-0041
  it('shows a value that is not a token as it is written in the code', async () => {
    await mountRecorded(mounted, styledBadge());

    await clickLayer('neutral:0.1');

    await expect.poll(() => displayed('Typographie', 'Taille')).toBe('13px');
    expect(displayed('Typographie', 'Graisse')).toBe('--hz-weight-medium');
  });

  // covers: FCT-0041
  it('reads the values of the only instance of a component that has no variant', async () => {
    await mountRecorded(mounted, cardComponent([]));

    await clickLayer('0.2');

    await expect.poll(() => displayed('Mise en page auto', 'Espacement')).toBe('--hz-space-4');
    expect(displayed('Style', 'Remplissage')).toBe('--hz-color-accent');
    expect(displayed('Typographie', 'Taille')).toBe('14px');
    await expect.poll(() => inspectorName()).toBe(layerName('0.2'));

    await clickLayer('0.0');

    await expect.poll(() => inspectorName()).toBe(layerName('0.0'));
    const values = allShownValues().join(' ');
    expect(values).not.toContain('--hz-space-4');
    expect(values).not.toContain('--hz-color-accent');
    expect(values).not.toContain('14px');
  });

  // covers: FCT-0041
  it('replaces the values when another element is selected, keeping none of the previous ones', async () => {
    await mountRecorded(mounted, styledBadge());
    await clickLayer('neutral:0');
    await expect.poll(() => displayed('Style', 'Remplissage')).toBe('--hz-color-surface');

    await clickLayer('neutral:0.1');

    await expect.poll(() => displayed('Typographie', 'Taille')).toBe('13px');
    const values = allShownValues().join(' ');
    expect(values).not.toContain('--hz-color-surface');
    expect(values).not.toContain('--hz-space-2');
    expect(values).not.toContain('--hz-radius-md');
  });

  // covers: FCT-0041
  it('reads the values of the variant of the selected instance, where its own rule overrides the common one', async () => {
    await mountRecorded(mounted, styledBadge());
    await clickLayer('neutral:0');
    await expect.poll(() => displayed('Style', 'Remplissage')).toBe('--hz-color-surface');
    await expect.poll(() => sectionText('Variante')).toContain('neutral');

    await clickLayer('accent:0');

    await expect.poll(() => displayed('Style', 'Remplissage')).toBe('--hz-color-accent');
    expect(displayed('Mise en page auto', 'Espacement')).toBe('--hz-space-2');
    expect(displayed('Style', 'Rayon')).toBe('--hz-radius-md');
    await expect.poll(() => sectionText('Variante')).toContain('accent');
  });
});

describe('selecting from the code', () => {
  // covers: FCT-0036, FCT-0041
  it('selects the element of a code line in the variant already selected, and shows its properties', async () => {
    await mountRecorded(mounted, styledBadge());
    await expect.poll(() => codeLines().length).toBeGreaterThan(0);
    await clickLayer('accent:0.0');
    await expect.poll(() => selectedLayerIds()).toEqual(['accent:0.0']);

    await userEvent.click(codeLineAt(LABEL_LINE));

    await expect.poll(() => selectedLayerIds()).toEqual(['accent:0.1']);
    await expect.poll(() => displayed('Typographie', 'Taille')).toBe('13px');
    await expect.poll(() => inspectorName()).toBe(layerName('accent:0.1'));
  });

  // covers: FCT-0036, FCT-0041
  it('selects the element of a code line in the first variant when no element is selected, and shows its properties', async () => {
    await mountRecorded(mounted, styledBadge());
    await expect.poll(() => codeLines().length).toBeGreaterThan(0);
    expect(selectedLayerIds()).toEqual([]);

    await userEvent.click(codeLineAt(LABEL_LINE));

    await expect.poll(() => selectedLayerIds()).toEqual(['neutral:0.1']);
    await expect.poll(() => displayed('Typographie', 'Taille')).toBe('13px');
    await expect.poll(() => inspectorName()).toBe(layerName('neutral:0.1'));
  });
});

describe('the properties panel is read-only', () => {
  // covers: FCT-0041
  it('offers no control that can be written, and none appears when its values are double-clicked', async () => {
    await mountRecorded(mounted, styledBadge());
    await clickLayer('neutral:0');
    await expect.poll(() => shownSectionTitles()).toHaveLength(5);
    await expect.poll(() => displayed('Style', 'Remplissage')).toBe('--hz-color-surface');
    expect(shownPropertyRows().length).toBeGreaterThan(0);
    expect(writableControls()).toEqual([]);

    for (const row of shownPropertyRows()) await userEvent.dblClick(row);

    expect(writableControls()).toEqual([]);
  });

  // covers: FCT-0041
  it('leaves the text of the file and the main process alone when its values are clicked and typed on', async () => {
    const { calls } = await mountRecorded(mounted, styledBadge());
    await expect.poll(() => codeLines().length).toBeGreaterThan(0);
    await clickLayer('neutral:0');
    await expect.poll(() => displayed('Style', 'Remplissage')).toBe('--hz-color-surface');
    expect(shownPropertyRows().length).toBeGreaterThan(0);
    const callsBefore = calls();
    const textBefore = codeTexts();

    for (const row of shownPropertyRows()) {
      await userEvent.click(row);
      await press('{Enter}');
      await press('x');
    }

    expect(calls()).toEqual(callsBefore);
    expect(codeTexts()).toEqual(textBefore);
    expect(displayed('Style', 'Remplissage')).toBe('--hz-color-surface');
  });
});
