import { describe, expect, it } from 'vitest';
import { removeRecentProjectEntry } from '../../../../../src/modules/project/application/recents/recent-projects-use-cases';
import type { RecentProject } from '../../../../../src/modules/project/domain/recents/recent-projects';
import { InMemoryProjectFileSystem } from '../doubles/in-memory-project-file-system';
import { InMemoryRecentProjectsStore } from '../doubles/in-memory-recent-projects-store';

function datedProject(index: number): RecentProject {
  const day = String(index + 1).padStart(2, '0');
  return { path: `E:\\projets\\lib-${day}`, name: `Lib ${day}`, openedAt: `2026-09-${day}T08:00:00.000Z` };
}

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
});
