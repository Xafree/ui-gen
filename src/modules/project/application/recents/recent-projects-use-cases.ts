import {
  keepMostRecent,
  parentFolderOf,
  removeRecentProject,
  type RecentProject,
} from '../../domain/recents/recent-projects.js';
import type { ProjectDependencies } from '../open/project-outcomes.js';

export interface RecentProjectView extends RecentProject {
  readonly available: boolean;
}

export interface RecentProjectsListing {
  readonly projects: RecentProjectView[];
  readonly unreadable: boolean;
}

export type RecentDependencies = Pick<ProjectDependencies, 'fileSystem' | 'recentProjects'>;

async function isAvailable(deps: RecentDependencies, path: string): Promise<boolean> {
  try {
    const state = await deps.fileSystem.readFolderState(path);
    return state.kind === 'empty' || state.kind === 'not-empty';
  } catch {
    return false;
  }
}

export function withAvailability(
  deps: RecentDependencies,
  projects: readonly RecentProject[],
): Promise<RecentProjectView[]> {
  return Promise.all(
    projects.map(async (project) => ({ ...project, available: await isAvailable(deps, project.path) })),
  );
}

export async function listRecentProjects(deps: RecentDependencies): Promise<RecentProjectsListing> {
  const { projects, unreadable } = await deps.recentProjects.load();
  return { projects: await withAvailability(deps, keepMostRecent(projects)), unreadable };
}

export async function removeRecentProjectEntry(deps: RecentDependencies, path: string): Promise<RecentProjectView[]> {
  const { projects } = await deps.recentProjects.load();
  const withoutEntry = removeRecentProject(projects, path);
  const remaining = keepMostRecent(withoutEntry);
  if (withoutEntry.length !== projects.length) await deps.recentProjects.save(remaining);
  return withAvailability(deps, remaining);
}

export async function proposeProjectLocation(deps: RecentDependencies, fallback: string): Promise<string> {
  const { projects } = await listRecentProjects(deps);
  const latestPath = projects.find((project) => project.available)?.path;
  const parent = latestPath === undefined ? null : parentFolderOf(latestPath);
  return parent ?? fallback;
}
