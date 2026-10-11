import { describe, expect, it } from 'vitest';
import { restoreRecentProjects } from '../../../../../src/modules/project/application/recents/forget-recent-projects';
import type { RecentProject } from '../../../../../src/modules/project/domain/recents/recent-projects';
import { InMemoryProjectFileSystem } from '../doubles/in-memory-project-file-system';
import { InMemoryRecentProjectsStore } from '../doubles/in-memory-recent-projects-store';

const horizon: RecentProject = {
  path: 'D:\\work\\kits\\kit-horizon',
  name: 'Kit Horizon',
  openedAt: '2026-10-10T11:00:00.000Z',
};

const atlas: RecentProject = {
  path: 'E:\\clients\\atlas-design-system',
  name: 'Atlas',
  openedAt: '2026-10-09T16:05:12.000Z',
};

describe('restoreRecentProjects', () => {
  // covers: FCT-0023
  it('adds a project once when the request lists it twice, the first copy winning', async () => {
    const renamedAtlas: RecentProject = { ...atlas, name: 'Atlas renommé', openedAt: '2026-10-09T18:00:00.000Z' };
    const recentProjects = new InMemoryRecentProjectsStore([horizon]);

    const restored = await restoreRecentProjects({ fileSystem: new InMemoryProjectFileSystem(), recentProjects }, [
      atlas,
      renamedAtlas,
    ]);

    expect(restored.map((project) => project.name)).toEqual(['Kit Horizon', 'Atlas']);
    expect(recentProjects.current).toEqual([horizon, atlas]);
  });

  // covers: FCT-0023
  it('keeps the list as it is when the request holds nothing', async () => {
    const recentProjects = new InMemoryRecentProjectsStore([horizon, atlas]);

    const restored = await restoreRecentProjects({ fileSystem: new InMemoryProjectFileSystem(), recentProjects }, []);

    expect(restored.map((project) => project.path)).toEqual([horizon.path, atlas.path]);
    expect(recentProjects.current).toEqual([horizon, atlas]);
  });
});
