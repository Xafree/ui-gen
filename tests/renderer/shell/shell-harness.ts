import { page, type Locator } from 'vitest/browser';
import { expect } from 'vitest';
import type { UiGenApi } from '../../../src/preload/ui-gen-api.js';
import type { ToastRequest } from '../../../src/renderer/motion/toast.js';
import { PROJECT_OPENED, type ProjectOpenedDetail } from '../../../src/renderer/studio-events.js';
import { UiStudio } from '../../../src/renderer/ui-studio.js';
import { kitHorizon, kitHorizonProject, listing, lumenUi, makeUiGenApi, type RecentsApi } from '../home/home-fixtures.js';
import { deepActiveElement, deepQueryAll, visibleText } from '../home/shadow-dom.js';

export type ProjectApi = UiGenApi['project'];

export const PENDING_CHAT = {
  tone: 'info',
  title: 'Disponible dans un prochain lot',
  message: 'Le Chat IA arrive avec le lot 6.',
};

export function shownHome(): Element | undefined {
  return deepQueryAll('ui-home')[0];
}

export function shownShell(): Element | undefined {
  return deepQueryAll('ui-shell')[0];
}

export function projectButton(): Locator {
  return page.getByRole('button', { name: /^Projet :/ });
}

export function studioNav(): Locator {
  return page.getByRole('navigation', { name: 'Écrans du studio' });
}

export function navLink(label: string): Locator {
  return studioNav().getByRole('link', { name: label, exact: true });
}

export function screensButton(): Locator {
  return page.getByRole('button', { name: /^Écran actuel :/ });
}

export function emptyStateText(): string {
  return visibleText(deepQueryAll('#av-text')[0] ?? null);
}

export function focusOn(locator: Locator): void {
  const element = locator.element();
  if (!(element instanceof HTMLElement)) throw new Error('expected a focusable HTML element');
  element.focus();
}

export function isFocused(locator: Locator): boolean {
  return deepActiveElement() === locator.element();
}

export function notificationRegions(): Element[] {
  return deepQueryAll('[aria-live]').filter((region) => region.getAttribute('aria-label') === 'Notifications');
}

export function recentsList(): RecentsApi['list'] {
  return () => Promise.resolve(listing([kitHorizon(), lumenUi()]));
}

export function studioApi(close: ProjectApi['close']): UiGenApi {
  const api = makeUiGenApi({ list: recentsList() });
  return { ...api, project: { ...api.project, close } };
}

export async function mountStudio(api: UiGenApi, mounted: UiStudio[]): Promise<UiStudio> {
  const studio = new UiStudio();
  studio.api = api;
  document.body.append(studio);
  mounted.push(studio);
  await studio.updateComplete;
  return studio;
}

export async function openKitHorizon(notice: ToastRequest | null): Promise<void> {
  await expect.poll(() => shownHome()).toBeDefined();
  const detail: ProjectOpenedDetail = { project: kitHorizonProject(), notice };
  shownHome()?.dispatchEvent(
    new CustomEvent<ProjectOpenedDetail>(PROJECT_OPENED, { detail, bubbles: true, composed: true }),
  );
  await expect.poll(() => shownShell()).toBeDefined();
  await expect.element(projectButton()).toBeVisible();
}
