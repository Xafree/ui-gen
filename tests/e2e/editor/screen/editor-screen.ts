import { expect, type Locator, type Page } from '@playwright/test';
import { writeFiles, writeRecents } from '../../project/studio-app';

const TSCONFIG = `{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "experimentalDecorators": true,
    "useDefineForClassFields": false,
    "strict": true
  },
  "include": ["src"]
}
`;

export async function writeTsconfig(project: string): Promise<void> {
  await writeFiles(project, { 'tsconfig.json': TSCONFIG });
}

export async function rememberRecentProject(userData: string, project: string): Promise<void> {
  await writeRecents(userData, [{ path: project, name: 'Kit Horizon', openedAt: new Date().toISOString() }]);
}

export function frameName(window: Page): Locator {
  return window.locator('#frame-name');
}

export function componentsTab(window: Page): Locator {
  return window.getByRole('tab', { name: /^Composants/ });
}

export function componentOption(window: Page, name: string): Locator {
  return window.locator('#comp-list [role="option"]').filter({ has: window.locator('.ed-comp__name', { hasText: name }) });
}

export function openedMark(window: Page): Locator {
  return window.locator('#comp-list [role="option"]').filter({ has: window.locator('.ed-comp__mark', { hasText: 'Ouvert' }) });
}

export async function expectEditorOn(window: Page, tag: string): Promise<void> {
  await expect(window.locator('ui-editor')).toBeVisible();
  await expect(frameName(window)).toHaveText(tag);
  await expect(window.locator('#page-title')).toHaveText(`Éditeur du composant ${tag}`);
}
