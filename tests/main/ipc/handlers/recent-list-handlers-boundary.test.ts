import { format } from 'node:util';
import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from 'vitest';
import { IpcRejectedError } from '../../../../src/main/ipc/payloads';
import { readRestorePayload } from '../../../../src/main/ipc/recent-list-handlers';
import type { RecentProject } from '../../../../src/modules/project/domain/recents/recent-projects';
import { RecordingRecentsStore, TRUSTED_URL, rejectionOf, setUpHandlers } from './handler-harness';

const SECRET_MARKER = 'top-secret-roadmap';

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

const stranger: RecentProject = {
  path: 'C:\\Windows\\System32',
  name: 'System32',
  openedAt: '2026-10-11T08:00:00.000Z',
};

class UnsavableRecentsStore extends RecordingRecentsStore {
  override save(): Promise<void> {
    return Promise.reject(new Error(`disk full while writing E:\\clients\\${SECRET_MARKER}`));
  }
}

interface ConsoleSpy {
  readonly mock: { readonly calls: readonly (readonly unknown[])[] };
}

let warn: MockInstance<typeof console.warn>;
let error: MockInstance<typeof console.error>;

function logged(spy: ConsoleSpy): string {
  return spy.mock.calls.map((call) => format(...call)).join('\n');
}

function listWithAHole(): unknown[] {
  const list: unknown[] = [];
  list.length = 1;
  return list;
}

beforeEach(() => {
  warn = vi.spyOn(console, 'warn').mockReturnValue(undefined);
  error = vi.spyOn(console, 'error').mockReturnValue(undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('readRestorePayload', () => {
  // covers: FCT-0023
  it.each([
    ['without milliseconds', '2026-10-01T08:30:00Z'],
    ['with an offset', '2026-10-01T10:30:00.000+02:00'],
  ])('accepts an ISO date %s', (_label, openedAt) => {
    expect(readRestorePayload({ projects: [{ ...pivot, openedAt }] })).toEqual({ projects: [{ ...pivot, openedAt }] });
  });

  it.each([
    ['a project with an extra field', { projects: [{ ...pivot, available: false }] }],
    ['a date that does not exist', { projects: [{ ...pivot, openedAt: '2026-13-45T08:30:00.000Z' }] }],
    ['a date followed by other text', { projects: [{ ...pivot, openedAt: '2026-10-01T08:30:00.000Z later' }] }],
    ['a list with a hole', { projects: listWithAHole() }],
    ['a project given as an array', { projects: [[pivot.path, pivot.name, pivot.openedAt]] }],
  ])('refuses %s', (_label, payload) => {
    expect(readRestorePayload(payload)).toBeNull();
  });
});

describe('recents:restore', () => {
  it('logs a refusal with its reason and sender, never the project it carried', async () => {
    const harness = setUpHandlers({ recents: [horizon] });

    await rejectionOf(() => harness.registry.invoke('recents:restore', { projects: [{ ...pivot, name: SECRET_MARKER }] }));

    expect(warn).toHaveBeenCalledTimes(1);
    expect(logged(warn)).toContain('IPC recents:restore refused (invalid-payload)');
    expect(logged(warn)).toContain(TRUSTED_URL);
    expect(logged(warn)).not.toContain(SECRET_MARKER);
    expect(logged(warn)).not.toContain(pivot.path);
  });

  // covers: FCT-0023
  it('refuses a project whose path was asked for removal without being in the list', async () => {
    const harness = setUpHandlers({ recents: [horizon] });
    await harness.registry.invoke('recents:remove', { path: stranger.path });

    const rejection = await rejectionOf(() => harness.registry.invoke('recents:restore', { projects: [stranger] }));

    expect(rejection).toBeInstanceOf(IpcRejectedError);
    expect(rejection).toMatchObject({ reason: 'invalid-payload' });
    expect(harness.recents.current).toEqual([horizon]);
  });

  // covers: FCT-0023
  it('restores a project removed with another letter case of its path, as it was stored', async () => {
    const harness = setUpHandlers({ recents: [horizon] });
    await harness.registry.invoke('recents:remove', { path: 'e:\\PROJETS\\Kit-Horizon' });
    expect(harness.recents.current).toEqual([]);

    const restored = await harness.registry.invoke('recents:restore', { projects: [horizon] });

    expect(restored).toEqual([{ ...horizon, available: false }]);
    expect(harness.recents.current).toEqual([horizon]);
  });
});

describe('recents:clear', () => {
  // covers: FCT-0022
  it('fails with a generic error and forgets nothing when the empty list cannot be saved', async () => {
    const harness = setUpHandlers({ recents: new UnsavableRecentsStore([horizon, pivot]) });

    const failure = await rejectionOf(() => harness.registry.invoke('recents:clear'));

    expect(failure).not.toBeInstanceOf(IpcRejectedError);
    expect(failure).toMatchObject({ message: 'The request failed unexpectedly' });
    expect(error).toHaveBeenCalledTimes(1);
    expect(logged(error)).toContain('IPC recents:clear failed');
    const refusal = await rejectionOf(() => harness.registry.invoke('recents:restore', { projects: [pivot] }));
    expect(refusal).toMatchObject({ reason: 'invalid-payload' });
  });
});
