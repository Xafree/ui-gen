import { describe, expect, it } from 'vitest';
import {
  createProject,
  type CreateProjectOutcome,
  type CreateProjectRequest,
} from '../../../../../src/modules/project/application/create/create-project';
import type { ProjectDependencies } from '../../../../../src/modules/project/application/open/project-outcomes';
import { createLibraryManifest } from '../../../../../src/modules/project/domain/library/library-manifest';
import type { RecentProject } from '../../../../../src/modules/project/domain/recents/recent-projects';
import { ProjectWriteError } from '../../../../../src/modules/project/ports/project-file-system';
import { InMemoryProjectFileSystem } from '../doubles/in-memory-project-file-system';
import { InMemoryRecentProjectsStore } from '../doubles/in-memory-recent-projects-store';

const LOCATION = 'E:\\projets';
const PROJECT_PATH = 'E:\\projets\\kit-horizon';
const NOW = new Date('2026-10-11T09:30:00.000Z');

const kitHorizonRequest: CreateProjectRequest = {
  name: 'Kit Horizon',
  location: LOCATION,
  packageName: '@kit-horizon/ui',
  tagPrefix: 'kh-',
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

interface Harness {
  readonly fileSystem: InMemoryProjectFileSystem;
  readonly recents: InMemoryRecentProjectsStore;
  readonly deps: ProjectDependencies;
}

function harness(recentProjects: readonly RecentProject[] = []): Harness {
  const fileSystem = new InMemoryProjectFileSystem().addFolder(LOCATION);
  const recents = new InMemoryRecentProjectsStore(recentProjects);
  return { fileSystem, recents, deps: { fileSystem, recentProjects: recents, clock: () => new Date(NOW) } };
}

function expectCreated(outcome: CreateProjectOutcome): Extract<CreateProjectOutcome, { kind: 'created' }> {
  if (outcome.kind !== 'created') throw new Error(`expected a created project, got ${JSON.stringify(outcome)}`);
  return outcome;
}

function expectInvalid(outcome: CreateProjectOutcome): Extract<CreateProjectOutcome, { kind: 'invalid-request' }> {
  if (outcome.kind !== 'invalid-request') throw new Error(`expected an invalid request, got ${JSON.stringify(outcome)}`);
  return outcome;
}

function readJson(content: string | undefined): unknown {
  if (content === undefined) throw new Error('expected a file');
  return JSON.parse(content);
}

describe('createProject', () => {
  // covers: FCT-0003
  it('creates the project folder named after the package inside the chosen location', async () => {
    const { fileSystem, deps } = harness();

    const { project } = expectCreated(await createProject(deps, kitHorizonRequest));

    expect(project.path).toBe(PROJECT_PATH);
    expect(fileSystem.hasFolder(PROJECT_PATH)).toBe(true);
    expect(fileSystem.createFolderCalls.map(({ target }) => target)).toEqual([PROJECT_PATH]);
  });

  // covers: FCT-0003
  it('writes an npm library manifest that depends on Lit', async () => {
    const { fileSystem, deps } = harness();

    expectCreated(await createProject(deps, kitHorizonRequest));

    const manifest = fileSystem.fileContent(`${PROJECT_PATH}\\package.json`);
    expect(manifest).toBe(createLibraryManifest('@kit-horizon/ui'));
    expect(readJson(manifest)).toMatchObject({ name: '@kit-horizon/ui', dependencies: { lit: '^3.3.3' } });
  });

  // covers: FCT-0003
  it('writes the UI-GEN settings file with the requested prefix and the default components folder', async () => {
    const { fileSystem, deps } = harness();

    expectCreated(await createProject(deps, kitHorizonRequest));

    expect(readJson(fileSystem.fileContent(`${PROJECT_PATH}\\uigen.json`))).toEqual({
      formatVersion: 1,
      tagPrefix: 'kh-',
      componentsDir: 'src/components',
    });
  });

  // covers: FCT-0003
  it('creates an empty components folder and no other file', async () => {
    const { fileSystem, deps } = harness();

    expectCreated(await createProject(deps, kitHorizonRequest));

    const [call] = fileSystem.createFolderCalls;
    expect(call?.entries).toContainEqual({ kind: 'folder', relativePath: 'src/components' });
    const filePaths = call?.entries.filter((entry) => entry.kind === 'file').map((entry) => entry.relativePath);
    expect(filePaths?.toSorted()).toEqual(['package.json', 'uigen.json']);
    expect(fileSystem.hasFolder(`${PROJECT_PATH}\\src\\components`)).toBe(true);
    const insideComponents = fileSystem
      .snapshot()
      .filter(({ path }) => path.startsWith(`${PROJECT_PATH}\\src\\components\\`));
    expect(insideComponents).toEqual([]);
  });

  // covers: FCT-0003
  it('returns the opened project so the editor can open on it', async () => {
    const { deps } = harness();

    const { project } = expectCreated(await createProject(deps, kitHorizonRequest));

    expect(project).toEqual({
      path: PROJECT_PATH,
      name: 'Kit Horizon',
      packageName: '@kit-horizon/ui',
      version: '0.1.0',
      settings: { formatVersion: 1, tagPrefix: 'kh-', componentsDir: 'src/components' },
    });
  });

  // covers: FCT-0003
  it('keeps the typed name without surrounding spaces', async () => {
    const { deps } = harness();

    const { project } = expectCreated(await createProject(deps, { ...kitHorizonRequest, name: '  Kit Horizon \t' }));

    expect(project.name).toBe('Kit Horizon');
  });

  // covers: FCT-0003
  it('derives the folder name from an edited package name', async () => {
    const { fileSystem, deps } = harness();

    const { project } = expectCreated(
      await createProject(deps, { ...kitHorizonRequest, packageName: '@acme/buttons', tagPrefix: 'ac-' }),
    );

    expect(project.path).toBe('E:\\projets\\acme-buttons');
    expect(project.packageName).toBe('@acme/buttons');
    expect(project.settings.tagPrefix).toBe('ac-');
    expect(fileSystem.fileContent('E:\\projets\\acme-buttons\\package.json')).toBe(createLibraryManifest('@acme/buttons'));
  });

  // covers: FCT-0003
  it('creates the project inside an existing empty folder', async () => {
    const { fileSystem, deps } = harness();
    fileSystem.addFolder(PROJECT_PATH);

    const { project } = expectCreated(await createProject(deps, kitHorizonRequest));

    expect(project.path).toBe(PROJECT_PATH);
    expect(fileSystem.fileContent(`${PROJECT_PATH}\\uigen.json`)).toBeDefined();
  });

  // covers: FCT-0003
  it('records the created project at the top of the recent projects with the clock date', async () => {
    const { recents, deps } = harness([atlas, nova]);

    expectCreated(await createProject(deps, kitHorizonRequest));

    const [head, ...rest] = recents.current;
    expect(head?.path).toBe(PROJECT_PATH);
    expect(head?.name).toBe('Kit Horizon');
    expect(Date.parse(head?.openedAt ?? '')).toBe(NOW.getTime());
    expect(rest).toEqual([atlas, nova]);
  });

  // covers: FCT-0003
  it('records the created project even when the recent projects file is unreadable', async () => {
    const fileSystem = new InMemoryProjectFileSystem().addFolder(LOCATION);
    const recents = new InMemoryRecentProjectsStore([], true);

    expectCreated(
      await createProject({ fileSystem, recentProjects: recents, clock: () => new Date(NOW) }, kitHorizonRequest),
    );

    expect(recents.current.map(({ path }) => path)).toEqual([PROJECT_PATH]);
  });
});

describe('createProject with an invalid request', () => {
  // covers: FCT-0003
  it('rejects an empty location without touching the disk', async () => {
    const { fileSystem, recents, deps } = harness();
    const before = fileSystem.snapshot();

    const { errors } = expectInvalid(await createProject(deps, { ...kitHorizonRequest, location: '' }));

    expect(errors).toEqual([{ field: 'location', code: 'empty' }]);
    expect(fileSystem.snapshot()).toEqual(before);
    expect(fileSystem.createFolderCalls).toEqual([]);
    expect(recents.saves).toEqual([]);
  });

  // covers: FCT-0003
  it('rejects a blank project name', async () => {
    const { fileSystem, deps } = harness();

    const { errors } = expectInvalid(await createProject(deps, { ...kitHorizonRequest, name: '   ' }));

    expect(errors).toEqual([{ field: 'name', code: 'empty' }]);
    expect(fileSystem.createFolderCalls).toEqual([]);
  });

  // covers: FCT-0003
  it('rejects a project name without any letter or digit', async () => {
    const { deps } = harness();

    const { errors } = expectInvalid(await createProject(deps, { ...kitHorizonRequest, name: '***' }));

    expect(errors).toEqual([{ field: 'name', code: 'no-alphanumeric' }]);
  });

  // covers: FCT-0003
  it('rejects an invalid package name', async () => {
    const { deps } = harness();

    const { errors } = expectInvalid(await createProject(deps, { ...kitHorizonRequest, packageName: '@Kit Horizon/UI' }));

    expect(errors).toEqual([{ field: 'packageName', code: 'invalid' }]);
  });

  // covers: FCT-0003
  it('rejects an invalid tag prefix with the reason given by the prefix rule', async () => {
    const { deps } = harness();

    const { errors } = expectInvalid(await createProject(deps, { ...kitHorizonRequest, tagPrefix: 'kh' }));

    expect(errors).toEqual([{ field: 'tagPrefix', code: 'missing-trailing-dash' }]);
  });

  // covers: FCT-0003
  it('reports every invalid field at once', async () => {
    const { fileSystem, recents, deps } = harness();

    const { errors } = expectInvalid(
      await createProject(deps, { name: '', location: '', packageName: '', tagPrefix: 'KH-' }),
    );

    expect(errors).toHaveLength(4);
    expect(errors).toEqual(
      expect.arrayContaining([
        { field: 'name', code: 'empty' },
        { field: 'location', code: 'empty' },
        { field: 'packageName', code: 'empty' },
        { field: 'tagPrefix', code: 'uppercase' },
      ]),
    );
    expect(fileSystem.createFolderCalls).toEqual([]);
    expect(recents.saves).toEqual([]);
  });
});

describe('createProject on an existing folder', () => {
  // covers: FCT-0004
  it('refuses a non-empty project folder and names it with its entry count', async () => {
    const { fileSystem, deps } = harness();
    fileSystem
      .addFolder(PROJECT_PATH)
      .addFile(`${PROJECT_PATH}\\notes.txt`, 'draft')
      .addFolder(`${PROJECT_PATH}\\assets`);

    const outcome = await createProject(deps, kitHorizonRequest);

    expect(outcome).toEqual({ kind: 'folder-not-empty', path: PROJECT_PATH, entryCount: 2 });
  });

  // covers: FCT-0004
  it('writes nothing and leaves the recent projects untouched when the folder is not empty', async () => {
    const { fileSystem, recents, deps } = harness([atlas]);
    fileSystem.addFolder(PROJECT_PATH).addFile(`${PROJECT_PATH}\\README.md`, '# Kit\n');
    const before = fileSystem.snapshot();

    const outcome = await createProject(deps, kitHorizonRequest);

    expect(outcome.kind).toBe('folder-not-empty');
    expect(fileSystem.snapshot()).toEqual(before);
    expect(fileSystem.writtenFiles).toEqual([]);
    expect(recents.saves).toEqual([]);
    expect(recents.current).toEqual([atlas]);
  });

  // covers: FCT-0004
  it('fails with EEXIST when a file already sits at the project path', async () => {
    const { fileSystem, recents, deps } = harness();
    fileSystem.addFile(PROJECT_PATH, 'not a folder');
    const before = fileSystem.snapshot();

    const outcome = await createProject(deps, kitHorizonRequest);

    expect(outcome).toEqual({ kind: 'write-failed', path: PROJECT_PATH, code: 'EEXIST' });
    expect(fileSystem.snapshot()).toEqual(before);
    expect(recents.saves).toEqual([]);
  });
});

describe('createProject when writing fails', () => {
  // covers: FCT-0013
  it('reports the write error by its path and code', async () => {
    const { fileSystem, deps } = harness();
    fileSystem.failNextCreationWith(new ProjectWriteError(PROJECT_PATH, 'ENOSPC'));

    const outcome = await createProject(deps, kitHorizonRequest);

    expect(outcome).toEqual({ kind: 'write-failed', path: PROJECT_PATH, code: 'ENOSPC' });
  });

  // covers: FCT-0013
  it('leaves no partial folder or file on the disk', async () => {
    const { fileSystem, deps } = harness();
    fileSystem.failNextCreationWith(new ProjectWriteError(PROJECT_PATH, 'EACCES'));
    const before = fileSystem.snapshot();

    const outcome = await createProject(deps, kitHorizonRequest);

    expect(outcome.kind).toBe('write-failed');
    expect(fileSystem.snapshot()).toEqual(before);
    expect(fileSystem.hasFolder(PROJECT_PATH)).toBe(false);
    expect(fileSystem.writtenFiles).toEqual([]);
  });

  // covers: FCT-0013
  it('does not record a failed creation in the recent projects', async () => {
    const { fileSystem, recents, deps } = harness([atlas]);
    fileSystem.failNextCreationWith(new ProjectWriteError(PROJECT_PATH, 'EPERM'));

    const outcome = await createProject(deps, kitHorizonRequest);

    expect(outcome.kind).toBe('write-failed');
    expect(recents.saves).toEqual([]);
    expect(recents.current).toEqual([atlas]);
  });

  // covers: FCT-0013
  it('reports an unexpected error with the UNKNOWN code on the project path', async () => {
    const { fileSystem, recents, deps } = harness();
    fileSystem.failNextCreationWith(new TypeError('disk adapter crashed'));
    const before = fileSystem.snapshot();

    const outcome = await createProject(deps, kitHorizonRequest);

    expect(outcome).toEqual({ kind: 'write-failed', path: PROJECT_PATH, code: 'UNKNOWN' });
    expect(fileSystem.snapshot()).toEqual(before);
    expect(recents.saves).toEqual([]);
  });
});
