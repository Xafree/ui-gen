import { describe, expect, it } from 'vitest';
import {
  clearRecentProjects,
  restoreRecentProjects,
} from '../../../../../src/modules/project/application/recents/forget-recent-projects';
import type { RecentProjectView } from '../../../../../src/modules/project/application/recents/recent-projects-use-cases';
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

const pivot: RecentProject = {
  path: 'D:\\archives\\pivot-ui',
  name: 'Prototype Pivot',
  openedAt: '2026-10-01T08:30:00.000Z',
};

const nova: RecentProject = {
  path: 'C:\\Users\\Camille\\Documents\\nova-ui',
  name: 'Nova UI',
  openedAt: '2026-09-28T07:45:00.000Z',
};

function view(project: RecentProject, available: boolean): RecentProjectView {
  return { path: project.path, name: project.name, openedAt: project.openedAt, available };
}

function datedProject(day: string): RecentProject {
  return { path: `E:\\projets\\lib-${day}`, name: `Lib ${day}`, openedAt: `2026-09-${day}T08:00:00.000Z` };
}

function diskWith(...projects: readonly RecentProject[]): InMemoryProjectFileSystem {
  const fileSystem = new InMemoryProjectFileSystem();
  for (const project of projects) fileSystem.addFolder(project.path);
  return fileSystem;
}

describe('clearRecentProjects', () => {
  // covers: FCT-0022
  it('saves an empty list and returns no project', async () => {
    const fileSystem = diskWith(horizon, atlas, nova);
    const recentProjects = new InMemoryRecentProjectsStore([horizon, atlas, nova]);

    const remaining = await clearRecentProjects({ fileSystem, recentProjects });

    expect(remaining).toEqual([]);
    expect(recentProjects.current).toEqual([]);
    expect(recentProjects.saves).toEqual([[]]);
  });

  // covers: FCT-0022
  it('leaves every project folder untouched', async () => {
    const fileSystem = diskWith(horizon, atlas).addFile(`${horizon.path}\\package.json`, '{ "name": "@horizon/ui" }\n');
    const before = fileSystem.snapshot();

    await clearRecentProjects({ fileSystem, recentProjects: new InMemoryRecentProjectsStore([horizon, atlas]) });

    expect(fileSystem.snapshot()).toEqual(before);
    expect(fileSystem.createFolderCalls).toEqual([]);
    expect(fileSystem.writtenFiles).toEqual([]);
  });
});

describe('restoreRecentProjects', () => {
  // covers: FCT-0022
  it('brings back a cleared list in its order, with the original dates', async () => {
    const fileSystem = diskWith(horizon, atlas, nova);
    const recentProjects = new InMemoryRecentProjectsStore([]);

    const restored = await restoreRecentProjects({ fileSystem, recentProjects }, [horizon, atlas, pivot, nova]);

    expect(restored).toEqual([view(horizon, true), view(atlas, true), view(pivot, false), view(nova, true)]);
    expect(recentProjects.current).toEqual([horizon, atlas, pivot, nova]);
  });

  // covers: FCT-0023
  it('puts a removed project back at its place between the remaining ones', async () => {
    const fileSystem = diskWith(horizon, atlas, nova);
    const recentProjects = new InMemoryRecentProjectsStore([horizon, atlas, nova]);

    const restored = await restoreRecentProjects({ fileSystem, recentProjects }, [pivot]);

    expect(restored).toEqual([view(horizon, true), view(atlas, true), view(pivot, false), view(nova, true)]);
    expect(recentProjects.current).toEqual([horizon, atlas, pivot, nova]);
  });

  // covers: FCT-0023
  it('sorts the restored projects by their opening date, whatever order they are given in', async () => {
    const recentProjects = new InMemoryRecentProjectsStore([atlas]);

    const restored = await restoreRecentProjects(
      { fileSystem: diskWith(horizon, atlas, nova), recentProjects },
      [nova, horizon],
    );

    expect(restored.map((project) => project.name)).toEqual(['Kit Horizon', 'Atlas', 'Nova UI']);
  });

  // covers: FCT-0023
  it('keeps the entry already in the list when the restored one has the same path', async () => {
    const reopenedAtlas: RecentProject = { ...atlas, name: 'Atlas maison', openedAt: '2026-10-11T09:00:00.000Z' };
    const recentProjects = new InMemoryRecentProjectsStore([reopenedAtlas, horizon]);

    const restored = await restoreRecentProjects({ fileSystem: diskWith(horizon, atlas), recentProjects }, [atlas]);

    expect(restored).toEqual([view(reopenedAtlas, true), view(horizon, true)]);
    expect(recentProjects.current).toEqual([reopenedAtlas, horizon]);
  });

  // covers: FCT-0023
  it('treats paths that differ only by letter case as the same project', async () => {
    const upperCaseAtlas: RecentProject = { ...atlas, path: 'E:\\Clients\\Atlas-Design-System' };
    const recentProjects = new InMemoryRecentProjectsStore([horizon, upperCaseAtlas]);

    const restored = await restoreRecentProjects({ fileSystem: diskWith(horizon), recentProjects }, [atlas]);

    expect(restored.map((project) => project.path)).toEqual([horizon.path, 'E:\\Clients\\Atlas-Design-System']);
  });

  // covers: FCT-0022
  it('keeps the 10 most recent projects only', async () => {
    const current = ['20', '19', '18', '17', '16', '15', '14', '13', '12'].map(datedProject);
    const recentProjects = new InMemoryRecentProjectsStore(current);

    const restored = await restoreRecentProjects({ fileSystem: new InMemoryProjectFileSystem(), recentProjects }, [
      datedProject('21'),
      datedProject('11'),
      datedProject('10'),
    ]);

    expect(restored.map((project) => project.name)).toEqual([
      'Lib 21',
      'Lib 20',
      'Lib 19',
      'Lib 18',
      'Lib 17',
      'Lib 16',
      'Lib 15',
      'Lib 14',
      'Lib 13',
      'Lib 12',
    ]);
    expect(recentProjects.current).toHaveLength(10);
  });

  // covers: FCT-0022
  it('never writes in a project folder while restoring', async () => {
    const fileSystem = diskWith(horizon, atlas);
    const before = fileSystem.snapshot();

    await restoreRecentProjects({ fileSystem, recentProjects: new InMemoryRecentProjectsStore([]) }, [horizon, atlas]);

    expect(fileSystem.snapshot()).toEqual(before);
    expect(fileSystem.createFolderCalls).toEqual([]);
    expect(fileSystem.writtenFiles).toEqual([]);
  });
});
