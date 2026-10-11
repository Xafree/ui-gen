import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  THEME_CHANGE_EVENT,
  THEME_STORAGE_KEY,
  applyStoredTheme,
  currentTheme,
  readStoredTheme,
  setTheme,
  themeToggleView,
  toggleTheme,
} from '../../../src/renderer/theme/theme.js';
import type { Theme, ThemeChangeDetail } from '../../../src/renderer/theme/theme.js';
import { emulateColorScheme, resetThemeEnvironment } from './theme-environment.js';

const listeners: ((event: Event) => void)[] = [];

function documentTheme(): string | null {
  return document.documentElement.getAttribute('data-theme');
}

function recordThemeChanges(): unknown[] {
  const details: unknown[] = [];
  const listener = (event: Event): void => {
    if (event instanceof CustomEvent) {
      const detail: unknown = event.detail;
      details.push(detail);
    }
  };
  document.addEventListener(THEME_CHANGE_EVENT, listener);
  listeners.push(listener);
  return details;
}

function storageFailure(): never {
  throw new DOMException('Storage is disabled', 'SecurityError');
}

beforeEach(async () => {
  await resetThemeEnvironment();
});

afterEach(async () => {
  for (const listener of listeners.splice(0)) {
    document.removeEventListener(THEME_CHANGE_EVENT, listener);
  }
  vi.restoreAllMocks();
  await resetThemeEnvironment();
});

describe('theme constants', () => {
  it('stores the theme under the key used by the mockups', () => {
    expect(THEME_STORAGE_KEY).toBe('uigen-theme');
  });

  it('announces theme changes with the ui:themechange event', () => {
    expect(THEME_CHANGE_EVENT).toBe('ui:themechange');
  });
});

describe('readStoredTheme', () => {
  // covers: FCT-0020
  it('returns null when no theme has ever been chosen', () => {
    expect(readStoredTheme()).toBeNull();
  });

  // covers: FCT-0017
  it.each<Theme>(['light', 'dark'])('returns the stored %s theme', (theme) => {
    localStorage.setItem('uigen-theme', theme);

    expect(readStoredTheme()).toBe(theme);
  });

  it.each(['', 'blue', 'DARK', ' dark', 'null', '{"theme":"dark"}'])(
    'returns null for the invalid stored value %j',
    (value) => {
      localStorage.setItem('uigen-theme', value);

      expect(readStoredTheme()).toBeNull();
    },
  );

  it('returns null when the storage cannot be read', () => {
    localStorage.setItem('uigen-theme', 'dark');
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(storageFailure);

    expect(readStoredTheme()).toBeNull();
  });
});

describe('applyStoredTheme', () => {
  // covers: FCT-0017
  it('opens the studio with the stored theme, even against the system scheme', async () => {
    await emulateColorScheme('light');
    localStorage.setItem('uigen-theme', 'dark');

    applyStoredTheme();

    expect(documentTheme()).toBe('dark');
    expect(currentTheme()).toBe('dark');
  });

  // covers: FCT-0017
  it('applies a stored light theme while the system is dark', async () => {
    await emulateColorScheme('dark');
    localStorage.setItem('uigen-theme', 'light');

    applyStoredTheme();

    expect(documentTheme()).toBe('light');
    expect(currentTheme()).toBe('light');
  });

  // covers: FCT-0020
  it('leaves data-theme unset when no theme has been chosen', () => {
    applyStoredTheme();

    expect(document.documentElement.hasAttribute('data-theme')).toBe(false);
  });

  it('leaves data-theme unset when the stored value is invalid', () => {
    localStorage.setItem('uigen-theme', 'sepia');

    applyStoredTheme();

    expect(document.documentElement.hasAttribute('data-theme')).toBe(false);
  });

  it('leaves data-theme unset when the storage cannot be read', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(storageFailure);

    expect(() => {
      applyStoredTheme();
    }).not.toThrow();
    expect(document.documentElement.hasAttribute('data-theme')).toBe(false);
  });
});

describe('currentTheme', () => {
  // covers: FCT-0020
  it.each<Theme>(['light', 'dark'])(
    'follows the %s system scheme when no theme is set',
    async (scheme) => {
      await emulateColorScheme(scheme);

      expect(currentTheme()).toBe(scheme);
    },
  );

  // covers: FCT-0020
  it('changes with the system scheme while no theme is set', async () => {
    await emulateColorScheme('dark');
    expect(currentTheme()).toBe('dark');

    await emulateColorScheme('light');
    expect(currentTheme()).toBe('light');
  });

  it.each<[Theme, Theme]>([
    ['light', 'dark'],
    ['dark', 'light'],
  ])('returns the %s data-theme over a %s system scheme', async (theme, scheme) => {
    await emulateColorScheme(scheme);
    document.documentElement.setAttribute('data-theme', theme);

    expect(currentTheme()).toBe(theme);
  });

  it('falls back to the system scheme when data-theme is invalid', async () => {
    await emulateColorScheme('dark');
    document.documentElement.setAttribute('data-theme', 'sepia');

    expect(currentTheme()).toBe('dark');
  });
});

describe('setTheme', () => {
  // covers: FCT-0016
  it('applies the theme to the whole document immediately', () => {
    setTheme('dark');

    expect(documentTheme()).toBe('dark');
    expect(currentTheme()).toBe('dark');

    setTheme('light');

    expect(documentTheme()).toBe('light');
    expect(currentTheme()).toBe('light');
  });

  // covers: FCT-0017
  it('remembers the chosen theme', () => {
    setTheme('dark');

    expect(localStorage.getItem('uigen-theme')).toBe('dark');
    expect(readStoredTheme()).toBe('dark');

    setTheme('light');

    expect(localStorage.getItem('uigen-theme')).toBe('light');
  });

  // covers: FCT-0016
  it('dispatches ui:themechange on document with the new theme', () => {
    const changes = recordThemeChanges();

    setTheme('dark');

    expect(changes).toEqual([{ theme: 'dark' } satisfies ThemeChangeDetail]);
  });

  it('dispatches ui:themechange after data-theme is applied', () => {
    const seen: (string | null)[] = [];
    const listener = (): void => {
      seen.push(documentTheme());
    };
    document.addEventListener('ui:themechange', listener);
    listeners.push(listener);

    setTheme('dark');

    expect(seen).toEqual(['dark']);
  });

  it('still applies and announces the theme when the storage cannot be written', () => {
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(storageFailure);
    const changes = recordThemeChanges();

    expect(() => {
      setTheme('dark');
    }).not.toThrow();
    expect(documentTheme()).toBe('dark');
    expect(changes).toEqual([{ theme: 'dark' }]);
  });
});

describe('toggleTheme', () => {
  // covers: FCT-0016
  it('switches from light to dark and returns the new theme', () => {
    document.documentElement.setAttribute('data-theme', 'light');

    expect(toggleTheme()).toBe('dark');
    expect(documentTheme()).toBe('dark');
    expect(localStorage.getItem('uigen-theme')).toBe('dark');
  });

  // covers: FCT-0016
  it('switches from dark to light and returns the new theme', () => {
    document.documentElement.setAttribute('data-theme', 'dark');

    expect(toggleTheme()).toBe('light');
    expect(documentTheme()).toBe('light');
    expect(localStorage.getItem('uigen-theme')).toBe('light');
  });

  // covers: FCT-0016
  it.each<[Theme, Theme]>([
    ['dark', 'light'],
    ['light', 'dark'],
  ])(
    'switches away from the %s system scheme when no theme is set',
    async (scheme, expected) => {
      await emulateColorScheme(scheme);

      expect(toggleTheme()).toBe(expected);
      expect(documentTheme()).toBe(expected);
    },
  );

  // covers: FCT-0016
  it('announces each switch with ui:themechange', () => {
    document.documentElement.setAttribute('data-theme', 'light');
    const changes = recordThemeChanges();

    toggleTheme();
    toggleTheme();

    expect(changes).toEqual([{ theme: 'dark' }, { theme: 'light' }]);
  });

  // covers: FCT-0017
  it('keeps the last switched theme for the next launch', async () => {
    await emulateColorScheme('light');
    document.documentElement.setAttribute('data-theme', 'light');
    toggleTheme();
    document.documentElement.removeAttribute('data-theme');

    applyStoredTheme();

    expect(currentTheme()).toBe('dark');
  });
});

describe('themeToggleView', () => {
  // covers: FCT-0016
  it('offers the dark theme from the light theme', () => {
    expect(themeToggleView('light')).toEqual({
      label: 'Passer au thème sombre',
      tip: 'Thème sombre',
      icon: 'lune',
    });
  });

  // covers: FCT-0016
  it('offers the light theme from the dark theme', () => {
    expect(themeToggleView('dark')).toEqual({
      label: 'Passer au thème clair',
      tip: 'Thème clair',
      icon: 'soleil',
    });
  });
});
