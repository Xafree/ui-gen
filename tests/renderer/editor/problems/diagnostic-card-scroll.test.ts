import { userEvent } from 'vitest/browser';
import { afterEach, describe, expect, it } from 'vitest';
import { mountCard } from '../code/code-fixtures.js';
import { htmlElement, isShown } from '../editor-fixtures.js';
import { underlineAt } from './problems-fixtures.js';

const mounted: Element[] = [];

afterEach(() => {
  for (const element of mounted.splice(0)) element.remove();
});

function nextFrame(): Promise<void> {
  return new Promise((resolve) => {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        resolve();
      });
    });
  });
}

describe('the diagnostic card of the code view', () => {
  // covers: FCT-0039
  it('stays open on a scroll event that moved nothing since it opened and closes when the code really scrolls', async () => {
    await mountCard(mounted);
    const pre = htmlElement('#code-pre');
    pre.style.height = '60px';
    await userEvent.hover(underlineAt(0));
    await expect.poll(() => isShown('#diag-card')).toBe(true);

    pre.dispatchEvent(new Event('scroll'));
    await nextFrame();

    expect(isShown('#diag-card')).toBe(true);

    pre.scrollTop += 40;
    await expect.poll(() => isShown('#diag-card')).toBe(false);
  });
});
