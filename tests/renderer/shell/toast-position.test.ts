import { afterEach, describe, expect, it } from 'vitest';
import type { UiStudio } from '../../../src/renderer/ui-studio.js';
import { deepQueryAll } from '../home/shadow-dom.js';
import { mountStudio, openKitHorizon, shownHome, studioApi } from './shell-harness.js';

const studios: UiStudio[] = [];

function toastRegionBottoms(): string[] {
  return deepQueryAll('.ui-toast-region').map((region) => getComputedStyle(region).bottom);
}

afterEach(() => {
  for (const studio of studios.splice(0)) {
    studio.remove();
  }
});

describe('toast region position', () => {
  // covers: FCT-0014
  it('sits 16 px from the bottom on the home screen and 36 px from it once a project is open', async () => {
    await mountStudio(studioApi(() => Promise.resolve()), studios);
    await expect.poll(() => shownHome()).toBeDefined();
    expect(toastRegionBottoms()).toEqual(['16px', '16px']);

    await openKitHorizon(null);

    expect(toastRegionBottoms()).toEqual(['36px']);
  });
});
