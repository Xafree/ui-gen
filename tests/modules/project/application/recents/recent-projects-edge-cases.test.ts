import { describe, expect, it, vi } from 'vitest';
import {
  listRecentProjects,
  proposeProjectLocation,
  removeRecentProjectEntry,
  type RecentProjectView,
} from '../../../../../src/modules/project/application/recents/recent-projects-use-cases';
import type { RecentProject } from '../../../../../src/modules/project/domain/recents/recent-projects';
import { InMemoryProjectFileSystem } from '../doubles/in-memory-project-file-system';
import { InMemoryRecentProjectsStore } from '../doubles/in-memory-recent-projects-store';

const DOCUMENTS = 'C:\\Users\\Camille\\Documents';

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

const nova: RecentProject = {
  path: 'C:\\Users\\Camille\\Documents\\nova-ui',
  name: 'Nova UI',
  openedAt: '2026-09-28T07:45:00.000Z',
};

function view(project: RecentProject, available: boolean): RecentProjectView {
  return { path: project.path, name: project.name, openedAt: project.openedAt, available };
}

function datedProject(index: number): RecentProject {
  const day = String(index + 1).padStart(2, '0');
  return { path: `E:\\projets\\lib-${day}`, name: `Lib ${day}`, openedAt: `2026-09-${day}T08:00:00.000Z` };
}

describe('listRecentProjects', () => {
  // covers: FCT-0011
  it('lists every project and marks the one whose folder probe fails as unavailable', async () => {
    const fileSystem = new InMemoryProjectFileSystem()
      .addFolder(horizon.path)
      .addFolder(atlas.path)
      .addFolder(nova.path);
    vi.spyOn(fileSystem, 'readFolderState').mockRejectedValueOnce(new Error('EACCES'));
    const recentProjects = new InMemoryRecentProjectsStore([nova, atlas, horizon]);

    const listing = await listRecentProjects({ fileSystem, recentProjects });

    expect(listing).toEqual({
      projects: [view(horizon, false), view(atlas, true), view(nova, true)],
      unreadable: false,
    });
  });
});

describe('proposeProjectLocation', () => {
  // covers: FCT-0011
  it('skips a recent project whose folder probe fails', async () => {
    const fileSystem = new InMemoryProjectFileSystem().addFolder(horizon.path).addFolder(atlas.path);
    vi.spyOn(fileSystem, 'readFolderState').mockRejectedValueOnce(new Error('EACCES'));
    const recentProjects = new InMemoryRecentProjectsStore([horizon, atlas]);

    const location = await proposeProjectLocation({ fileSystem, recentProjects }, DOCUMENTS);

    expect(location).toBe('E:\\clients');
  });
});

describe('removeRecentProjectEntry', () => {
  // covers: FCT-0010
  it('returns and saves the remaining projects most recent first, 10 at most', async () => {
    const twelve = Array.from({ length: 12 }, (_, index) => datedProject(index));
    const recentProjects = new InMemoryRecentProjectsStore(twelve);
    const expectedNames = [
      'Lib 12',
      'Lib 11',
      'Lib 10',
      'Lib 09',
      'Lib 08',
      'Lib 07',
      'Lib 06',
      'Lib 04',
      'Lib 03',
      'Lib 02',
    ];

    const remaining = await removeRecentProjectEntry(
      { fileSystem: new InMemoryProjectFileSystem(), recentProjects },
      'E:\\projets\\lib-05',
    );

    expect(remaining.map((project) => project.name)).toEqual(expectedNames);
    expect(recentProjects.current.map((project) => project.name)).toEqual(expectedNames);
  });

  // covers: FCT-0011
  it('removes the entry and returns the list even when a folder probe fails', async () => {
    const fileSystem = new InMemoryProjectFileSystem()
      .addFolder(horizon.path)
      .addFolder(atlas.path)
      .addFolder(nova.path);
    vi.spyOn(fileSystem, 'readFolderState').mockRejectedValueOnce(new Error('EACCES'));
    const recentProjects = new InMemoryRecentProjectsStore([horizon, atlas, nova]);

    const remaining = await removeRecentProjectEntry({ fileSystem, recentProjects }, nova.path);

    expect(remaining).toEqual([view(horizon, false), view(atlas, true)]);
    expect(recentProjects.current).toEqual([horizon, atlas]);
  });

  // covers: FCT-0011
  it('saves nothing when the path is not in the list', async () => {
    const fileSystem = new InMemoryProjectFileSystem().addFolder(atlas.path).addFolder(nova.path);
    const recentProjects = new InMemoryRecentProjectsStore([atlas, nova]);

    const remaining = await removeRecentProjectEntry({ fileSystem, recentProjects }, horizon.path);

    expect(remaining).toEqual([view(atlas, true), view(nova, true)]);
    expect(recentProjects.saves).toEqual([]);
  });

  it('never overwrites an unreadable list', async () => {
    const fileSystem = new InMemoryProjectFileSystem().addFolder(atlas.path);
    const recentProjects = new InMemoryRecentProjectsStore([atlas], true);

    const remaining = await removeRecentProjectEntry({ fileSystem, recentProjects }, atlas.path);

    expect(remaining).toEqual([]);
    expect(recentProjects.saves).toEqual([]);
    expect(recentProjects.current).toEqual([atlas]);
  });
});
