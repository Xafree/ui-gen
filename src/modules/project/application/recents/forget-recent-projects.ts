import { findRecentProject, keepMostRecent, type RecentProject } from '../../domain/recents/recent-projects.js';
import { withAvailability, type RecentDependencies, type RecentProjectView } from './recent-projects-use-cases.js';

function withMissingProjects(current: readonly RecentProject[], candidates: readonly RecentProject[]): RecentProject[] {
  const merged = [...current];
  for (const candidate of candidates) {
    if (findRecentProject(merged, candidate.path) === undefined) merged.push(candidate);
  }
  return keepMostRecent(merged);
}

export async function clearRecentProjects(deps: RecentDependencies): Promise<RecentProjectView[]> {
  await deps.recentProjects.save([]);
  return [];
}

export async function restoreRecentProjects(
  deps: RecentDependencies,
  projects: readonly RecentProject[],
): Promise<RecentProjectView[]> {
  const { projects: current } = await deps.recentProjects.load();
  const restored = withMissingProjects(current, projects);
  await deps.recentProjects.save(restored);
  return withAvailability(deps, restored);
}
