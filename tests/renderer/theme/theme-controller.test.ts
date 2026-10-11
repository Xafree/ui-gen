import { LitElement, html } from 'lit';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { ThemeController } from '../../../src/renderer/theme/theme-controller.js';
import { applyStoredTheme, setTheme, toggleTheme } from '../../../src/renderer/theme/theme.js';
import { emulateColorScheme, nextFrame, resetThemeEnvironment } from './theme-environment.js';

class ThemeProbe extends LitElement {
  readonly themeController = new ThemeController(this);
  renderCount = 0;

  override render() {
    this.renderCount += 1;
    return html`<output>${this.themeController.theme}</output>`;
  }
}

customElements.define('theme-controller-probe', ThemeProbe);

const mounted: ThemeProbe[] = [];

async function mountProbe(): Promise<ThemeProbe> {
  const probe = document.createElement('theme-controller-probe');
  if (!(probe instanceof ThemeProbe)) {
    throw new TypeError('theme-controller-probe is not a ThemeProbe');
  }
  document.body.append(probe);
  mounted.push(probe);
  await probe.updateComplete;
  return probe;
}

function renderedTheme(probe: ThemeProbe): string | null | undefined {
  return probe.shadowRoot?.querySelector('output')?.textContent;
}

beforeEach(async () => {
  await resetThemeEnvironment();
});

afterEach(async () => {
  for (const probe of mounted.splice(0)) {
    probe.remove();
  }
  await resetThemeEnvironment();
});

describe('ThemeController', () => {
  // covers: FCT-0020
  it.each(['light', 'dark'] as const)(
    'starts with the %s system scheme when no theme was chosen',
    async (scheme) => {
      await emulateColorScheme(scheme);

      const probe = await mountProbe();

      expect(probe.themeController.theme).toBe(scheme);
      expect(renderedTheme(probe)).toBe(scheme);
    },
  );

  // covers: FCT-0017
  it('starts with the stored theme against the system scheme', async () => {
    await emulateColorScheme('light');
    localStorage.setItem('uigen-theme', 'dark');
    applyStoredTheme();

    const probe = await mountProbe();

    expect(probe.themeController.theme).toBe('dark');
    expect(renderedTheme(probe)).toBe('dark');
  });

  // covers: FCT-0020
  it('follows a system scheme change and redraws the host', async () => {
    await emulateColorScheme('light');
    const probe = await mountProbe();

    await emulateColorScheme('dark');

    await expect.poll(() => renderedTheme(probe)).toBe('dark');
    expect(probe.themeController.theme).toBe('dark');

    await emulateColorScheme('light');

    await expect.poll(() => renderedTheme(probe)).toBe('light');
    expect(probe.themeController.theme).toBe('light');
  });

  // covers: FCT-0016
  it('updates and redraws the host as soon as the theme is set', async () => {
    await emulateColorScheme('light');
    const probe = await mountProbe();

    setTheme('dark');

    expect(probe.themeController.theme).toBe('dark');
    expect(probe.isUpdatePending).toBe(true);
    await probe.updateComplete;
    expect(renderedTheme(probe)).toBe('dark');
  });

  // covers: FCT-0016
  it('follows each toggle back and forth', async () => {
    document.documentElement.setAttribute('data-theme', 'light');
    const probe = await mountProbe();

    toggleTheme();
    await probe.updateComplete;
    expect(renderedTheme(probe)).toBe('dark');

    toggleTheme();
    await probe.updateComplete;
    expect(renderedTheme(probe)).toBe('light');
  });

  // covers: FCT-0016
  it('updates every connected host of the studio', async () => {
    document.documentElement.setAttribute('data-theme', 'light');
    const first = await mountProbe();
    const second = await mountProbe();

    setTheme('dark');
    await Promise.all([first.updateComplete, second.updateComplete]);

    expect(renderedTheme(first)).toBe('dark');
    expect(renderedTheme(second)).toBe('dark');
  });

  it('stops redrawing the host on theme events once disconnected', async () => {
    document.documentElement.setAttribute('data-theme', 'light');
    const probe = await mountProbe();
    const rendersBeforeRemoval = probe.renderCount;
    probe.remove();

    setTheme('dark');

    expect(probe.isUpdatePending).toBe(false);
    await nextFrame();
    expect(probe.renderCount).toBe(rendersBeforeRemoval);
    expect(renderedTheme(probe)).toBe('light');
  });

  it('stops redrawing the host on system scheme changes once disconnected', async () => {
    await emulateColorScheme('light');
    const probe = await mountProbe();
    const rendersBeforeRemoval = probe.renderCount;
    probe.remove();

    await emulateColorScheme('dark');
    await nextFrame();

    expect(probe.isUpdatePending).toBe(false);
    expect(probe.renderCount).toBe(rendersBeforeRemoval);
    expect(renderedTheme(probe)).toBe('light');
  });

  it('follows the theme again once the host is reconnected', async () => {
    document.documentElement.setAttribute('data-theme', 'light');
    const probe = await mountProbe();
    probe.remove();
    document.body.append(probe);
    await probe.updateComplete;

    setTheme('dark');
    await probe.updateComplete;

    expect(probe.themeController.theme).toBe('dark');
    expect(renderedTheme(probe)).toBe('dark');
  });
});
