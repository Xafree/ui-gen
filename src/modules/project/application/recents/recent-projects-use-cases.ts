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

type RecentDependencies = Pick<ProjectDependencies, 'fileSystem' | 'recentProjects'>;

function withAvailability(deps: RecentDependencies, projects: readonly RecentProject[]): Promise<RecentProjectView[]> {
  return Promise.all(
    projects.map(async (project) => {
      const state = await deps.fileSystem.readFolderState(project.path);
      return { ...project, available: state.kind === 'empty' || state.kind === 'not-empty' };
    }),
  );
}

export async function listRecentProjects(deps: RecentDependencies): Promise<RecentProjectsListing> {
  const { projects, unreadable } = await deps.recentProjects.load();
  return { projects: await withAvailability(deps, keepMostRecent(projects)), unreadable };
}

export async function removeRecentProjectEntry(deps: RecentDependencies, path: string): Promise<RecentProjectView[]> {
  const { projects } = await deps.recentProjects.load();
  const remaining = keepMostRecent(removeRecentProject(projects, path));
  await deps.recentProjects.save(remaining);
  return withAvailability(deps, remaining);
}

export async function proposeProjectLocation(deps: RecentDependencies, fallback: string): Promise<string> {
  const { projects } = await listRecentProjects(deps);
  const latestPath = projects.find((project) => project.available)?.path;
  const parent = latestPath === undefined ? null : parentFolderOf(latestPath);
  return parent ?? fallback;
}
