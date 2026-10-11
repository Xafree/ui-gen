import { cdp } from 'vitest/browser';
import type {} from '@vitest/browser-playwright';

export type ColorScheme = 'light' | 'dark';

export async function emulateColorScheme(scheme: ColorScheme): Promise<void> {
  await cdp().send('Emulation.setEmulatedMedia', {
    features: [{ name: 'prefers-color-scheme', value: scheme }],
  });
}

export async function resetThemeEnvironment(): Promise<void> {
  await cdp().send('Emulation.setEmulatedMedia', { features: [] });
  document.documentElement.removeAttribute('data-theme');
  localStorage.clear();
}

export function nextFrame(): Promise<void> {
  return new Promise((resolve) => {
    requestAnimationFrame(() => {
      setTimeout(resolve, 0);
    });
  });
}
