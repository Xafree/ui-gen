export type Theme = 'light' | 'dark';

export interface ThemeChangeDetail {
  readonly theme: Theme;
}

export const THEME_STORAGE_KEY = 'uigen-theme';
export const THEME_CHANGE_EVENT = 'ui:themechange';

const DARK_SCHEME = '(prefers-color-scheme: dark)';

function isTheme(value: unknown): value is Theme {
  return value === 'light' || value === 'dark';
}

export function readStoredTheme(): Theme | null {
  try {
    const stored = localStorage.getItem(THEME_STORAGE_KEY);
    return isTheme(stored) ? stored : null;
  } catch {
    return null;
  }
}

export function applyStoredTheme(): void {
  const stored = readStoredTheme();
  if (stored !== null) document.documentElement.dataset['theme'] = stored;
}

export function currentTheme(): Theme {
  const applied = document.documentElement.dataset['theme'];
  if (isTheme(applied)) return applied;
  return matchMedia(DARK_SCHEME).matches ? 'dark' : 'light';
}

export function setTheme(theme: Theme): void {
  document.documentElement.dataset['theme'] = theme;
  try {
    localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch (error) {
    console.warn('Theme preference not saved', error);
  }
  document.dispatchEvent(new CustomEvent<ThemeChangeDetail>(THEME_CHANGE_EVENT, { detail: { theme } }));
}

export function toggleTheme(): Theme {
  const next: Theme = currentTheme() === 'dark' ? 'light' : 'dark';
  setTheme(next);
  return next;
}

export function themeToggleView(theme: Theme): {
  readonly label: string;
  readonly tip: string;
  readonly icon: 'lune' | 'soleil';
} {
  return theme === 'dark'
    ? { label: 'Passer au thème clair', tip: 'Thème clair', icon: 'soleil' }
    : { label: 'Passer au thème sombre', tip: 'Thème sombre', icon: 'lune' };
}
