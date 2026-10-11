import { describe, expect, it } from 'vitest';
import { rememberProject } from '../../../../../src/modules/project/application/recents/remember-project';
import type { RecentProject } from '../../../../../src/modules/project/domain/recents/recent-projects';
import { InMemoryRecentProjectsStore } from '../doubles/in-memory-recent-projects-store';

const NOW = '2026-10-11T09:30:00.000Z';

const atlas: RecentProject = {
  path: 'E:\\clients\\atlas-design-system',
  name: 'Atlas',
  openedAt: '2026-10-09T16:05:12.000Z',
};

const nova: RecentProject = {
  path: 'C:\\Users\\Camille\\Documents\\nova-ui',
  name: 'Nova UI',
  openedAt: '2026-09-28T07:45:00.000Z',
};

function remember(store: InMemoryRecentProjectsStore, project: { path: string; name: string }): Promise<void> {
  return rememberProject({ recentProjects: store, clock: () => new Date(NOW) }, project);
}

describe('rememberProject', () => {
  // covers: FCT-0010
  it('puts the project first, dated with the clock as an ISO string', async () => {
    const store = new InMemoryRecentProjectsStore([atlas, nova]);

    await remember(store, { path: 'E:\\projets\\kit-horizon', name: 'Kit Horizon' });

    expect(store.current).toEqual([
      { path: 'E:\\projets\\kit-horizon', name: 'Kit Horizon', openedAt: NOW },
      atlas,
      nova,
    ]);
    expect(store.saves).toHaveLength(1);
  });

  // covers: FCT-0010
  it('lists a project that is remembered again once, at the top', async () => {
    const store = new InMemoryRecentProjectsStore([atlas, nova]);

    await remember(store, { path: 'e:\\Clients\\Atlas-Design-System', name: 'Atlas' });

    expect(store.current).toEqual([
      { path: 'e:\\Clients\\Atlas-Design-System', name: 'Atlas', openedAt: NOW },
      nova,
    ]);
  });
});
