import { findRecentProject } from '../../domain/recents/recent-projects.js';
import { openProjectFolder } from './open-project-folder.js';
import type { OpenProjectOutcome, ProjectDependencies } from './project-outcomes.js';

export async function openRecentProject(deps: ProjectDependencies, path: string): Promise<OpenProjectOutcome> {
  const { projects } = await deps.recentProjects.load();
  if (findRecentProject(projects, path) === undefined) {
    return { kind: 'invalid-request', errors: [{ field: 'path', code: 'not-granted' }] };
  }
  return openProjectFolder(deps, path);
}
