import { page, type Locator } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PRICING_CARD, fakeEditorApi, htmlElement, mountEditor, press, textOf } from './editor-fixtures.js';

const mounted: Element[] = [];

beforeEach(async () => {
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
  const fake = fakeEditorApi({ lastOpened: () => Promise.resolve(PRICING_CARD.path) });
  await mountEditor(fake.api, mounted);
  await expect.poll(() => textOf('#frame-name')).toBe('hz-pricing-card');
});

afterEach(() => {
  vi.restoreAllMocks();
  for (const element of mounted.splice(0)) element.remove();
});

function screenSize(label: string): Locator {
  return page.getByRole('radiogroup', { name: "Taille d'écran du cadre" }).getByText(label, { exact: true });
}

function zoomValue(): Locator {
  return page.getByRole('button', { name: /^Zoom : / });
}

function zoomIn(): Locator {
  return page.getByRole('button', { name: 'Zoom avant', exact: true });
}

function zoomOut(): Locator {
  return page.getByRole('button', { name: 'Zoom arrière', exact: true });
}

function shownZoom(): string {
  return textOf('#zoom-value');
}

function zoomPercent(): number {
  return Number.parseInt(shownZoom(), 10);
}

function frameWidth(): number {
  return htmlElement('#frame-body').offsetWidth;
}

async function resetZoom(): Promise<void> {
  await zoomValue().click();
  await expect.poll(() => shownZoom()).toBe('100 %');
}

async function clickThrough(button: Locator, times: number): Promise<string[]> {
  const seen: string[] = [];
  for (let click = 0; click < times; click += 1) {
    await button.click();
    seen.push(shownZoom());
  }
  return seen;
}

function wheel(deltaY: number, ctrlKey: boolean): void {
  const viewport = htmlElement('#viewport');
  const box = viewport.getBoundingClientRect();
  viewport.dispatchEvent(
    new WheelEvent('wheel', {
      deltaY,
      ctrlKey,
      clientX: box.left + box.width / 2,
      clientY: box.top + box.height / 2,
      bubbles: true,
      cancelable: true,
      composed: true,
    }),
  );
}

describe('<ui-editor> screen size of the frame', () => {
  // covers: FCT-0033
  it('shows the Ordinateur frame, 1 024 px wide, by default', async () => {
    await expect.poll(() => frameWidth()).toBe(1024);
    expect(textOf('#frame-chip')).toBe('1 024 px');
    expect(textOf('#frame-meta')).toBe('3 variantes · Ordinateur');
    await expect.element(page.getByRole('radio', { name: 'Ordinateur' })).toBeChecked();
  });

  // covers: FCT-0033
  it('gives the frame 375 px on Mobile, 768 px on Tablette and 1 024 px on Ordinateur, and says so on its label', async () => {
    await screenSize('Mobile').click();

    await expect.poll(() => frameWidth()).toBe(375);
    expect(textOf('#frame-chip')).toBe('375 px');
    expect(textOf('#frame-meta')).toBe('3 variantes · Mobile');
    await expect.element(page.getByRole('radio', { name: 'Mobile' })).toBeChecked();

    await screenSize('Tablette').click();

    await expect.poll(() => frameWidth()).toBe(768);
    expect(textOf('#frame-chip')).toBe('768 px');
    expect(textOf('#frame-meta')).toBe('3 variantes · Tablette');

    await screenSize('Ordinateur').click();

    await expect.poll(() => frameWidth()).toBe(1024);
    expect(textOf('#frame-chip')).toBe('1 024 px');
    expect(textOf('#frame-meta')).toBe('3 variantes · Ordinateur');
  });
});

describe('<ui-editor> canvas zoom', () => {
  // covers: FCT-0034
  it('goes back to 100 % on a click on the zoom value', async () => {
    await zoomIn().click();
    await zoomIn().click();

    await zoomValue().click();

    await expect.poll(() => shownZoom()).toBe('100 %');
    await expect.element(zoomValue()).toHaveAccessibleName('Zoom : 100 %. Revenir à 100 %');
  });

  // covers: FCT-0034
  it('zooms in by steps of 125, 150, 200, 300 and 400 %, and no further', async () => {
    await resetZoom();

    expect(await clickThrough(zoomIn(), 6)).toEqual(['125 %', '150 %', '200 %', '300 %', '400 %', '400 %']);
  });

  // covers: FCT-0034
  it('zooms out by steps of 75, 67, 50, 33 and 25 %, and no further', async () => {
    await resetZoom();

    expect(await clickThrough(zoomOut(), 6)).toEqual(['75 %', '67 %', '50 %', '33 %', '25 %', '25 %']);
  });

  // covers: FCT-0034
  it('zooms by steps with Ctrl + and Ctrl −', async () => {
    await resetZoom();
    htmlElement('#viewport').focus();

    await press('{Control>}={/Control}');
    await expect.poll(() => shownZoom()).toBe('125 %');
    await press('{Control>}+{/Control}');
    await expect.poll(() => shownZoom()).toBe('150 %');
    await press('{Control>}-{/Control}');
    await expect.poll(() => shownZoom()).toBe('125 %');
    await press('{Control>}-{/Control}');
    await press('{Control>}-{/Control}');
    await expect.poll(() => shownZoom()).toBe('75 %');
  });

  // covers: FCT-0034
  it('zooms with Ctrl and the wheel, never above 400 % nor below 25 %', async () => {
    await resetZoom();

    wheel(-120, true);
    await expect.poll(() => zoomPercent()).toBeGreaterThan(100);

    for (let turn = 0; turn < 40; turn += 1) wheel(-480, true);
    await expect.poll(() => shownZoom()).toBe('400 %');

    for (let turn = 0; turn < 80; turn += 1) wheel(480, true);
    await expect.poll(() => shownZoom()).toBe('25 %');
  });

  // covers: FCT-0034
  it('does not zoom on the wheel without Ctrl', async () => {
    await resetZoom();

    for (let turn = 0; turn < 5; turn += 1) wheel(-240, false);

    expect(shownZoom()).toBe('100 %');
  });

  // covers: FCT-0034
  it('fits the whole frame width in the view with « Ajuster »', async () => {
    await resetZoom();
    await clickThrough(zoomIn(), 5);
    expect(shownZoom()).toBe('400 %');

    await page.getByRole('button', { name: 'Ajuster le cadre à la vue' }).click();

    await expect.poll(() => zoomPercent()).toBeLessThanOrEqual(100);
    expect(zoomPercent()).toBeGreaterThanOrEqual(25);
    const view = htmlElement('#viewport').getBoundingClientRect();
    const frame = htmlElement('#frame').getBoundingClientRect();
    expect(frame.width).toBeGreaterThan(0);
    expect(frame.left).toBeGreaterThanOrEqual(view.left);
    expect(frame.right).toBeLessThanOrEqual(view.right);
  });
});
