import type { RecentProject } from '../domain/recents/recent-projects.js';

export interface RecentProjectsLoad {
  readonly projects: RecentProject[];
  readonly unreadable: boolean;
}

export interface RecentProjectsStore {
  load(): Promise<RecentProjectsLoad>;
  save(projects: readonly RecentProject[]): Promise<void>;
}
