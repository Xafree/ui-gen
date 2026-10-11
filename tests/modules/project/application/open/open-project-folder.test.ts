import { describe, expect, it } from 'vitest';
import { openProjectFolder } from '../../../../../src/modules/project/application/open/open-project-folder';
import { InMemoryProjectFileSystem } from '../doubles/in-memory-project-file-system';
import {
  ATLAS_PACKAGE_JSON,
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

function packageWithoutLit(): string {
  return `{
  "name": "atlas-docs",
  "version": "1.0.0",
  "private": true,
  "dependencies": {
    "react": "^18.3.1"
  },
  "devDependencies": {
    "lit": "^3.2.0"
  }
}
`;
}

function nonLibraryFolder(packageJson: string | null): InMemoryProjectFileSystem {
  const fileSystem = new InMemoryProjectFileSystem().addFolder(LIBRARY_PATH).addFile(inLibrary('README.md'), '# Notes\n');
  if (packageJson !== null) fileSystem.addFile(inLibrary('package.json'), packageJson);
  return fileSystem;
}

describe('openProjectFolder with uigen.json', () => {
  // covers: FCT-0006
  it('opens a folder that carries valid settings without asking anything', async () => {
    const { deps } = harness(withAtlasLibrary().addFile(inLibrary('uigen.json'), VALID_SETTINGS));

    const { project } = expectOutcome(await openProjectFolder(deps, LIBRARY_PATH), 'opened');

    expect(project).toEqual({
      path: LIBRARY_PATH,
      name: 'Atlas Design',
      packageName: '@atlas-design/components',
      version: '2.4.1',
      settings: { formatVersion: 1, tagPrefix: 'at-', componentsDir: 'src/lib' },
    });
  });

  // covers: FCT-0006
  it('keeps the name the project already has in the recent list', async () => {
    const { deps } = harness(withAtlasLibrary().addFile(inLibrary('uigen.json'), VALID_SETTINGS), [NOVA_RECENT, ATLAS_RECENT]);

    const { project } = expectOutcome(await openProjectFolder(deps, LIBRARY_PATH), 'opened');

    expect(project.name).toBe('Atlas maison');
  });

  // covers: FCT-0006
  it('puts the opened project at the head of the recent list', async () => {
    const { deps, recents } = harness(withAtlasLibrary().addFile(inLibrary('uigen.json'), VALID_SETTINGS), [NOVA_RECENT]);

    expectOutcome(await openProjectFolder(deps, LIBRARY_PATH), 'opened');

    expect(recents.current).toEqual([
      { path: LIBRARY_PATH, name: 'Atlas Design', openedAt: NOW.toISOString() },
      NOVA_RECENT,
    ]);
  });

  // covers: FCT-0006
  it('does not touch any file of the folder when opening it', async () => {
    const fileSystem = withAtlasLibrary().addFile(inLibrary('uigen.json'), VALID_SETTINGS);
    const before = fileSystem.snapshot();
    const { deps } = harness(fileSystem);

    expectOutcome(await openProjectFolder(deps, LIBRARY_PATH), 'opened');

    expect(fileSystem.snapshot()).toEqual(before);
    expect(fileSystem.writtenFiles).toEqual([]);
  });

  it('refuses unreadable settings as invalid JSON and leaves everything intact', async () => {
    const fileSystem = withAtlasLibrary().addFile(inLibrary('uigen.json'), '{ "formatVersion": 1, "tagPrefix": "at-",');
    const before = fileSystem.snapshot();
    const { deps, recents } = harness(fileSystem, [NOVA_RECENT]);

    const outcome = expectOutcome(await openProjectFolder(deps, LIBRARY_PATH), 'invalid-settings');

    expect(outcome.path).toBe(LIBRARY_PATH);
    expect(outcome.errors).toEqual([{ code: 'invalid-json', field: null }]);
    expect(fileSystem.snapshot()).toEqual(before);
    expect(recents.saves).toEqual([]);
  });

  it('refuses settings with an unknown field, names it, and leaves the file intact', async () => {
    const settings = '{\n  "formatVersion": 1,\n  "tagprefix": "at-",\n  "tagPrefix": "at-",\n  "componentsDir": "src/lib"\n}\n';
    const fileSystem = withAtlasLibrary().addFile(inLibrary('uigen.json'), settings);
    const { deps, recents } = harness(fileSystem);

    const outcome = expectOutcome(await openProjectFolder(deps, LIBRARY_PATH), 'invalid-settings');

    expect(outcome.errors).toContainEqual({ code: 'unknown-field', field: 'tagprefix' });
    expect(fileSystem.fileContent(inLibrary('uigen.json'))).toBe(settings);
    expect(fileSystem.writtenFiles).toEqual([]);
    expect(recents.saves).toEqual([]);
  });

  it('refuses settings written by a newer UI-GEN version and leaves the file intact', async () => {
    const settings = '{\n  "formatVersion": 2,\n  "tagPrefix": "at-",\n  "componentsDir": "src/lib"\n}\n';
    const fileSystem = withAtlasLibrary().addFile(inLibrary('uigen.json'), settings);
    const { deps, recents } = harness(fileSystem);

    const outcome = expectOutcome(await openProjectFolder(deps, LIBRARY_PATH), 'invalid-settings');

    expect(outcome.errors).toContainEqual({ code: 'newer-format-version', field: 'formatVersion' });
    expect(fileSystem.fileContent(inLibrary('uigen.json'))).toBe(settings);
    expect(fileSystem.writtenFiles).toEqual([]);
    expect(recents.saves).toEqual([]);
  });

  it('reports a folder that no longer exists as missing without recording it', async () => {
    const { deps, recents } = harness(new InMemoryProjectFileSystem().addFolder('E:\\clients'));

    const outcome = await openProjectFolder(deps, LIBRARY_PATH);

    expect(outcome).toEqual({ kind: 'missing', path: LIBRARY_PATH });
    expect(recents.saves).toEqual([]);
  });
});

describe('openProjectFolder on a Lit library without uigen.json', () => {
  // covers: FCT-0007
  it('proposes the prefix used by most of the declared components', async () => {
    const { deps } = harness(withAtlasLibrary());

    const { proposal } = expectOutcome(await openProjectFolder(deps, LIBRARY_PATH), 'needs-adoption');

    expect(proposal.tagPrefix).toBe('at-');
    expect(proposal.exampleTag).toBe('at-button');
  });

  // covers: FCT-0021
  it('proposes the folder that contains the components', async () => {
    const { deps } = harness(withAtlasLibrary());

    const { proposal } = expectOutcome(await openProjectFolder(deps, LIBRARY_PATH), 'needs-adoption');

    expect(proposal.componentsDir).toBe('src/lib');
  });

  // covers: FCT-0007
  // covers: FCT-0021
  it('describes the library in the adoption proposal', async () => {
    const { deps } = harness(withAtlasLibrary());

    const { proposal } = expectOutcome(await openProjectFolder(deps, LIBRARY_PATH), 'needs-adoption');

    expect(proposal).toEqual({
      path: LIBRARY_PATH,
      name: 'Atlas Design',
      packageName: '@atlas-design/components',
      version: '2.4.1',
      componentCount: 4,
      componentsDir: 'src/lib',
      tagPrefix: 'at-',
      exampleTag: 'at-button',
    });
  });

  // covers: FCT-0007
  it('writes nothing before the adoption is confirmed', async () => {
    const fileSystem = withAtlasLibrary();
    const before = fileSystem.snapshot();
    const { deps, recents } = harness(fileSystem, [NOVA_RECENT]);

    expectOutcome(await openProjectFolder(deps, LIBRARY_PATH), 'needs-adoption');

    expect(fileSystem.snapshot()).toEqual(before);
    expect(fileSystem.writtenFiles).toEqual([]);
    expect(fileSystem.fileContent(inLibrary('uigen.json'))).toBeUndefined();
    expect(recents.saves).toEqual([]);
  });

  // covers: FCT-0008
  it('leaves every file of the folder and the recent list untouched when the adoption is never confirmed', async () => {
    const fileSystem = withAtlasLibrary();
    const before = fileSystem.snapshot();
    const { deps, recents } = harness(fileSystem, [NOVA_RECENT, ATLAS_RECENT]);

    expectOutcome(await openProjectFolder(deps, LIBRARY_PATH), 'needs-adoption');

    expect(fileSystem.snapshot()).toEqual(before);
    expect(fileSystem.fileContent(inLibrary('package.json'))).toBe(ATLAS_PACKAGE_JSON);
    expect(recents.saves).toEqual([]);
    expect(recents.current).toEqual([NOVA_RECENT, ATLAS_RECENT]);
  });

  // covers: FCT-0007
  // covers: FCT-0021
  it('proposes no prefix and the default folder for a Lit library that declares no component yet', async () => {
    const fileSystem = new InMemoryProjectFileSystem()
      .addFolder(LIBRARY_PATH)
      .addFile(inLibrary('package.json'), ATLAS_PACKAGE_JSON)
      .addFile(inLibrary('src\\index.ts'), "export const VERSION = '2.4.1';\n");
    const { deps } = harness(fileSystem);

    const { proposal } = expectOutcome(await openProjectFolder(deps, LIBRARY_PATH), 'needs-adoption');

    expect(proposal.tagPrefix).toBeNull();
    expect(proposal.exampleTag).toBeNull();
    expect(proposal.componentCount).toBe(0);
    expect(proposal.componentsDir).toBe('src/components');
  });

  // covers: FCT-0007
  it('accepts Lit declared as a peer dependency', async () => {
    const packageJson = '{\n  "name": "@atlas-design/components",\n  "version": "2.4.1",\n  "peerDependencies": { "lit": "^3.0.0" }\n}\n';
    const fileSystem = new InMemoryProjectFileSystem().addFolder(LIBRARY_PATH).addFile(inLibrary('package.json'), packageJson);
    const { deps } = harness(fileSystem);

    const { proposal } = expectOutcome(await openProjectFolder(deps, LIBRARY_PATH), 'needs-adoption');

    expect(proposal.packageName).toBe('@atlas-design/components');
  });
});

describe('openProjectFolder on a folder that is not a Lit library', () => {
  // covers: FCT-0009
  it('explains that package.json is missing', async () => {
    const fileSystem = nonLibraryFolder(null);
    const before = fileSystem.snapshot();
    const { deps, recents } = harness(fileSystem);

    const outcome = await openProjectFolder(deps, LIBRARY_PATH);

    expect(outcome).toEqual({ kind: 'not-a-lit-library', path: LIBRARY_PATH, reason: 'missing-package-json' });
    expect(fileSystem.snapshot()).toEqual(before);
    expect(recents.saves).toEqual([]);
  });

  // covers: FCT-0009
  it('explains that package.json cannot be read', async () => {
    const fileSystem = nonLibraryFolder('{\n  "name": "atlas",\n  "dependencies": { "lit": "^3.2.0" }\n');
    const before = fileSystem.snapshot();
    const { deps, recents } = harness(fileSystem);

    const outcome = await openProjectFolder(deps, LIBRARY_PATH);

    expect(outcome).toEqual({ kind: 'not-a-lit-library', path: LIBRARY_PATH, reason: 'invalid-package-json' });
    expect(fileSystem.snapshot()).toEqual(before);
    expect(recents.saves).toEqual([]);
  });

  // covers: FCT-0009
  it('explains that package.json has no package name', async () => {
    const fileSystem = nonLibraryFolder('{\n  "version": "1.0.0",\n  "dependencies": { "lit": "^3.2.0" }\n}\n');
    const { deps } = harness(fileSystem);

    const outcome = await openProjectFolder(deps, LIBRARY_PATH);

    expect(outcome).toEqual({ kind: 'not-a-lit-library', path: LIBRARY_PATH, reason: 'invalid-package-json' });
  });

  // covers: FCT-0009
  it('explains that the package does not depend on Lit', async () => {
    const fileSystem = nonLibraryFolder(packageWithoutLit());
    const before = fileSystem.snapshot();
    const { deps, recents } = harness(fileSystem);

    const outcome = await openProjectFolder(deps, LIBRARY_PATH);

    expect(outcome).toEqual({ kind: 'not-a-lit-library', path: LIBRARY_PATH, reason: 'missing-lit-dependency' });
    expect(fileSystem.snapshot()).toEqual(before);
    expect(fileSystem.writtenFiles).toEqual([]);
    expect(recents.saves).toEqual([]);
  });
});
