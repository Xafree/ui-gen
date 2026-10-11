import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  ATLAS_RECENT,
  LIBRARY_PATH,
  inLibrary,
  withAtlasLibrary,
} from '../../../modules/project/application/open/lit-library-fixtures';
import { RecordingFileSystem, setUpHandlers } from './handler-harness';

const atlasAdoption = { path: LIBRARY_PATH, tagPrefix: 'at-', componentsDir: 'src/lib' };

const UIGEN_SETTINGS = '{\n  "formatVersion": 1,\n  "tagPrefix": "at-",\n  "componentsDir": "src/lib"\n}\n';

function withAdoptableAtlas(): RecordingFileSystem {
  const fileSystem = new RecordingFileSystem();
  withAtlasLibrary(fileSystem);
  return fileSystem;
}

beforeEach(() => {
  vi.spyOn(console, 'warn').mockReturnValue(undefined);
  vi.spyOn(console, 'error').mockReturnValue(undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('adoption through the project handlers', () => {
  // covers: FCT-0007
  it('lets a recent project that lost its settings file be adopted after the confirmation', async () => {
    const harness = setUpHandlers({ fileSystem: withAdoptableAtlas(), recents: [ATLAS_RECENT] });

    expect(await harness.registry.invoke('recents:open', { path: LIBRARY_PATH })).toMatchObject({
      kind: 'needs-adoption',
      proposal: { path: LIBRARY_PATH, name: 'Atlas maison' },
    });
    expect(harness.fileSystem.fileContent(inLibrary('uigen.json'))).toBeUndefined();

    expect(await harness.registry.invoke('project:adopt', atlasAdoption)).toMatchObject({
      kind: 'opened',
      project: { path: LIBRARY_PATH, name: 'Atlas maison' },
    });
    expect(harness.fileSystem.fileContent(inLibrary('uigen.json'))).toBe(UIGEN_SETTINGS);
    expect(harness.session.currentProject).toMatchObject({ path: LIBRARY_PATH });
  });

  // covers: FCT-0007
  it('keeps the adoption pending after a refused confirmation so that it can be confirmed again', async () => {
    const harness = setUpHandlers({ fileSystem: withAdoptableAtlas(), recents: [ATLAS_RECENT] });
    await harness.registry.invoke('recents:open', { path: LIBRARY_PATH });

    expect(await harness.registry.invoke('project:adopt', { ...atlasAdoption, tagPrefix: 'At-' })).toEqual({
      kind: 'invalid-request',
      errors: [{ field: 'tagPrefix', code: 'uppercase' }],
    });
    expect(harness.fileSystem.fileContent(inLibrary('uigen.json'))).toBeUndefined();
    expect(harness.session.currentProject).toBeNull();

    expect(await harness.registry.invoke('project:adopt', atlasAdoption)).toMatchObject({ kind: 'opened' });
    expect(harness.fileSystem.fileContent(inLibrary('uigen.json'))).toBe(UIGEN_SETTINGS);
  });
});
