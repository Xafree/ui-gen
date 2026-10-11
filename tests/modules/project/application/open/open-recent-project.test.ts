import { describe, expect, it } from 'vitest';
import { openRecentProject } from '../../../../../src/modules/project/application/open/open-recent-project';
import { InMemoryProjectFileSystem } from '../doubles/in-memory-project-file-system';
import {
  ATLAS_RECENT,
  LIBRARY_PATH,
  NOVA_RECENT,
  NOW,
  VALID_SETTINGS,
  expectOutcome,
  harness,
  inLibrary,
  withAtlasLibrary,
} from './lit-library-fixtures';

describe('openRecentProject', () => {
  // covers: FCT-0006
  it('opens a recent project that carries valid settings and moves it to the head of the list', async () => {
    const { deps, recents } = harness(withAtlasLibrary().addFile(inLibrary('uigen.json'), VALID_SETTINGS), [
      NOVA_RECENT,
      ATLAS_RECENT,
    ]);

    const { project } = expectOutcome(await openRecentProject(deps, LIBRARY_PATH), 'opened');

    expect(project).toEqual({
      path: LIBRARY_PATH,
      name: 'Atlas maison',
      packageName: '@atlas-design/components',
      version: '2.4.1',
      settings: { formatVersion: 1, tagPrefix: 'at-', componentsDir: 'src/lib' },
    });
    expect(recents.current).toEqual([
      { path: LIBRARY_PATH, name: 'Atlas maison', openedAt: NOW.toISOString() },
      NOVA_RECENT,
    ]);
  });

  it('refuses a path that is not in the recent list and touches nothing', async () => {
    const fileSystem = withAtlasLibrary().addFile(inLibrary('uigen.json'), VALID_SETTINGS);
    const before = fileSystem.snapshot();
    const { deps, recents } = harness(fileSystem, [NOVA_RECENT]);

    const outcome = await openRecentProject(deps, LIBRARY_PATH);

    expect(outcome).toEqual({ kind: 'invalid-request', errors: [{ field: 'path', code: 'not-granted' }] });
    expect(fileSystem.snapshot()).toEqual(before);
    expect(recents.saves).toEqual([]);
  });

  it('refuses any path when the recent list is empty', async () => {
    const { deps } = harness(withAtlasLibrary().addFile(inLibrary('uigen.json'), VALID_SETTINGS));

    const outcome = await openRecentProject(deps, LIBRARY_PATH);

    expect(outcome).toEqual({ kind: 'invalid-request', errors: [{ field: 'path', code: 'not-granted' }] });
  });

  // covers: FCT-0007
  it('asks for adoption when a recent library has lost its uigen.json', async () => {
    const fileSystem = withAtlasLibrary();
    const { deps, recents } = harness(fileSystem, [ATLAS_RECENT]);

    const { proposal } = expectOutcome(await openRecentProject(deps, LIBRARY_PATH), 'needs-adoption');

    expect(proposal.tagPrefix).toBe('at-');
    expect(fileSystem.writtenFiles).toEqual([]);
    expect(recents.saves).toEqual([]);
  });

  it('reports a recent project whose folder was removed as missing and keeps the list unchanged', async () => {
    const { deps, recents } = harness(new InMemoryProjectFileSystem().addFolder('E:\\clients'), [NOVA_RECENT, ATLAS_RECENT]);

    const outcome = await openRecentProject(deps, LIBRARY_PATH);

    expect(outcome).toEqual({ kind: 'missing', path: LIBRARY_PATH });
    expect(recents.saves).toEqual([]);
    expect(recents.current).toEqual([NOVA_RECENT, ATLAS_RECENT]);
  });
});
