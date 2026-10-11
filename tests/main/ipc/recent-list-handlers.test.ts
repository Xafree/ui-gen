import { describe, expect, it } from 'vitest';
import { IpcRejectedError } from '../../../src/main/ipc/payloads';
import { readRestorePayload, registerRecentListHandlers } from '../../../src/main/ipc/recent-list-handlers';
import { ForgottenRecents } from '../../../src/main/session/forgotten-recents';
import type { RecentProject } from '../../../src/modules/project/domain/recents/recent-projects';
import { NOW } from '../../modules/project/application/open/lit-library-fixtures';
import {
  FOREIGN_URL,
  FakeIpcRegistry,
  RecordingFileSystem,
  RecordingRecentsStore,
  TRUSTED_URL,
  rejectionOf,
} from './handlers/handler-harness';

const horizon: RecentProject = {
  path: 'E:\\projets\\kit-horizon',
  name: 'Kit Horizon',
  openedAt: '2026-10-10T11:00:00.000Z',
};

const pivot: RecentProject = {
  path: 'D:\\archives\\pivot-ui',
  name: 'Prototype Pivot',
  openedAt: '2026-10-01T08:30:00.000Z',
};

const nova: RecentProject = {
  path: '\\\\nas-atelier\\partage\\nova-ui',
  name: 'Nova UI',
  openedAt: '2026-09-28T07:45:00.000Z',
};

interface RecentListHarness {
  readonly registry: FakeIpcRegistry;
  readonly fileSystem: RecordingFileSystem;
  readonly recents: RecordingRecentsStore;
  readonly checkedSenders: string[];
}

function setUp(projects: readonly RecentProject[], forgotten = new ForgottenRecents()): RecentListHarness {
  const registry = new FakeIpcRegistry();
  const fileSystem = new RecordingFileSystem().addFolder(horizon.path).addFolder(nova.path);
  const recents = new RecordingRecentsStore(projects);
  const checkedSenders: string[] = [];
  registerRecentListHandlers(registry, {
    project: { fileSystem, recentProjects: recents, clock: () => new Date(NOW) },
    forgotten,
    isTrustedSender: (senderUrl: string) => {
      checkedSenders.push(senderUrl);
      return senderUrl === TRUSTED_URL;
    },
  });
  return { registry, fileSystem, recents, checkedSenders };
}

function entries(count: number): RecentProject[] {
  return Array.from({ length: count }, (_, index) => {
    const day = String(index + 1).padStart(2, '0');
    return { path: `E:\\projets\\lib-${day}`, name: `Lib ${day}`, openedAt: `2026-09-${day}T08:00:00.000Z` };
  });
}

describe('readRestorePayload', () => {
  // covers: FCT-0022
  it('reads the projects to restore', () => {
    expect(readRestorePayload({ projects: [horizon, pivot, nova] })).toEqual({ projects: [horizon, pivot, nova] });
  });

  // covers: FCT-0022
  it('accepts ten projects', () => {
    expect(readRestorePayload({ projects: entries(10) })).toEqual({ projects: entries(10) });
  });

  // covers: FCT-0022
  it('accepts a name of 200 characters', () => {
    const name = 'N'.repeat(200);

    expect(readRestorePayload({ projects: [{ ...pivot, name }] })).toEqual({ projects: [{ ...pivot, name }] });
  });

  it.each([
    ['a string', 'D:\\archives\\pivot-ui'],
    ['null', null],
    ['an array of projects', [pivot]],
    ['an object without projects', {}],
    ['an object with another key', { projects: [pivot], force: true }],
    ['projects that are not an array', { projects: pivot }],
    ['eleven projects', { projects: entries(11) }],
    ['a project that is not an object', { projects: ['D:\\archives\\pivot-ui'] }],
    ['a relative path', { projects: [{ ...pivot, path: 'archives\\pivot-ui' }] }],
    ['an empty path', { projects: [{ ...pivot, path: '' }] }],
    ['a device namespace path', { projects: [{ ...pivot, path: '\\\\.\\PhysicalDrive0' }] }],
    ['a path with a NUL character', { projects: [{ ...pivot, path: 'D:\\archives\\pivot-ui\u0000.txt' }] }],
    ['a name of 201 characters', { projects: [{ ...pivot, name: 'N'.repeat(201) }] }],
    ['a name that is not a string', { projects: [{ ...pivot, name: 42 }] }],
    ['a missing name', { projects: [{ path: pivot.path, openedAt: pivot.openedAt }] }],
    ['a date that is not a date', { projects: [{ ...pivot, openedAt: 'hier' }] }],
    ['a date that is not ISO', { projects: [{ ...pivot, openedAt: '10/01/2026 08:30' }] }],
    ['a date that is a number', { projects: [{ ...pivot, openedAt: 1_790_000_000_000 }] }],
  ])('refuses %s', (_label, payload) => {
    expect(readRestorePayload(payload)).toBeNull();
  });
});

describe('registerRecentListHandlers', () => {
  it('registers the remove, clear and restore channels', () => {
    const { registry } = setUp([]);

    expect(registry.channels.toSorted()).toEqual(['recents:clear', 'recents:remove', 'recents:restore']);
  });
});

describe('recents:clear', () => {
  // covers: FCT-0022
  it('empties the saved list and returns no project, without touching any folder', async () => {
    const harness = setUp([horizon, pivot, nova]);

    expect(await harness.registry.invoke('recents:clear')).toEqual([]);
    expect(harness.recents.current).toEqual([]);
    expect(harness.fileSystem.touchedPaths).toEqual([]);
  });

  // covers: FCT-0022
  it('lets the cleared list be restored, each project at its place', async () => {
    const harness = setUp([horizon, pivot, nova]);
    await harness.registry.invoke('recents:clear');

    const restored = await harness.registry.invoke('recents:restore', { projects: [horizon, pivot, nova] });

    expect(restored).toEqual([
      { ...horizon, available: true },
      { ...pivot, available: false },
      { ...nova, available: true },
    ]);
    expect(harness.recents.current).toEqual([horizon, pivot, nova]);
  });

  it('refuses an untrusted sender and keeps the list', async () => {
    const harness = setUp([horizon, pivot]);

    const rejection = await rejectionOf(() => harness.registry.invokeFrom({ url: FOREIGN_URL }, 'recents:clear'));

    expect(rejection).toBeInstanceOf(IpcRejectedError);
    expect(rejection).toMatchObject({ reason: 'untrusted-sender' });
    expect(harness.recents.loadCount).toBe(0);
    expect(harness.recents.saves).toEqual([]);
  });
});

describe('recents:remove', () => {
  // covers: FCT-0023
  it('lets a removed project be restored at its place', async () => {
    const harness = setUp([horizon, pivot, nova]);
    expect(await harness.registry.invoke('recents:remove', { path: pivot.path })).toEqual([
      { ...horizon, available: true },
      { ...nova, available: true },
    ]);

    const restored = await harness.registry.invoke('recents:restore', { projects: [pivot] });

    expect(restored).toEqual([
      { ...horizon, available: true },
      { ...pivot, available: false },
      { ...nova, available: true },
    ]);
    expect(harness.recents.current).toEqual([horizon, pivot, nova]);
  });

  // covers: FCT-0023
  it('still restores an earlier removal after the list was cleared', async () => {
    const harness = setUp([horizon, pivot, nova]);
    await harness.registry.invoke('recents:remove', { path: pivot.path });
    await harness.registry.invoke('recents:clear');

    const restored = await harness.registry.invoke('recents:restore', { projects: [pivot] });

    expect(restored).toEqual([{ ...pivot, available: false }]);
  });
});

describe('recents:restore', () => {
  // covers: FCT-0023
  it('refuses a project that was never removed nor cleared during the session', async () => {
    const harness = setUp([horizon]);

    const rejection = await rejectionOf(() => harness.registry.invoke('recents:restore', { projects: [pivot] }));

    expect(rejection).toBeInstanceOf(IpcRejectedError);
    expect(rejection).toMatchObject({ reason: 'invalid-payload' });
    expect(harness.recents.saves).toEqual([]);
    expect(harness.recents.current).toEqual([horizon]);
  });

  // covers: FCT-0023
  it('refuses a removed project whose opening date was changed', async () => {
    const harness = setUp([horizon, pivot]);
    await harness.registry.invoke('recents:remove', { path: pivot.path });
    const savesAfterRemoval = harness.recents.saves.length;

    const rejection = await rejectionOf(() =>
      harness.registry.invoke('recents:restore', { projects: [{ ...pivot, openedAt: '2026-10-11T09:30:00.000Z' }] }),
    );

    expect(rejection).toMatchObject({ reason: 'invalid-payload' });
    expect(harness.recents.saves).toHaveLength(savesAfterRemoval);
    expect(harness.recents.current).toEqual([horizon]);
  });

  // covers: FCT-0022
  it('refuses the whole request when one project was not forgotten', async () => {
    const harness = setUp([horizon, pivot]);
    await harness.registry.invoke('recents:remove', { path: pivot.path });
    const savesAfterRemoval = harness.recents.saves.length;

    const rejection = await rejectionOf(() => harness.registry.invoke('recents:restore', { projects: [pivot, nova] }));

    expect(rejection).toMatchObject({ reason: 'invalid-payload' });
    expect(harness.recents.saves).toHaveLength(savesAfterRemoval);
    expect(harness.recents.current).toEqual([horizon]);
  });

  it('refuses a malformed request before reading the list', async () => {
    const harness = setUp([horizon]);

    const rejection = await rejectionOf(() => harness.registry.invoke('recents:restore', { projects: pivot }));

    expect(rejection).toBeInstanceOf(IpcRejectedError);
    expect(rejection).toMatchObject({ reason: 'invalid-payload' });
    expect(harness.recents.loadCount).toBe(0);
  });

  it('refuses an untrusted sender, even for a forgotten project', async () => {
    const forgotten = new ForgottenRecents();
    forgotten.remember([pivot]);
    const harness = setUp([horizon], forgotten);

    const rejection = await rejectionOf(() =>
      harness.registry.invokeFrom({ url: FOREIGN_URL }, 'recents:restore', { projects: [pivot] }),
    );

    expect(rejection).toMatchObject({ reason: 'untrusted-sender' });
    expect(harness.checkedSenders).toEqual([FOREIGN_URL]);
    expect(harness.recents.loadCount).toBe(0);
    expect(harness.recents.current).toEqual([horizon]);
  });

  it('refuses a request without sender frame', async () => {
    const forgotten = new ForgottenRecents();
    forgotten.remember([pivot]);
    const harness = setUp([horizon], forgotten);

    const rejection = await rejectionOf(() => harness.registry.invokeFrom(null, 'recents:restore', { projects: [pivot] }));

    expect(rejection).toMatchObject({ reason: 'untrusted-sender' });
    expect(harness.recents.current).toEqual([horizon]);
  });
});
