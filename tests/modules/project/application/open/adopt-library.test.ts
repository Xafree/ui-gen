import { describe, expect, it } from 'vitest';
import { adoptLibrary, type AdoptLibraryRequest } from '../../../../../src/modules/project/application/open/adopt-library';
import { InMemoryProjectFileSystem } from '../doubles/in-memory-project-file-system';
import {
  ATLAS_PACKAGE_JSON,
  ATLAS_RECENT,
  FailingWriteFileSystem,
  LIBRARY_PATH,
  NOVA_RECENT,
  NOW,
  VALID_SETTINGS,
  expectOutcome,
  harness,
  inLibrary,
  withAtlasLibrary,
} from './lit-library-fixtures';

const confirmedProposal: AdoptLibraryRequest = { path: LIBRARY_PATH, tagPrefix: 'at-', componentsDir: 'src/lib' };

describe('adoptLibrary', () => {
  // covers: FCT-0007
  // covers: FCT-0021
  it('writes uigen.json with the confirmed prefix and folder once the adoption is confirmed', async () => {
    const fileSystem = withAtlasLibrary();
    const { deps } = harness(fileSystem);

    expectOutcome(await adoptLibrary(deps, confirmedProposal), 'opened');

    expect(fileSystem.writtenFiles).toEqual([inLibrary('uigen.json')]);
    expect(fileSystem.fileContent(inLibrary('uigen.json'))).toBe(
      '{\n  "formatVersion": 1,\n  "tagPrefix": "at-",\n  "componentsDir": "src/lib"\n}\n',
    );
  });

  // covers: FCT-0007
  it('keeps the prefix the user changed instead of the proposed one', async () => {
    const fileSystem = withAtlasLibrary();
    const { deps } = harness(fileSystem);

    const { project } = expectOutcome(await adoptLibrary(deps, { ...confirmedProposal, tagPrefix: 'atlas-' }), 'opened');

    expect(project.settings.tagPrefix).toBe('atlas-');
    expect(fileSystem.fileContent(inLibrary('uigen.json'))).toBe(
      '{\n  "formatVersion": 1,\n  "tagPrefix": "atlas-",\n  "componentsDir": "src/lib"\n}\n',
    );
  });

  // covers: FCT-0021
  it('keeps the components folder the user changed instead of the proposed one', async () => {
    const fileSystem = withAtlasLibrary();
    const { deps } = harness(fileSystem);

    const { project } = expectOutcome(await adoptLibrary(deps, { ...confirmedProposal, componentsDir: 'src/lib/button' }), 'opened');

    expect(project.settings.componentsDir).toBe('src/lib/button');
    expect(fileSystem.fileContent(inLibrary('uigen.json'))).toBe(
      '{\n  "formatVersion": 1,\n  "tagPrefix": "at-",\n  "componentsDir": "src/lib/button"\n}\n',
    );
  });

  // covers: FCT-0007
  it('opens the adopted library and records it at the head of the recent list', async () => {
    const fileSystem = withAtlasLibrary();
    const { deps, recents } = harness(fileSystem, [NOVA_RECENT]);

    const { project } = expectOutcome(await adoptLibrary(deps, confirmedProposal), 'opened');

    expect(project).toEqual({
      path: LIBRARY_PATH,
      name: 'Atlas Design',
      packageName: '@atlas-design/components',
      version: '2.4.1',
      settings: { formatVersion: 1, tagPrefix: 'at-', componentsDir: 'src/lib' },
    });
    expect(recents.current).toEqual([
      { path: LIBRARY_PATH, name: 'Atlas Design', openedAt: NOW.toISOString() },
      NOVA_RECENT,
    ]);
  });

  // covers: FCT-0007
  it('leaves the library files other than uigen.json untouched', async () => {
    const fileSystem = withAtlasLibrary();
    const before = fileSystem.snapshot();
    const { deps } = harness(fileSystem);

    expectOutcome(await adoptLibrary(deps, confirmedProposal), 'opened');

    const after = fileSystem.snapshot().filter((entry) => entry.path !== inLibrary('uigen.json'));
    expect(after).toEqual(before);
    expect(fileSystem.fileContent(inLibrary('package.json'))).toBe(ATLAS_PACKAGE_JSON);
  });
});

describe('adoptLibrary refusals', () => {
  // covers: FCT-0007
  it('refuses an invalid prefix by its cause and writes nothing', async () => {
    const fileSystem = withAtlasLibrary();
    const before = fileSystem.snapshot();
    const { deps, recents } = harness(fileSystem);

    const outcome = expectOutcome(await adoptLibrary(deps, { ...confirmedProposal, tagPrefix: 'At-' }), 'invalid-request');

    expect(outcome.errors).toEqual([{ field: 'tagPrefix', code: 'uppercase' }]);
    expect(fileSystem.snapshot()).toEqual(before);
    expect(fileSystem.writtenFiles).toEqual([]);
    expect(recents.saves).toEqual([]);
  });

  // covers: FCT-0007
  it('refuses a prefix without trailing dash', async () => {
    const { deps, fileSystem } = harness(withAtlasLibrary());

    const outcome = expectOutcome(await adoptLibrary(deps, { ...confirmedProposal, tagPrefix: 'at' }), 'invalid-request');

    expect(outcome.errors).toEqual([{ field: 'tagPrefix', code: 'missing-trailing-dash' }]);
    expect(fileSystem.writtenFiles).toEqual([]);
  });

  // covers: FCT-0021
  it('refuses a components folder that leaves the project and writes nothing', async () => {
    const fileSystem = withAtlasLibrary();
    const before = fileSystem.snapshot();
    const { deps, recents } = harness(fileSystem);

    const outcome = expectOutcome(
      await adoptLibrary(deps, { ...confirmedProposal, componentsDir: '../shared/components' }),
      'invalid-request',
    );

    expect(outcome.errors).toEqual([{ field: 'componentsDir', code: 'invalid-components-dir' }]);
    expect(fileSystem.snapshot()).toEqual(before);
    expect(recents.saves).toEqual([]);
  });

  // covers: FCT-0021
  it('refuses an absolute Windows components folder', async () => {
    const { deps, fileSystem } = harness(withAtlasLibrary());

    const outcome = expectOutcome(
      await adoptLibrary(deps, { ...confirmedProposal, componentsDir: 'C:\\atlas\\src\\lib' }),
      'invalid-request',
    );

    expect(outcome.errors).toEqual([{ field: 'componentsDir', code: 'invalid-components-dir' }]);
    expect(fileSystem.writtenFiles).toEqual([]);
  });

  // covers: FCT-0006
  it('opens a folder that already has uigen.json with its own settings and does not overwrite them', async () => {
    const fileSystem = withAtlasLibrary().addFile(inLibrary('uigen.json'), VALID_SETTINGS);
    const { deps } = harness(fileSystem, [ATLAS_RECENT]);

    const { project } = expectOutcome(
      await adoptLibrary(deps, { path: LIBRARY_PATH, tagPrefix: 'zz-', componentsDir: 'elsewhere' }),
      'opened',
    );

    expect(project.settings).toEqual({ formatVersion: 1, tagPrefix: 'at-', componentsDir: 'src/lib' });
    expect(project.name).toBe('Atlas maison');
    expect(fileSystem.fileContent(inLibrary('uigen.json'))).toBe(VALID_SETTINGS);
    expect(fileSystem.writtenFiles).toEqual([]);
  });

  // covers: FCT-0009
  it('does not adopt a folder that is not a Lit library', async () => {
    const fileSystem = new InMemoryProjectFileSystem()
      .addFolder(LIBRARY_PATH)
      .addFile(inLibrary('package.json'), '{\n  "name": "atlas-docs",\n  "dependencies": { "react": "^18.3.1" }\n}\n');
    const before = fileSystem.snapshot();
    const { deps, recents } = harness(fileSystem);

    const outcome = await adoptLibrary(deps, confirmedProposal);

    expect(outcome).toEqual({ kind: 'not-a-lit-library', path: LIBRARY_PATH, reason: 'missing-lit-dependency' });
    expect(fileSystem.snapshot()).toEqual(before);
    expect(recents.saves).toEqual([]);
  });

  it('reports a folder that disappeared before the confirmation as missing', async () => {
    const { deps, fileSystem } = harness(new InMemoryProjectFileSystem().addFolder('E:\\clients'));

    const outcome = await adoptLibrary(deps, confirmedProposal);

    expect(outcome).toEqual({ kind: 'missing', path: LIBRARY_PATH });
    expect(fileSystem.writtenFiles).toEqual([]);
  });

  it('reports a failed write of uigen.json by its code and does not record the project', async () => {
    const fileSystem = withAtlasLibrary(new FailingWriteFileSystem());
    const { deps, recents } = harness(fileSystem, [NOVA_RECENT]);

    const outcome = expectOutcome(await adoptLibrary(deps, confirmedProposal), 'write-failed');

    expect(outcome.code).toBe('EPERM');
    expect(fileSystem.fileContent(inLibrary('uigen.json'))).toBeUndefined();
    expect(recents.saves).toEqual([]);
    expect(recents.current).toEqual([NOVA_RECENT]);
  });
});
