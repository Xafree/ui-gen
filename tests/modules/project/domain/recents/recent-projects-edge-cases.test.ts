import { describe, expect, it } from 'vitest';
import {
  parseRecentProjects,
  serializeRecentProjects,
  type RecentProject,
} from '../../../../../src/modules/project/domain/recents/recent-projects';

const kitHorizon: RecentProject = {
  path: 'E:\\projets\\kit-horizon',
  name: 'Kit Horizon',
  openedAt: '2026-10-11T09:30:00.000Z',
};

const atlas: RecentProject = {
  path: 'E:\\clients\\atlas-design-system',
  name: 'Atlas',
  openedAt: '2026-10-09T16:05:12.000Z',
};

const kitHorizonFile = `{
  "formatVersion": 1,
  "projects": [
    {
      "path": "E:\\\\projets\\\\kit-horizon",
      "name": "Kit Horizon",
      "openedAt": "2026-10-11T09:30:00.000Z"
    }
  ]
}
`;

describe('parseRecentProjects on malformed entries', () => {
  // covers: FCT-0010
  it.each([
    ['an entry that is null', null],
    ['an entry whose path is empty', { ...kitHorizon, path: '' }],
    ['an entry whose openedAt is a number that reads as a year', { ...kitHorizon, openedAt: 2026 }],
  ])('rejects the whole file when it holds %s', (_label, entry) => {
    expect(parseRecentProjects({ formatVersion: 1, projects: [atlas, entry] })).toBeNull();
  });
});

describe('serializeRecentProjects on projects that carry more than the file format', () => {
  // covers: FCT-0010
  it('writes only the path, the name and the opening date of each project', () => {
    const withExtraField = { ...kitHorizon, missing: true };
    expect(serializeRecentProjects([withExtraField])).toBe(kitHorizonFile);
  });

  // covers: FCT-0010
  it('writes the fields in the order of the file format whatever the order of the given project', () => {
    const reordered: RecentProject = {
      openedAt: kitHorizon.openedAt,
      name: kitHorizon.name,
      path: kitHorizon.path,
    };
    expect(serializeRecentProjects([reordered])).toBe(kitHorizonFile);
  });
});
