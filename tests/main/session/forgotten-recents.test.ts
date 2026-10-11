import { describe, expect, it } from 'vitest';
import { ForgottenRecents } from '../../../src/main/session/forgotten-recents';
import type { RecentProject } from '../../../src/modules/project/domain/recents/recent-projects';

const pivot: RecentProject = {
  path: 'D:\\archives\\pivot-ui',
  name: 'Prototype Pivot',
  openedAt: '2026-10-01T08:30:00.000Z',
};

const horizon: RecentProject = {
  path: 'E:\\projets\\kit-horizon',
  name: 'Kit Horizon',
  openedAt: '2026-10-10T11:00:00.000Z',
};

const nova: RecentProject = {
  path: 'C:\\Users\\Camille\\Documents\\nova-ui',
  name: 'Nova UI',
  openedAt: '2026-09-28T07:45:00.000Z',
};

describe('ForgottenRecents', () => {
  // covers: FCT-0023
  it('forgets nothing at the start of a session', () => {
    const forgotten = new ForgottenRecents();

    expect(forgotten.isForgotten(pivot)).toBe(false);
  });

  // covers: FCT-0023
  it('recognizes an entry removed during the session', () => {
    const forgotten = new ForgottenRecents();

    forgotten.remember([pivot]);

    expect(forgotten.isForgotten({ ...pivot })).toBe(true);
    expect(forgotten.isForgotten(horizon)).toBe(false);
  });

  // covers: FCT-0022
  it('keeps every entry forgotten earlier when the list is cleared afterwards', () => {
    const forgotten = new ForgottenRecents();

    forgotten.remember([pivot]);
    forgotten.remember([horizon, nova]);

    expect([pivot, horizon, nova].map((project) => forgotten.isForgotten(project))).toEqual([true, true, true]);
  });

  // covers: FCT-0023
  it('does not recognize an entry whose name differs from the forgotten one', () => {
    const forgotten = new ForgottenRecents();
    forgotten.remember([pivot]);

    expect(forgotten.isForgotten({ ...pivot, name: 'Pivot renommé' })).toBe(false);
  });

  // covers: FCT-0023
  it('does not recognize an entry whose opening date differs from the forgotten one', () => {
    const forgotten = new ForgottenRecents();
    forgotten.remember([pivot]);

    expect(forgotten.isForgotten({ ...pivot, openedAt: '2026-10-11T09:30:00.000Z' })).toBe(false);
  });

  // covers: FCT-0023
  it('does not recognize an entry whose path differs from the forgotten one', () => {
    const forgotten = new ForgottenRecents();
    forgotten.remember([pivot]);

    expect(forgotten.isForgotten({ ...pivot, path: 'C:\\Windows\\System32' })).toBe(false);
  });

  // covers: FCT-0022
  it('remembers nothing from an empty clearing', () => {
    const forgotten = new ForgottenRecents();

    forgotten.remember([]);

    expect(forgotten.isForgotten(horizon)).toBe(false);
  });
});
