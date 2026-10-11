import { describe, expect, it, vi } from 'vitest';
import {
  createProject,
  type CreateProjectRequest,
} from '../../../../../src/modules/project/application/create/create-project';
import type { ProjectDependencies } from '../../../../../src/modules/project/application/open/project-outcomes';
import { ProjectWriteError } from '../../../../../src/modules/project/ports/project-file-system';
import { InMemoryProjectFileSystem } from '../doubles/in-memory-project-file-system';
import { InMemoryRecentProjectsStore } from '../doubles/in-memory-recent-projects-store';

const LOCATION = 'E:\\projets';
const PROJECT_PATH = 'E:\\projets\\kit-horizon';

const kitHorizonRequest: CreateProjectRequest = {
  name: 'Kit Horizon',
  location: LOCATION,
  packageName: '@kit-horizon/ui',
  tagPrefix: 'kh-',
};

function dependenciesOn(
  fileSystem: InMemoryProjectFileSystem,
  recentProjects = new InMemoryRecentProjectsStore(),
): ProjectDependencies {
  return { fileSystem, recentProjects, clock: () => new Date('2026-10-11T09:30:00.000Z') };
}

describe('createProject edge cases', () => {
  // covers: FCT-0003
  it('rejects a location made only of blanks', async () => {
    const fileSystem = new InMemoryProjectFileSystem();

    const outcome = await createProject(dependenciesOn(fileSystem), { ...kitHorizonRequest, location: '  \t ' });

    expect(outcome).toEqual({ kind: 'invalid-request', errors: [{ field: 'location', code: 'empty' }] });
    expect(fileSystem.createFolderCalls).toEqual([]);
  });

  // covers: FCT-0004
  it('never asks the file system to create a folder over a file', async () => {
    const fileSystem = new InMemoryProjectFileSystem().addFolder(LOCATION).addFile(PROJECT_PATH, 'not a folder');

    const outcome = await createProject(dependenciesOn(fileSystem), kitHorizonRequest);

    expect(outcome).toEqual({ kind: 'write-failed', path: PROJECT_PATH, code: 'EEXIST' });
    expect(fileSystem.createFolderCalls).toEqual([]);
  });
});

describe('createProject when the target folder cannot be probed', () => {
  // covers: FCT-0013
  it('reports the code of the write error raised by the probe and writes nothing', async () => {
    const fileSystem = new InMemoryProjectFileSystem().addFolder(LOCATION);
    vi.spyOn(fileSystem, 'readFolderState').mockRejectedValueOnce(new ProjectWriteError(PROJECT_PATH, 'EACCES'));
    const recents = new InMemoryRecentProjectsStore();
    const before = fileSystem.snapshot();

    const outcome = await createProject(dependenciesOn(fileSystem, recents), kitHorizonRequest);

    expect(outcome).toEqual({ kind: 'write-failed', path: PROJECT_PATH, code: 'EACCES' });
    expect(fileSystem.snapshot()).toEqual(before);
    expect(fileSystem.createFolderCalls).toEqual([]);
    expect(recents.saves).toEqual([]);
  });

  // covers: FCT-0013
  it('reports an unexpected probe error with the UNKNOWN code', async () => {
    const fileSystem = new InMemoryProjectFileSystem().addFolder(LOCATION);
    vi.spyOn(fileSystem, 'readFolderState').mockRejectedValueOnce(new TypeError('probe crashed'));

    const outcome = await createProject(dependenciesOn(fileSystem), kitHorizonRequest);

    expect(outcome).toEqual({ kind: 'write-failed', path: PROJECT_PATH, code: 'UNKNOWN' });
    expect(fileSystem.createFolderCalls).toEqual([]);
  });
});
