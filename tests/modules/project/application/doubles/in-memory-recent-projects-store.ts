import type { RecentProject } from '../../../../../src/modules/project/domain/recents/recent-projects';
import type {
  RecentProjectsLoad,
  RecentProjectsStore,
} from '../../../../../src/modules/project/ports/recent-projects-store';

export class InMemoryRecentProjectsStore implements RecentProjectsStore {
  readonly saves: (readonly RecentProject[])[] = [];
  private projects: readonly RecentProject[];
  private readonly unreadable: boolean;

  constructor(projects: readonly RecentProject[] = [], unreadable = false) {
    this.projects = projects;
    this.unreadable = unreadable;
  }

  get current(): readonly RecentProject[] {
    return this.projects;
  }

  load(): Promise<RecentProjectsLoad> {
    return Promise.resolve({ projects: this.unreadable ? [] : [...this.projects], unreadable: this.unreadable });
  }

  save(projects: readonly RecentProject[]): Promise<void> {
    this.saves.push([...projects]);
    this.projects = [...projects];
    return Promise.resolve();
  }
}
