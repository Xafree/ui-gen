import { describe, expect, it } from 'vitest';
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

const horizon: RecentProject = {
  path: 'D:\\work\\kits\\kit-horizon',
  name: 'Kit Horizon',
  openedAt: '2026-10-10T11:00:00.000Z',
};

function view(project: RecentProject, available: boolean): RecentProjectView {
  return { path: project.path, name: project.name, openedAt: project.openedAt, available };
}

function datedProject(index: number): RecentProject {
  const day = String(index + 1).padStart(2, '0');
  return { path: `E:\\projets\\lib-${day}`, name: `Lib ${day}`, openedAt: `2026-09-${day}T08:00:00.000Z` };
}

describe('listRecentProjects', () => {
  // covers: FCT-0010
  it('lists the recent projects from the most recent to the oldest', async () => {
    const fileSystem = new InMemoryProjectFileSystem().addFolder(atlas.path).addFolder(nova.path).addFolder(horizon.path);
    const recentProjects = new InMemoryRecentProjectsStore([nova, horizon, atlas]);

    const listing = await listRecentProjects({ fileSystem, recentProjects });

    expect(listing).toEqual({
      projects: [view(horizon, true), view(atlas, true), view(nova, true)],
      unreadable: false,
    });
  });

  // covers: FCT-0010
  it('keeps the 10 most recent projects only', async () => {
    const twelve = Array.from({ length: 12 }, (_, index) => datedProject(index));
    const fileSystem = new InMemoryProjectFileSystem();
    for (const project of twelve) fileSystem.addFolder(project.path);
    const recentProjects = new InMemoryRecentProjectsStore(twelve);

    const listing = await listRecentProjects({ fileSystem, recentProjects });

    expect(listing.projects.map((project) => project.name)).toEqual([
      'Lib 12',
      'Lib 11',
      'Lib 10',
      'Lib 09',
      'Lib 08',
      'Lib 07',
      'Lib 06',
      'Lib 05',
      'Lib 04',
      'Lib 03',
    ]);
  });

  // covers: FCT-0010
  it('returns an empty readable listing when no project was ever opened', async () => {
    const listing = await listRecentProjects({
      fileSystem: new InMemoryProjectFileSystem(),
      recentProjects: new InMemoryRecentProjectsStore(),
    });

    expect(listing).toEqual({ projects: [], unreadable: false });
  });

  // covers: FCT-0010
  it('reports an unreadable list with no project and writes nothing', async () => {
    const fileSystem = new InMemoryProjectFileSystem().addFolder(atlas.path);
    const recentProjects = new InMemoryRecentProjectsStore([atlas], true);

    const listing = await listRecentProjects({ fileSystem, recentProjects });

    expect(listing).toEqual({ projects: [], unreadable: true });
    expect(recentProjects.saves).toEqual([]);
    expect(fileSystem.writtenFiles).toEqual([]);
  });

  // covers: FCT-0011
  it('marks a project whose folder was moved or deleted as unavailable', async () => {
    const fileSystem = new InMemoryProjectFileSystem().addFolder(horizon.path).addFolder(nova.path);
    const recentProjects = new InMemoryRecentProjectsStore([horizon, atlas, nova]);

    const listing = await listRecentProjects({ fileSystem, recentProjects });

    expect(listing.projects).toEqual([view(horizon, true), view(atlas, false), view(nova, true)]);
  });

  // covers: FCT-0011
  it('marks a project whose path is now a file as unavailable', async () => {
    const fileSystem = new InMemoryProjectFileSystem().addFile(atlas.path, 'not a folder');
    const recentProjects = new InMemoryRecentProjectsStore([atlas]);

    const listing = await listRecentProjects({ fileSystem, recentProjects });

    expect(listing.projects).toEqual([view(atlas, false)]);
  });

  // covers: FCT-0011
  it('treats an empty project folder as available', async () => {
    const fileSystem = new InMemoryProjectFileSystem().addFolder(atlas.path);
    const recentProjects = new InMemoryRecentProjectsStore([atlas]);

    const listing = await listRecentProjects({ fileSystem, recentProjects });

    expect(listing.projects).toEqual([view(atlas, true)]);
  });

  // covers: FCT-0011
  it('treats a project folder with content as available', async () => {
    const fileSystem = new InMemoryProjectFileSystem()
      .addFolder(atlas.path)
      .addFile(`${atlas.path}\\package.json`, '{}');
    const recentProjects = new InMemoryRecentProjectsStore([atlas]);

    const listing = await listRecentProjects({ fileSystem, recentProjects });

    expect(listing.projects).toEqual([view(atlas, true)]);
  });
});

describe('removeRecentProjectEntry', () => {
  // covers: FCT-0011
  it('removes the entry, saves the new list and returns it', async () => {
    const fileSystem = new InMemoryProjectFileSystem().addFolder(horizon.path).addFolder(nova.path);
    const recentProjects = new InMemoryRecentProjectsStore([horizon, atlas, nova]);

    const remaining = await removeRecentProjectEntry({ fileSystem, recentProjects }, atlas.path);

    expect(remaining).toEqual([view(horizon, true), view(nova, true)]);
    expect(recentProjects.saves).toEqual([[horizon, nova]]);
    expect(recentProjects.current).toEqual([horizon, nova]);
  });

  // covers: FCT-0011
  it('never touches the folder of the removed project', async () => {
    const fileSystem = new InMemoryProjectFileSystem()
      .addFolder(atlas.path)
      .addFile(`${atlas.path}\\package.json`, '{"name":"atlas"}')
      .addFolder(nova.path);
    const before = fileSystem.snapshot();
    const recentProjects = new InMemoryRecentProjectsStore([atlas, nova]);

    const remaining = await removeRecentProjectEntry({ fileSystem, recentProjects }, atlas.path);

    expect(remaining).toEqual([view(nova, true)]);
    expect(fileSystem.snapshot()).toEqual(before);
    expect(fileSystem.createFolderCalls).toEqual([]);
    expect(fileSystem.writtenFiles).toEqual([]);
  });

  // covers: FCT-0011
  it('returns the remaining projects with their availability', async () => {
    const fileSystem = new InMemoryProjectFileSystem().addFolder(horizon.path);
    const recentProjects = new InMemoryRecentProjectsStore([horizon, atlas, nova]);

    const remaining = await removeRecentProjectEntry({ fileSystem, recentProjects }, horizon.path);

    expect(remaining).toEqual([view(atlas, false), view(nova, false)]);
  });

  // covers: FCT-0011
  it('leaves an empty list when the last entry is removed', async () => {
    const recentProjects = new InMemoryRecentProjectsStore([atlas]);

    const remaining = await removeRecentProjectEntry(
      { fileSystem: new InMemoryProjectFileSystem(), recentProjects },
      atlas.path,
    );

    expect(remaining).toEqual([]);
    expect(recentProjects.current).toEqual([]);
  });

  // covers: FCT-0011
  it('keeps the list unchanged when the path is not in it', async () => {
    const fileSystem = new InMemoryProjectFileSystem().addFolder(atlas.path).addFolder(nova.path);
    const recentProjects = new InMemoryRecentProjectsStore([atlas, nova]);

    const remaining = await removeRecentProjectEntry({ fileSystem, recentProjects }, horizon.path);

    expect(remaining).toEqual([view(atlas, true), view(nova, true)]);
    expect(recentProjects.current).toEqual([atlas, nova]);
  });
});

describe('proposeProjectLocation', () => {
  // covers: FCT-0010
  it('proposes the parent folder of the most recent project', async () => {
    const fileSystem = new InMemoryProjectFileSystem().addFolder(horizon.path).addFolder(atlas.path);
    const recentProjects = new InMemoryRecentProjectsStore([atlas, horizon]);

    const location = await proposeProjectLocation({ fileSystem, recentProjects }, DOCUMENTS);

    expect(location).toBe('D:\\work\\kits');
  });

  // covers: FCT-0011
  it('skips recent projects whose folder is gone', async () => {
    const fileSystem = new InMemoryProjectFileSystem().addFolder(nova.path);
    const recentProjects = new InMemoryRecentProjectsStore([horizon, atlas, nova]);

    const location = await proposeProjectLocation({ fileSystem, recentProjects }, 'D:\\fallback');

    expect(location).toBe('C:\\Users\\Camille\\Documents');
  });

  // covers: FCT-0011
  it('falls back when no recent project is available', async () => {
    const recentProjects = new InMemoryRecentProjectsStore([horizon, atlas]);

    const location = await proposeProjectLocation(
      { fileSystem: new InMemoryProjectFileSystem().addFile(horizon.path, 'file'), recentProjects },
      DOCUMENTS,
    );

    expect(location).toBe(DOCUMENTS);
  });

  // covers: FCT-0010
  it('falls back when there is no recent project', async () => {
    const location = await proposeProjectLocation(
      { fileSystem: new InMemoryProjectFileSystem(), recentProjects: new InMemoryRecentProjectsStore() },
      DOCUMENTS,
    );

    expect(location).toBe(DOCUMENTS);
  });

  // covers: FCT-0010
  it('falls back when the recent list is unreadable', async () => {
    const fileSystem = new InMemoryProjectFileSystem().addFolder(horizon.path);
    const recentProjects = new InMemoryRecentProjectsStore([horizon], true);

    const location = await proposeProjectLocation({ fileSystem, recentProjects }, DOCUMENTS);

    expect(location).toBe(DOCUMENTS);
  });

  // covers: FCT-0010
  it('falls back when the most recent available project sits at a drive root', async () => {
    const rootProject: RecentProject = { path: 'F:\\atlas', name: 'Atlas', openedAt: '2026-10-10T11:00:00.000Z' };
    const fileSystem = new InMemoryProjectFileSystem().addFolder(rootProject.path);
    const recentProjects = new InMemoryRecentProjectsStore([rootProject]);

    const location = await proposeProjectLocation({ fileSystem, recentProjects }, DOCUMENTS);

    expect(location).toBe(DOCUMENTS);
  });

  // covers: FCT-0010
  it('writes nothing', async () => {
    const fileSystem = new InMemoryProjectFileSystem().addFolder(horizon.path);
    const before = fileSystem.snapshot();
    const recentProjects = new InMemoryRecentProjectsStore([horizon, atlas]);

    await proposeProjectLocation({ fileSystem, recentProjects }, DOCUMENTS);

    expect(recentProjects.saves).toEqual([]);
    expect(fileSystem.snapshot()).toEqual(before);
    expect(fileSystem.createFolderCalls).toEqual([]);
    expect(fileSystem.writtenFiles).toEqual([]);
  });
});
