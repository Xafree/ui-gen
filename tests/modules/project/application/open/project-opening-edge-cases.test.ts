import { describe, expect, it, vi } from 'vitest';
import {
  adoptLibrary,
  type AdoptLibraryRequest,
} from '../../../../../src/modules/project/application/open/adopt-library';
import { openProjectFolder } from '../../../../../src/modules/project/application/open/open-project-folder';
import type { SourceFile } from '../../../../../src/modules/project/ports/project-file-system';
import { InMemoryProjectFileSystem } from '../doubles/in-memory-project-file-system';
import {
  ATLAS_RECENT,
  LIBRARY_PATH,
  NOVA_RECENT,
  NOW,
  expectOutcome,
  harness,
  inLibrary,
  withAtlasLibrary,
} from './lit-library-fixtures';

const SETTINGS_PATH = inLibrary('uigen.json');
const confirmedProposal: AdoptLibraryRequest = { path: LIBRARY_PATH, tagPrefix: 'at-', componentsDir: 'src/lib' };

describe('openProjectFolder edge cases', () => {
  it('reports a path that is a file rather than a folder as missing and touches nothing', async () => {
    const fileSystem = new InMemoryProjectFileSystem().addFolder('E:\\clients').addFile(LIBRARY_PATH, 'not a folder');
    const before = fileSystem.snapshot();
    const { deps, recents } = harness(fileSystem);

    const outcome = await openProjectFolder(deps, LIBRARY_PATH);

    expect(outcome).toEqual({ kind: 'missing', path: LIBRARY_PATH });
    expect(fileSystem.snapshot()).toEqual(before);
    expect(recents.saves).toEqual([]);
  });

  // covers: FCT-0021
  it('proposes the components folder even when the file system reports Windows separators', async () => {
    const fileSystem = withAtlasLibrary();
    const windowsListing: SourceFile[] = [
      { relativePath: 'src\\lib\\button\\at-button.ts', content: "@customElement('at-button')" },
      { relativePath: 'src\\lib\\card\\at-card.ts', content: "customElements.define('at-card', AtCard);" },
    ];
    vi.spyOn(fileSystem, 'listSourceFiles').mockResolvedValue(windowsListing);
    const { deps } = harness(fileSystem);

    const { proposal } = expectOutcome(await openProjectFolder(deps, LIBRARY_PATH), 'needs-adoption');

    expect(proposal.componentsDir).toBe('src/lib');
  });
});

describe('adoptLibrary edge cases', () => {
  it('reports an unexpected write error with the UNKNOWN code and records nothing', async () => {
    const fileSystem = withAtlasLibrary();
    vi.spyOn(fileSystem, 'writeFileAtomically').mockRejectedValueOnce(new TypeError('disk crashed'));
    const { deps, recents } = harness(fileSystem, [NOVA_RECENT]);

    const outcome = await adoptLibrary(deps, confirmedProposal);

    expect(outcome).toEqual({ kind: 'write-failed', path: LIBRARY_PATH, code: 'UNKNOWN' });
    expect(fileSystem.fileContent(SETTINGS_PATH)).toBeUndefined();
    expect(recents.saves).toEqual([]);
  });

  it('never overwrites a uigen.json that is invalid and says why', async () => {
    const settings = '{\n  "formatVersion": 1,\n  "tagprefix": "at-",\n  "componentsDir": "src/lib"\n}\n';
    const fileSystem = withAtlasLibrary().addFile(SETTINGS_PATH, settings);
    const { deps, recents } = harness(fileSystem);

    const outcome = expectOutcome(await adoptLibrary(deps, confirmedProposal), 'invalid-settings');

    expect(outcome.errors).toContainEqual({ code: 'unknown-field', field: 'tagprefix' });
    expect(fileSystem.fileContent(SETTINGS_PATH)).toBe(settings);
    expect(fileSystem.writtenFiles).toEqual([]);
    expect(recents.saves).toEqual([]);
  });

  // covers: FCT-0006
  // covers: FCT-0007
  it('opens an adopted library straight away the next time without asking again', async () => {
    const { deps } = harness(withAtlasLibrary());
    expectOutcome(await adoptLibrary(deps, confirmedProposal), 'opened');

    const { project } = expectOutcome(await openProjectFolder(deps, LIBRARY_PATH), 'opened');

    expect(project.settings).toEqual({ formatVersion: 1, tagPrefix: 'at-', componentsDir: 'src/lib' });
  });

  // covers: FCT-0007
  it('keeps the name a recent library already has when it is adopted', async () => {
    const { deps, recents } = harness(withAtlasLibrary(), [NOVA_RECENT, ATLAS_RECENT]);

    const { project } = expectOutcome(await adoptLibrary(deps, confirmedProposal), 'opened');

    expect(project.name).toBe('Atlas maison');
    expect(recents.current).toEqual([
      { path: LIBRARY_PATH, name: 'Atlas maison', openedAt: NOW.toISOString() },
      NOVA_RECENT,
    ]);
  });

  // covers: FCT-0007
  // covers: FCT-0021
  it('reports the invalid prefix and the invalid components folder together', async () => {
    const { deps, fileSystem } = harness(withAtlasLibrary());

    const outcome = expectOutcome(
      await adoptLibrary(deps, { path: LIBRARY_PATH, tagPrefix: 'at', componentsDir: '../lib' }),
      'invalid-request',
    );

    expect(outcome.errors).toEqual([
      { field: 'tagPrefix', code: 'missing-trailing-dash' },
      { field: 'componentsDir', code: 'invalid-components-dir' },
    ]);
    expect(fileSystem.writtenFiles).toEqual([]);
  });
});
