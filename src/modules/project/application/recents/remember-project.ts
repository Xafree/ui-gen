import { recordRecentProject } from '../../domain/recents/recent-projects.js';
import type { ProjectDependencies } from '../open/project-outcomes.js';

export async function rememberProject(
  deps: Pick<ProjectDependencies, 'recentProjects' | 'clock'>,
  project: { readonly path: string; readonly name: string },
): Promise<void> {
  const { projects } = await deps.recentProjects.load();
  const entry = { path: project.path, name: project.name, openedAt: deps.clock().toISOString() };
  await deps.recentProjects.save(recordRecentProject(projects, entry));
}
