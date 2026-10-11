import { describe, expect, it } from 'vitest';
import { readRestorePayload } from '../../../../src/main/ipc/recent-list-handlers';
import type { RecentProject } from '../../../../src/modules/project/domain/recents/recent-projects';

const pivot: RecentProject = {
  path: 'D:\\archives\\pivot-ui',
  name: 'Prototype Pivot',
  openedAt: '2026-10-01T08:30:00.000Z',
};

function restoring(openedAt: string): { projects: RecentProject[] } {
  return { projects: [{ ...pivot, openedAt }] };
}

describe('readRestorePayload dates', () => {
  // covers: FCT-0023
  it.each([
    ['minutes only and no zone', '2026-10-01T08:30'],
    ['seconds and no zone', '2026-10-01T08:30:00'],
    ['minutes only in UTC', '2026-10-01T08:30Z'],
    ['nine fraction digits', '2026-10-01T08:30:00.123456789Z'],
    ['a negative offset', '2026-10-01T03:30:00-05:00'],
  ])('accepts an ISO date with %s', (_label, openedAt) => {
    expect(readRestorePayload(restoring(openedAt))).toEqual(restoring(openedAt));
  });

  it.each([
    ['no time', '2026-10-01'],
    ['a space instead of the T separator', '2026-10-01 08:30:00'],
    ['ten fraction digits', '2026-10-01T08:30:00.1234567890Z'],
    ['a fraction mark without digits', '2026-10-01T08:30:00.Z'],
    ['an offset without colon', '2026-10-01T08:30:00+0200'],
    ['a lowercase UTC mark', '2026-10-01T08:30:00z'],
    ['a trailing newline', '2026-10-01T08:30:00Z\n'],
    ['a leading space', ' 2026-10-01T08:30:00Z'],
  ])('refuses an ISO-like date with %s', (_label, openedAt) => {
    expect(readRestorePayload(restoring(openedAt))).toBeNull();
  });
});
