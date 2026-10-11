import { format } from 'node:util';
import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from 'vitest';
import { IPC_CHANNELS, type IpcChannel } from '../../../../src/main/ipc/channels';
import { IpcRejectedError } from '../../../../src/main/ipc/payloads';
import type { RecentProject } from '../../../../src/modules/project/domain/recents/recent-projects';
import type { RecentProjectsLoad } from '../../../../src/modules/project/ports/recent-projects-store';
import {
  ATLAS_RECENT,
  LIBRARY_PATH,
  NOVA_RECENT,
  VALID_SETTINGS,
  inLibrary,
  withAtlasLibrary,
} from '../../../modules/project/application/open/lit-library-fixtures';
import {
  DOCUMENTS,
  FOREIGN_URL,
  RecordingFileSystem,
  RecordingRecentsStore,
  TRUSTED_URL,
  expectNothingTouched,
  rejectionOf,
  setUpHandlers,
  type HandlerHarness,
} from './handler-harness';

const MARKETING_SITE = 'E:\\clients\\marketing-site';
const NOVA_KIT = 'E:\\clients\\nova-kit';
const SECRET_MARKER = 'top-secret-roadmap';

const createRequest = {
  name: 'Kit Horizon',
  location: DOCUMENTS,
  packageName: '@kit-horizon/ui',
  tagPrefix: 'kh-',
};

const atlasAdoption = { path: LIBRARY_PATH, tagPrefix: 'at-', componentsDir: 'src/lib' };

const atlasSettings = { formatVersion: 1, tagPrefix: 'at-', componentsDir: 'src/lib' };

const openedAtlas = {
  kind: 'opened',
  project: {
    path: LIBRARY_PATH,
    packageName: '@atlas-design/components',
    version: '2.4.1',
    settings: atlasSettings,
  },
};

interface ChannelCall {
  readonly channel: IpcChannel;
  readonly payload: unknown;
}

const VALID_CALLS: readonly ChannelCall[] = [
  { channel: 'recents:list', payload: undefined },
  { channel: 'recents:remove', payload: { path: `E:\\clients\\${SECRET_MARKER}` } },
  { channel: 'recents:open', payload: { path: `E:\\clients\\${SECRET_MARKER}` } },
  { channel: 'project:default-location', payload: undefined },
  { channel: 'project:choose-location', payload: undefined },
  { channel: 'project:create', payload: { ...createRequest, name: SECRET_MARKER } },
  { channel: 'project:open-folder', payload: undefined },
  { channel: 'project:adopt', payload: { ...atlasAdoption, path: `E:\\clients\\${SECRET_MARKER}` } },
  { channel: 'project:close', payload: undefined },
];

const MALFORMED_CALLS: readonly ChannelCall[] = [
  { channel: 'recents:remove', payload: { path: SECRET_MARKER } },
  { channel: 'recents:remove', payload: null },
  { channel: 'recents:open', payload: { path: `E:\\clients\\${SECRET_MARKER}`, force: true } },
  { channel: 'recents:open', payload: `E:\\clients\\${SECRET_MARKER}` },
  { channel: 'project:create', payload: { ...createRequest, location: SECRET_MARKER } },
  { channel: 'project:create', payload: { name: SECRET_MARKER, location: DOCUMENTS } },
  { channel: 'project:adopt', payload: { ...atlasAdoption, componentsDir: 42, path: `E:\\clients\\${SECRET_MARKER}` } },
  { channel: 'project:adopt', payload: undefined },
];

function withOpenableAtlas(): RecordingFileSystem {
  const fileSystem = new RecordingFileSystem();
  withAtlasLibrary(fileSystem).addFile(inLibrary('uigen.json'), VALID_SETTINGS);
  return fileSystem;
}

function withAdoptableAtlas(): RecordingFileSystem {
  const fileSystem = new RecordingFileSystem();
  withAtlasLibrary(fileSystem);
  return fileSystem;
}

function addMarketingSite(fileSystem: RecordingFileSystem): RecordingFileSystem {
  fileSystem.addFolder(MARKETING_SITE).addFile(
    `${MARKETING_SITE}\\package.json`,
    `{
  "name": "marketing-site",
  "version": "1.0.0",
  "private": true,
  "dependencies": {
    "astro": "^4.16.0"
  }
}
`,
  );
  return fileSystem;
}

function addNovaKit(fileSystem: RecordingFileSystem): RecordingFileSystem {
  fileSystem
    .addFolder(NOVA_KIT)
    .addFile(
      `${NOVA_KIT}\\package.json`,
      `{
  "name": "nova-kit",
  "version": "0.3.0",
  "dependencies": {
    "lit": "^3.1.4"
  }
}
`,
    )
    .addFile(
      `${NOVA_KIT}\\src\\nv-badge.ts`,
      `import { LitElement, html } from 'lit';

export class NvBadge extends LitElement {
  render() {
    return html\`<span><slot></slot></span>\`;
  }
}

customElements.define('nv-badge', NvBadge);
`,
    );
  return fileSystem;
}

function projectOf(outcome: unknown): unknown {
  if (typeof outcome === 'object' && outcome !== null && 'project' in outcome) return outcome.project;
  throw new Error(`Expected an outcome with a project, got ${JSON.stringify(outcome)}`);
}

function recent(slug: string, openedAt: string): RecentProject {
  return { path: `E:\\work\\${slug}`, name: `Projet ${slug}`, openedAt };
}

async function openAtlasForAdoption(harness: HandlerHarness): Promise<void> {
  harness.picker.answer(LIBRARY_PATH);
  expect(await harness.registry.invoke('project:open-folder')).toMatchObject({
    kind: 'needs-adoption',
    proposal: { path: LIBRARY_PATH },
  });
}

let warn: MockInstance<typeof console.warn>;
let error: MockInstance<typeof console.error>;

function warnings(): string {
  return warn.mock.calls.map((call) => format(...call)).join('\n');
}

beforeEach(() => {
  warn = vi.spyOn(console, 'warn').mockReturnValue(undefined);
  error = vi.spyOn(console, 'error').mockReturnValue(undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('registerProjectHandlers', () => {
  it('registers exactly the project and recent list channels', () => {
    const { registry } = setUpHandlers();

    expect(registry.channels.toSorted()).toEqual([
      'project:adopt',
      'project:choose-location',
      'project:close',
      'project:create',
      'project:default-location',
      'project:open-folder',
      'recents:clear',
      'recents:list',
      'recents:open',
      'recents:remove',
      'recents:restore',
    ]);
    expect(Object.values(IPC_CHANNELS)).toEqual(expect.arrayContaining(registry.channels));
  });

  it('declares the component channels next to the project ones', () => {
    expect(Object.values(IPC_CHANNELS)).toEqual(
      expect.arrayContaining(['component:list', 'component:open', 'component:last-opened']),
    );
  });
});

describe('sender check', () => {
  it.each(VALID_CALLS)('rejects $channel from an untrusted sender before any use case runs', async ({ channel, payload }) => {
    const harness = setUpHandlers({ fileSystem: withOpenableAtlas(), recents: [ATLAS_RECENT] });

    const rejection = await rejectionOf(() => harness.registry.invokeFrom({ url: FOREIGN_URL }, channel, payload));

    expect(rejection).toBeInstanceOf(IpcRejectedError);
    expect(rejection).toMatchObject({ reason: 'untrusted-sender' });
    expect(harness.checkedSenders).toEqual([FOREIGN_URL]);
    expectNothingTouched(harness);
    expect(harness.session.currentProject).toBeNull();
  });

  it.each(VALID_CALLS)('checks the sender of $channel before reading a malformed payload', async ({ channel }) => {
    const harness = setUpHandlers();

    const rejection = await rejectionOf(() => harness.registry.invokeFrom({ url: FOREIGN_URL }, channel, { path: 42 }));

    expect(rejection).toBeInstanceOf(IpcRejectedError);
    expect(rejection).toMatchObject({ reason: 'untrusted-sender' });
    expectNothingTouched(harness);
  });

  it.each(VALID_CALLS)('rejects $channel when the sender frame is gone, even if every URL were trusted', async ({ channel, payload }) => {
    const harness = setUpHandlers({ isTrustedSender: () => true, recents: [ATLAS_RECENT] });

    const rejection = await rejectionOf(() => harness.registry.invokeFrom(null, channel, payload));

    expect(rejection).toBeInstanceOf(IpcRejectedError);
    expect(rejection).toMatchObject({ reason: 'untrusted-sender' });
    expectNothingTouched(harness);
  });

  it('keeps the current project when an untrusted sender asks to close it', async () => {
    const harness = setUpHandlers({ fileSystem: withOpenableAtlas() });
    harness.picker.answer(LIBRARY_PATH);
    const opened = await harness.registry.invoke('project:open-folder');

    const rejection = await rejectionOf(() => harness.registry.invokeFrom({ url: FOREIGN_URL }, 'project:close'));

    expect(rejection).toMatchObject({ reason: 'untrusted-sender' });
    expect(harness.session.currentProject).toEqual(projectOf(opened));
  });

  it('does not grant a location chosen for an untrusted sender', async () => {
    const harness = setUpHandlers();
    harness.picker.answer('D:\\design\\kits');

    await rejectionOf(() => harness.registry.invokeFrom({ url: FOREIGN_URL }, 'project:choose-location'));

    expect(harness.session.isLocationGranted('D:\\design\\kits')).toBe(false);
    expect(harness.picker.purposes).toEqual([]);
  });

  it('logs the reason and the sender URL of a refused sender, never the payload', async () => {
    const harness = setUpHandlers();

    await rejectionOf(() =>
      harness.registry.invokeFrom({ url: FOREIGN_URL }, 'project:create', { ...createRequest, name: SECRET_MARKER }),
    );

    expect(warn).toHaveBeenCalledTimes(1);
    expect(warnings()).toContain('untrusted-sender');
    expect(warnings()).toContain(FOREIGN_URL);
    expect(warnings()).not.toContain(SECRET_MARKER);
  });

  it('logs a refused sender whose frame is gone', async () => {
    const harness = setUpHandlers();

    await rejectionOf(() => harness.registry.invokeFrom(null, 'recents:open', { path: `E:\\clients\\${SECRET_MARKER}` }));

    expect(warn).toHaveBeenCalledTimes(1);
    expect(warnings()).toContain('untrusted-sender');
    expect(warnings()).not.toContain(SECRET_MARKER);
  });
});

describe('payload check', () => {
  it.each(MALFORMED_CALLS)('rejects a malformed $channel payload before any use case runs', async ({ channel, payload }) => {
    const harness = setUpHandlers({ fileSystem: withOpenableAtlas(), recents: [ATLAS_RECENT] });

    const rejection = await rejectionOf(() => harness.registry.invoke(channel, payload));

    expect(rejection).toBeInstanceOf(IpcRejectedError);
    expect(rejection).toMatchObject({ reason: 'invalid-payload' });
    expect(harness.checkedSenders).toEqual([TRUSTED_URL]);
    expectNothingTouched(harness);
    expect(harness.session.currentProject).toBeNull();
  });

  it('logs the reason and the sender URL of a malformed payload, never the payload', async () => {
    const harness = setUpHandlers();

    await rejectionOf(() => harness.registry.invoke('recents:open', { path: SECRET_MARKER }));

    expect(warn).toHaveBeenCalledTimes(1);
    expect(warnings()).toContain('invalid-payload');
    expect(warnings()).toContain(TRUSTED_URL);
    expect(warnings()).not.toContain(SECRET_MARKER);
  });
});

describe('recents:list', () => {
  // covers: FCT-0010
  it('lists the ten most recent projects, newest first, with their availability', async () => {
    const fileSystem = new RecordingFileSystem().addFolder('E:\\work\\p12').addFolder('E:\\work\\p09').addFolder('E:\\work\\p01');
    const { registry } = setUpHandlers({
      fileSystem,
      recents: [
        recent('p04', '2026-10-04T08:00:00.000Z'),
        recent('p11', '2026-10-11T08:00:00.000Z'),
        recent('p01', '2026-10-01T08:00:00.000Z'),
        recent('p07', '2026-10-07T08:00:00.000Z'),
        recent('p12', '2026-10-12T08:00:00.000Z'),
        recent('p02', '2026-10-02T08:00:00.000Z'),
        recent('p09', '2026-10-09T08:00:00.000Z'),
        recent('p05', '2026-10-05T08:00:00.000Z'),
        recent('p10', '2026-10-10T08:00:00.000Z'),
        recent('p03', '2026-10-03T08:00:00.000Z'),
        recent('p08', '2026-10-08T08:00:00.000Z'),
        recent('p06', '2026-10-06T08:00:00.000Z'),
      ],
    });

    expect(await registry.invoke('recents:list')).toEqual({
      unreadable: false,
      projects: [
        { path: 'E:\\work\\p12', name: 'Projet p12', openedAt: '2026-10-12T08:00:00.000Z', available: true },
        { path: 'E:\\work\\p11', name: 'Projet p11', openedAt: '2026-10-11T08:00:00.000Z', available: false },
        { path: 'E:\\work\\p10', name: 'Projet p10', openedAt: '2026-10-10T08:00:00.000Z', available: false },
        { path: 'E:\\work\\p09', name: 'Projet p09', openedAt: '2026-10-09T08:00:00.000Z', available: true },
        { path: 'E:\\work\\p08', name: 'Projet p08', openedAt: '2026-10-08T08:00:00.000Z', available: false },
        { path: 'E:\\work\\p07', name: 'Projet p07', openedAt: '2026-10-07T08:00:00.000Z', available: false },
        { path: 'E:\\work\\p06', name: 'Projet p06', openedAt: '2026-10-06T08:00:00.000Z', available: false },
        { path: 'E:\\work\\p05', name: 'Projet p05', openedAt: '2026-10-05T08:00:00.000Z', available: false },
        { path: 'E:\\work\\p04', name: 'Projet p04', openedAt: '2026-10-04T08:00:00.000Z', available: false },
        { path: 'E:\\work\\p03', name: 'Projet p03', openedAt: '2026-10-03T08:00:00.000Z', available: false },
      ],
    });
  });

  // covers: FCT-0010
  it('reports an unreadable recent projects file with an empty list', async () => {
    const { registry } = setUpHandlers({ recents: new RecordingRecentsStore([ATLAS_RECENT], true) });

    expect(await registry.invoke('recents:list')).toEqual({ projects: [], unreadable: true });
  });

  it('never grants a recent project folder as a creation location', async () => {
    const harness = setUpHandlers({ fileSystem: withOpenableAtlas(), recents: [ATLAS_RECENT] });

    await harness.registry.invoke('recents:list');

    expect(harness.session.isLocationGranted(LIBRARY_PATH)).toBe(false);
    expect(harness.session.isLocationGranted('E:\\clients')).toBe(false);
  });
});

describe('recents:remove', () => {
  it('removes a recent project and returns the remaining ones', async () => {
    const fileSystem = new RecordingFileSystem().addFolder(NOVA_RECENT.path);
    const harness = setUpHandlers({ fileSystem, recents: [ATLAS_RECENT, NOVA_RECENT] });

    expect(await harness.registry.invoke('recents:remove', { path: LIBRARY_PATH })).toEqual([
      { ...NOVA_RECENT, available: true },
    ]);
    expect(harness.recents.current).toEqual([NOVA_RECENT]);
  });

  it('never touches a folder that is not in the recent projects', async () => {
    const harness = setUpHandlers({ fileSystem: withOpenableAtlas(), recents: [NOVA_RECENT] });

    await harness.registry.invoke('recents:remove', { path: LIBRARY_PATH }).catch((rejection: unknown) => rejection);

    expect(harness.fileSystem.touchedPaths).not.toContain(LIBRARY_PATH);
    expect(harness.recents.saves).toEqual([]);
    expect(harness.recents.current).toEqual([NOVA_RECENT]);
  });
});

describe('recents:open', () => {
  it('opens a recent project and makes it the current project', async () => {
    const harness = setUpHandlers({ fileSystem: withOpenableAtlas(), recents: [ATLAS_RECENT] });

    const outcome = await harness.registry.invoke('recents:open', { path: LIBRARY_PATH });

    expect(outcome).toMatchObject({ ...openedAtlas, project: { ...openedAtlas.project, name: 'Atlas maison' } });
    expect(harness.session.currentProject).toEqual(projectOf(outcome));
  });

  it('reports a recent project whose folder is gone', async () => {
    const harness = setUpHandlers({ recents: [ATLAS_RECENT] });

    expect(await harness.registry.invoke('recents:open', { path: LIBRARY_PATH })).toEqual({
      kind: 'missing',
      path: LIBRARY_PATH,
    });
    expect(harness.session.currentProject).toBeNull();
  });

  it('never opens a folder that is not in the recent projects, even a valid project', async () => {
    const harness = setUpHandlers({ fileSystem: withOpenableAtlas(), recents: [NOVA_RECENT] });

    const result = await harness.registry
      .invoke('recents:open', { path: LIBRARY_PATH })
      .catch((rejection: unknown) => rejection);

    expect(result).not.toMatchObject({ kind: 'opened' });
    expect(harness.fileSystem.touchedPaths).not.toContain(LIBRARY_PATH);
    expect(harness.session.currentProject).toBeNull();
    expect(harness.recents.saves).toEqual([]);
  });

  it('never grants the opened recent project folder as a creation location', async () => {
    const harness = setUpHandlers({ fileSystem: withOpenableAtlas(), recents: [ATLAS_RECENT] });

    await harness.registry.invoke('recents:open', { path: LIBRARY_PATH });

    expect(harness.session.isLocationGranted(LIBRARY_PATH)).toBe(false);
    expect(await harness.registry.invoke('project:create', { ...createRequest, location: LIBRARY_PATH })).toEqual({
      kind: 'invalid-request',
      errors: [{ field: 'location', code: 'not-granted' }],
    });
  });
});

describe('project:default-location', () => {
  it('proposes the parent folder of the latest available recent project and grants it', async () => {
    const harness = setUpHandlers({ fileSystem: withOpenableAtlas(), recents: [NOVA_RECENT, ATLAS_RECENT] });

    expect(await harness.registry.invoke('project:default-location')).toBe('E:\\clients');
    expect(harness.session.isLocationGranted('E:\\clients')).toBe(true);
    expect(harness.session.isLocationGranted(LIBRARY_PATH)).toBe(false);
  });

  it('proposes the Documents folder when no recent project is available', async () => {
    const harness = setUpHandlers({ recents: [ATLAS_RECENT] });

    expect(await harness.registry.invoke('project:default-location')).toBe(DOCUMENTS);
    expect(harness.session.isLocationGranted(DOCUMENTS)).toBe(true);
  });

  it('lets a project be created in the proposed location only once it was proposed', async () => {
    const harness = setUpHandlers({ fileSystem: withOpenableAtlas(), recents: [ATLAS_RECENT] });
    const request = { ...createRequest, location: 'E:\\clients' };

    expect(await harness.registry.invoke('project:create', request)).toEqual({
      kind: 'invalid-request',
      errors: [{ field: 'location', code: 'not-granted' }],
    });

    await harness.registry.invoke('project:default-location');

    expect(await harness.registry.invoke('project:create', request)).toMatchObject({
      kind: 'created',
      project: { path: 'E:\\clients\\kit-horizon' },
    });
  });
});

describe('project:choose-location', () => {
  it('returns the chosen folder and grants it as a creation location', async () => {
    const harness = setUpHandlers();
    harness.picker.answer('D:\\design\\kits');

    expect(await harness.registry.invoke('project:choose-location')).toEqual({ canceled: false, path: 'D:\\design\\kits' });
    expect(harness.picker.purposes).toEqual(['choose-location']);
    expect(harness.session.isLocationGranted('D:\\design\\kits')).toBe(true);
    expect(await harness.registry.invoke('project:create', { ...createRequest, location: 'D:\\design\\kits' })).toMatchObject({
      kind: 'created',
      project: { path: 'D:\\design\\kits\\kit-horizon' },
    });
  });

  it('reports a canceled dialog without granting anything', async () => {
    const harness = setUpHandlers();
    harness.picker.answer(null);

    expect(await harness.registry.invoke('project:choose-location')).toEqual({ canceled: true });
    expect(harness.picker.purposes).toEqual(['choose-location']);
    expect(harness.session.isLocationGranted('D:\\design\\kits')).toBe(false);
  });
});

describe('project:create', () => {
  // covers: FCT-0004
  it('refuses to create a project over a folder that is not empty and writes nothing', async () => {
    const fileSystem = new RecordingFileSystem();
    fileSystem
      .addFolder(DOCUMENTS)
      .addFolder(`${DOCUMENTS}\\kit-horizon`)
      .addFile(`${DOCUMENTS}\\kit-horizon\\notes.md`, '# Kit Horizon\n\nIdées de composants.\n');
    const before = fileSystem.snapshot();
    const harness = setUpHandlers({ fileSystem });

    expect(await harness.registry.invoke('project:create', createRequest)).toEqual({
      kind: 'folder-not-empty',
      path: 'C:\\Users\\Camille\\Documents\\kit-horizon',
      entryCount: 1,
    });
    expect(fileSystem.snapshot()).toEqual(before);
    expect(fileSystem.createFolderCalls).toEqual([]);
    expect(harness.session.currentProject).toBeNull();
    expect(harness.recents.saves).toEqual([]);
  });

  it('creates the project in the Documents folder and makes it the current project', async () => {
    const harness = setUpHandlers({ fileSystem: new RecordingFileSystem().addFolder(DOCUMENTS) });

    const outcome = await harness.registry.invoke('project:create', createRequest);

    expect(outcome).toEqual({
      kind: 'created',
      project: {
        path: 'C:\\Users\\Camille\\Documents\\kit-horizon',
        name: 'Kit Horizon',
        packageName: '@kit-horizon/ui',
        version: '0.1.0',
        settings: { formatVersion: 1, tagPrefix: 'kh-', componentsDir: 'src/components' },
      },
    });
    expect(harness.session.currentProject).toEqual(projectOf(outcome));
  });

  it('refuses a location that was never granted before any file system access', async () => {
    const harness = setUpHandlers({ fileSystem: new RecordingFileSystem().addFolder('D:\\design\\kits') });

    expect(await harness.registry.invoke('project:create', { ...createRequest, location: 'D:\\design\\kits' })).toEqual({
      kind: 'invalid-request',
      errors: [{ field: 'location', code: 'not-granted' }],
    });
    expectNothingTouched(harness);
    expect(harness.session.currentProject).toBeNull();
  });
});

describe('project:open-folder', () => {
  // covers: FCT-0006
  it('opens a folder that holds the UI-GEN settings file and makes it the current project', async () => {
    const harness = setUpHandlers({ fileSystem: withOpenableAtlas() });
    harness.picker.answer(LIBRARY_PATH);

    const outcome = await harness.registry.invoke('project:open-folder');

    expect(outcome).toMatchObject(openedAtlas);
    expect(harness.picker.purposes).toEqual(['open-project']);
    expect(harness.session.currentProject).toEqual(projectOf(outcome));
  });

  it('reports a canceled dialog without reading any folder', async () => {
    const harness = setUpHandlers({ fileSystem: withOpenableAtlas() });
    harness.picker.answer(null);

    expect(await harness.registry.invoke('project:open-folder')).toEqual({ kind: 'canceled' });
    expect(harness.fileSystem.touchedPaths).toEqual([]);
    expect(harness.session.currentProject).toBeNull();
  });

  it('never grants an opened project folder as a creation location', async () => {
    const harness = setUpHandlers({ fileSystem: withOpenableAtlas() });
    harness.picker.answer(LIBRARY_PATH);

    await harness.registry.invoke('project:open-folder');

    expect(harness.session.isLocationGranted(LIBRARY_PATH)).toBe(false);
  });
});

describe('project:adopt', () => {
  it('adopts the library that the last opened folder proposed and makes it the current project', async () => {
    const harness = setUpHandlers({ fileSystem: withAdoptableAtlas() });
    await openAtlasForAdoption(harness);

    const outcome = await harness.registry.invoke('project:adopt', atlasAdoption);

    expect(outcome).toMatchObject(openedAtlas);
    expect(harness.session.currentProject).toEqual(projectOf(outcome));
    expect(harness.fileSystem.fileContent(inLibrary('uigen.json'))).toBe(
      '{\n  "formatVersion": 1,\n  "tagPrefix": "at-",\n  "componentsDir": "src/lib"\n}\n',
    );
  });

  it('refuses an adoption when no opened folder proposed one', async () => {
    const harness = setUpHandlers({ fileSystem: withAdoptableAtlas() });

    expect(await harness.registry.invoke('project:adopt', atlasAdoption)).toEqual({
      kind: 'invalid-request',
      errors: [{ field: 'path', code: 'not-pending' }],
    });
    expectNothingTouched(harness);
    expect(harness.fileSystem.fileContent(inLibrary('uigen.json'))).toBeUndefined();
  });

  it('refuses to adopt another folder than the pending one', async () => {
    const harness = setUpHandlers({ fileSystem: addNovaKit(withAdoptableAtlas()) });
    await openAtlasForAdoption(harness);
    const touchedBefore = harness.fileSystem.touchedPaths.length;

    expect(await harness.registry.invoke('project:adopt', { ...atlasAdoption, path: NOVA_KIT, tagPrefix: 'nv-', componentsDir: 'src' })).toEqual({
      kind: 'invalid-request',
      errors: [{ field: 'path', code: 'not-pending' }],
    });
    expect(harness.fileSystem.touchedPaths).toHaveLength(touchedBefore);
    expect(harness.fileSystem.fileContent(`${NOVA_KIT}\\uigen.json`)).toBeUndefined();
    expect(harness.session.currentProject).toBeNull();
  });

  it('only keeps the last folder that needed an adoption as pending', async () => {
    const harness = setUpHandlers({ fileSystem: addNovaKit(withAdoptableAtlas()) });
    await openAtlasForAdoption(harness);
    harness.picker.answer(NOVA_KIT);
    expect(await harness.registry.invoke('project:open-folder')).toMatchObject({ kind: 'needs-adoption' });

    expect(await harness.registry.invoke('project:adopt', atlasAdoption)).toEqual({
      kind: 'invalid-request',
      errors: [{ field: 'path', code: 'not-pending' }],
    });
    expect(await harness.registry.invoke('project:adopt', { path: NOVA_KIT, tagPrefix: 'nv-', componentsDir: 'src' })).toMatchObject({
      kind: 'opened',
      project: { path: NOVA_KIT },
    });
  });

  it('forgets the pending adoption when the project is closed', async () => {
    const harness = setUpHandlers({ fileSystem: withAdoptableAtlas() });
    await openAtlasForAdoption(harness);

    expect(await harness.registry.invoke('project:close')).toBeUndefined();

    expect(await harness.registry.invoke('project:adopt', atlasAdoption)).toEqual({
      kind: 'invalid-request',
      errors: [{ field: 'path', code: 'not-pending' }],
    });
    expect(harness.fileSystem.fileContent(inLibrary('uigen.json'))).toBeUndefined();
  });

  it('forgets the pending adoption when another opened folder is not a Lit library', async () => {
    const harness = setUpHandlers({ fileSystem: addMarketingSite(withAdoptableAtlas()) });
    await openAtlasForAdoption(harness);
    harness.picker.answer(MARKETING_SITE);
    expect(await harness.registry.invoke('project:open-folder')).toEqual({
      kind: 'not-a-lit-library',
      path: MARKETING_SITE,
      reason: 'missing-lit-dependency',
    });

    expect(await harness.registry.invoke('project:adopt', atlasAdoption)).toEqual({
      kind: 'invalid-request',
      errors: [{ field: 'path', code: 'not-pending' }],
    });
  });

  it('forgets the pending adoption when a recent project is opened', async () => {
    const fileSystem = withAdoptableAtlas();
    fileSystem
      .addFolder(NOVA_RECENT.path)
      .addFile(`${NOVA_RECENT.path}\\package.json`, '{\n  "name": "@nova/ui",\n  "version": "1.2.0",\n  "dependencies": {\n    "lit": "^3.3.0"\n  }\n}\n')
      .addFile(`${NOVA_RECENT.path}\\uigen.json`, '{\n  "formatVersion": 1,\n  "tagPrefix": "nv-",\n  "componentsDir": "src/components"\n}\n');
    const harness = setUpHandlers({ fileSystem, recents: [NOVA_RECENT] });
    await openAtlasForAdoption(harness);
    expect(await harness.registry.invoke('recents:open', { path: NOVA_RECENT.path })).toMatchObject({ kind: 'opened' });

    expect(await harness.registry.invoke('project:adopt', atlasAdoption)).toEqual({
      kind: 'invalid-request',
      errors: [{ field: 'path', code: 'not-pending' }],
    });
  });

  it('forgets the pending adoption when a project is created', async () => {
    const harness = setUpHandlers({ fileSystem: withAdoptableAtlas().addFolder(DOCUMENTS) });
    await openAtlasForAdoption(harness);
    expect(await harness.registry.invoke('project:create', createRequest)).toMatchObject({ kind: 'created' });

    expect(await harness.registry.invoke('project:adopt', atlasAdoption)).toEqual({
      kind: 'invalid-request',
      errors: [{ field: 'path', code: 'not-pending' }],
    });
  });
});

describe('project:close', () => {
  it('closes the current project', async () => {
    const harness = setUpHandlers({ fileSystem: withOpenableAtlas() });
    harness.picker.answer(LIBRARY_PATH);
    await harness.registry.invoke('project:open-folder');

    expect(await harness.registry.invoke('project:close')).toBeUndefined();
    expect(harness.session.currentProject).toBeNull();
  });
});

describe('unexpected use case failure', () => {
  const lockedFolderCause = new Error(`EBUSY: resource busy or locked, scandir 'E:\\clients\\${SECRET_MARKER}'`);

  class LockedFileSystem extends RecordingFileSystem {
    override readFolderState(): Promise<never> {
      return Promise.reject(lockedFolderCause);
    }
  }

  const unreadableStoreCause = new Error(
    `EPERM: operation not permitted, open 'C:\\Users\\Camille\\AppData\\Roaming\\UI-GEN\\${SECRET_MARKER}.json'`,
  );

  class BrokenRecentsStore extends RecordingRecentsStore {
    override load(): Promise<RecentProjectsLoad> {
      return Promise.reject(unreadableStoreCause);
    }
  }

  function expectGenericError(rejection: unknown): void {
    expect(rejection).toBeInstanceOf(Error);
    expect(rejection).not.toBeInstanceOf(IpcRejectedError);
    const message = rejection instanceof Error ? rejection.message : '';
    expect(message).not.toContain(SECRET_MARKER);
    expect(message).not.toMatch(/[A-Za-z]:\\/);
    expect(message).not.toContain('EBUSY');
    expect(message).not.toContain('EPERM');
  }

  it('rejects with a generic error that names no path and logs the cause', async () => {
    const harness = setUpHandlers({ fileSystem: new LockedFileSystem() });
    harness.picker.answer(`E:\\clients\\${SECRET_MARKER}`);

    const rejection = await rejectionOf(() => harness.registry.invoke('project:open-folder'));

    expectGenericError(rejection);
    expect(error.mock.calls.flat()).toContain(lockedFolderCause);
    expect(harness.session.currentProject).toBeNull();
  });

  it('rejects listing the recent projects with a generic error when the store fails', async () => {
    const harness = setUpHandlers({ recents: new BrokenRecentsStore() });

    const rejection = await rejectionOf(() => harness.registry.invoke('recents:list'));

    expectGenericError(rejection);
    expect(error.mock.calls.flat()).toContain(unreadableStoreCause);
  });
});
