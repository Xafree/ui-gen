import { page } from 'vitest/browser';
import { vi, type Mock } from 'vitest';
import type { RecentProjectsListing } from '../../../../src/modules/project/application/recents/recent-projects-use-cases.js';
import type { RecentProject } from '../../../../src/modules/project/domain/recents/recent-projects.js';
import type { UiGenApi } from '../../../../src/preload/ui-gen-api.js';
import { UiHome } from '../../../../src/renderer/home/ui-home.js';
import type { RecentsApi } from '../home-fixtures.js';
import { deepQueryAll, removeShownToasts, visibleText } from '../shadow-dom.js';

const mounted: UiHome[] = [];

export async function mountHome(api: UiGenApi): Promise<UiHome> {
  const home = new UiHome();
  home.api = api;
  home.notice = null;
  document.body.append(home);
  mounted.push(home);
  await home.updateComplete;
  return home;
}

export function unmountHomes(): void {
  for (const home of mounted.splice(0)) {
    home.remove();
  }
  removeShownToasts();
}

export function listOf(result: RecentProjectsListing): RecentsApi['list'] {
  return vi.fn<RecentsApi['list']>(() => Promise.resolve(result));
}

export function recentSection() {
  return page.getByRole('region', { name: 'Projets récents' });
}

export function recentLinks() {
  return recentSection().getByRole('link');
}

export function recentLinkTexts(): string[] {
  return recentLinks()
    .elements()
    .map((link) => visibleText(link));
}

export function removeButtonOf(name: string) {
  return page.getByRole('button', { name: `Retirer de la liste : ${name}` });
}

function undoButtonOf(title: string): HTMLButtonElement | null {
  const toast = deepQueryAll('.ui-toast').find(
    (candidate) => visibleText(candidate.querySelector('.ui-toast__title')) === title,
  );
  const button = toast?.querySelector('.ui-toast__actions button');
  return button instanceof HTMLButtonElement && visibleText(button) === 'Annuler' ? button : null;
}

export async function hasUndo(title: string): Promise<boolean> {
  await vi.waitFor(() => {
    if (undoButtonOf(title) === null) throw new Error(`No « Annuler » in the toast « ${title} »`);
  });
  return true;
}

export async function undo(title: string): Promise<void> {
  await hasUndo(title);
  undoButtonOf(title)?.click();
}

export function sentProjects(restore: Mock<RecentsApi['restore']>, call = 0): RecentProject[] {
  const projects = restore.mock.calls[call]?.[0] ?? [];
  return projects.map(({ path, name, openedAt }) => ({ path, name, openedAt }));
}
