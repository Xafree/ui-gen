import { userEvent } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { deepQueryAll } from '../../home/shadow-dom.js';
import { htmlElement, press, textOf } from '../editor-fixtures.js';
import { clickLayer, recordNotices, selectedLayerIds } from '../layers/layer-fixtures.js';
import { CARD_TEXT, cardComponent, codeLines, mountCard, warning } from './code-fixtures.js';

const mounted: Element[] = [];

function lineTexts(): string[] {
  return codeLines().map((line) => line.textContent);
}

function targetLines(): number[] {
  return codeLines().flatMap((line, index) => (line.classList.contains('is-target') ? [index + 1] : []));
}

function attribute(selector: string, name: string): string | null {
  return htmlElement(selector).getAttribute(name);
}

beforeEach(() => {
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
  for (const element of mounted.splice(0)) element.remove();
});

describe('the code of the opened component', () => {
  // covers: FCT-0038
  it('shows the exact text of the file, one numbered line after the other, without a phantom last line', async () => {
    await mountCard(mounted);

    expect(lineTexts().join('\n')).toBe(CARD_TEXT.slice(0, -1));
    expect(codeLines()).toHaveLength(CARD_TEXT.split('\n').length - 1);
    expect(getComputedStyle(codeLines()[0] as HTMLElement, '::before').content).toBe('counter(ln)');
    expect(textOf('#code-path')).toMatch(/hz-pricing-card\.ts$/);
    expect(textOf('#tab-code')).toBe('hz-pricing-card.ts');
  });

  // covers: FCT-0038
  it('colors the tokens of the highlight list with their tk-* class without changing the text', async () => {
    await mountCard(mounted);

    const colored = (className: string): string[] => deepQueryAll(`#code .${className}`).map((token) => token.textContent);
    expect(colored('tk-kw')).toEqual(['import']);
    expect(colored('tk-str')).toEqual(["'lit'"]);
    expect(colored('tk-dec')).toEqual(['@customElement']);
    expect(colored('tk-num')).toEqual(['29']);
    expect(lineTexts().join('\n')).toBe(CARD_TEXT.slice(0, -1));
  });

  // covers: FCT-0038
  it('keeps the lines of a CRLF file free of carriage returns and copies the file bytes unchanged', async () => {
    const crlf = CARD_TEXT.replaceAll('\n', '\r\n');
    const write = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue(undefined);
    await mountCard(mounted, cardComponent([], crlf));

    expect(codeLines()).toHaveLength(CARD_TEXT.split('\n').length - 1);
    expect(lineTexts().some((line) => line.includes('\r'))).toBe(false);
    await userEvent.click(htmlElement('#code-copy'));
    await expect.poll(() => write.mock.calls).toEqual([[crlf]]);
  });
});

describe('copying the code', () => {
  // covers: FCT-0038
  it('copies the text of the file and announces « <file> copié »', async () => {
    const write = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue(undefined);
    const notices = recordNotices();
    await mountCard(mounted);

    expect(attribute('#code-copy', 'aria-label')).toBe('Copier le code');
    await userEvent.click(htmlElement('#code-copy'));

    await expect.poll(() => write.mock.calls).toEqual([[CARD_TEXT]]);
    await expect.poll(() => notices.seen().map(({ title, message }) => `${title} ${message}`).join('|')).toContain('hz-pricing-card.ts copié');
    notices.stop();
  });

  // covers: FCT-0038
  it('announces « Copie impossible » when the clipboard refuses the text', async () => {
    vi.spyOn(navigator.clipboard, 'writeText').mockRejectedValue(new Error('blocked'));
    const notices = recordNotices();
    await mountCard(mounted);

    await userEvent.click(htmlElement('#code-copy'));

    await expect.poll(() => notices.seen().map(({ title, message }) => `${title} ${message}`).join('|')).toContain('Copie impossible');
    notices.stop();
  });
});

describe('the code panel', () => {
  // covers: FCT-0038
  it('folds and unfolds from its button and from Ctrl J, with matching state and labels', async () => {
    await mountCard(mounted);
    expect(attribute('#code-collapse', 'aria-expanded')).toBe('true');
    expect(attribute('#code-collapse', 'aria-label')).toBe('Replier le panneau de code');

    await userEvent.click(htmlElement('#code-collapse'));
    expect(attribute('#code-collapse', 'aria-expanded')).toBe('false');
    expect(attribute('#code-collapse', 'aria-label')).toBe('Déplier le panneau de code');
    expect(attribute('#code-panel', 'data-collapsed')).toBe('true');

    await press('{Control>}j{/Control}');
    expect(attribute('#code-collapse', 'aria-expanded')).toBe('true');
    expect(attribute('#code-panel', 'data-collapsed')).toBe('false');

    await press('{Control>}j{/Control}');
    expect(attribute('#code-collapse', 'aria-expanded')).toBe('false');
  });

  // covers: FCT-0038
  it('resizes from 120 px to its maximum with the arrow keys by 16 px, Home, End, and resets to 272 px on double click', async () => {
    await mountCard(mounted);
    const split = htmlElement('#code-split');
    const value = (): number => Number(split.getAttribute('aria-valuenow'));
    expect([split.getAttribute('role'), split.getAttribute('aria-valuemin'), value()]).toEqual(['separator', '120', 272]);
    split.focus();

    await press('{ArrowUp}');
    expect(value()).toBe(288);
    await press('{ArrowDown}{ArrowDown}');
    expect(value()).toBe(256);
    await press('{Home}');
    expect(value()).toBe(120);
    await press('{ArrowDown}');
    expect(value()).toBe(120);
    await press('{End}');
    const maximum = Number(split.getAttribute('aria-valuemax'));
    expect(value()).toBe(maximum);
    expect(maximum).toBeGreaterThanOrEqual(160);
    expect(maximum).toBeLessThanOrEqual(600);
    await press('{ArrowUp}');
    expect(value()).toBe(maximum);

    await userEvent.dblClick(split);
    expect(value()).toBe(272);
  });
});

describe('selecting from the code', () => {
  // covers: FCT-0036
  it('highlights the lines of the selected layer and only them', async () => {
    await mountCard(mounted);
    expect(targetLines()).toEqual([]);

    await clickLayer('0.2');
    await expect.poll(() => targetLines()).toEqual([13]);

    await clickLayer('0');
    await expect.poll(() => targetLines()).toEqual([10, 11, 12, 13, 14]);
  });

  // covers: FCT-0036
  it('selects the element of a code line on click', async () => {
    await mountCard(mounted);

    await userEvent.click(codeLines()[10] as HTMLElement);

    await expect.poll(() => selectedLayerIds()).toEqual(['0.0']);
    await expect.poll(() => targetLines()).toEqual([11]);
  });

  // covers: FCT-0036
  it('selects the element of the cursor line on Enter, and nothing for a line outside the template', async () => {
    await mountCard(mounted);
    htmlElement('#code-pre').focus();

    await press('{Control>}{Home}{/Control}');
    await press('{Enter}');
    expect(selectedLayerIds()).toEqual([]);

    await press('{ArrowDown>12/}');
    await press('{Enter}');

    await expect.poll(() => selectedLayerIds()).toEqual(['0.2']);
    await expect.poll(() => targetLines()).toEqual([13]);
    expect(codeLines()[12]?.classList.contains('is-active')).toBe(true);
  });

  // covers: FCT-0038
  it('shows the cursor position of the code in the status bar', async () => {
    await mountCard(mounted, cardComponent([warning()]));
    htmlElement('#code-pre').focus();

    await press('{Control>}{Home}{/Control}');

    await expect.poll(() => textOf('#status-cursor')).toMatch(/^Ligne 1, col\. 1/);
  });
});
