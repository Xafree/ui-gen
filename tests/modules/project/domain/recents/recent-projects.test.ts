import { describe, expect, it } from 'vitest';
import {
  MAX_RECENT_PROJECTS,
  RECENT_PROJECTS_FILE,
  findRecentProject,
  parentFolderOf,
  parseRecentProjects,
  recordRecentProject,
  removeRecentProject,
  samePath,
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

const nova: RecentProject = {
  path: 'C:\\Users\\Camille\\Documents\\nova-ui',
  name: 'Nova UI',
  openedAt: '2026-09-28T07:45:00.000Z',
};

const recentsFile = `{
  "formatVersion": 1,
  "projects": [
    {
      "path": "E:\\\\projets\\\\kit-horizon",
      "name": "Kit Horizon",
      "openedAt": "2026-10-11T09:30:00.000Z"
    },
    {
      "path": "E:\\\\clients\\\\atlas-design-system",
      "name": "Atlas",
      "openedAt": "2026-10-09T16:05:12.000Z"
    },
    {
      "path": "C:\\\\Users\\\\Camille\\\\Documents\\\\nova-ui",
      "name": "Nova UI",
      "openedAt": "2026-09-28T07:45:00.000Z"
    }
  ]
}
`;

function projectOpenedOnDay(day: number): RecentProject {
  const dd = String(day).padStart(2, '0');
  return {
    path: `E:\\projets\\kit-${dd}`,
    name: `Kit ${dd}`,
    openedAt: `2026-09-${dd}T10:00:00.000Z`,
  };
}

function namesOf(projects: readonly RecentProject[]): string[] {
  return projects.map((project) => project.name);
}

describe('recent projects constants', () => {
  // covers: FCT-0010
  it('keeps at most ten recent projects', () => {
    expect(MAX_RECENT_PROJECTS).toBe(10);
  });

  // covers: FCT-0010
  it('stores recent projects in recent-projects.json', () => {
    expect(RECENT_PROJECTS_FILE).toBe('recent-projects.json');
  });
});

describe('samePath', () => {
  // covers: FCT-0010
  it('treats Windows paths that differ only by case as the same path', () => {
    expect(samePath('E:\\Projets\\Kit-Horizon', 'e:\\projets\\kit-horizon')).toBe(true);
  });

  // covers: FCT-0010
  it('tells two different folders apart', () => {
    expect(samePath('E:\\projets\\kit-horizon', 'E:\\projets\\kit-horizon-2')).toBe(false);
    expect(samePath('E:\\projets\\kit-horizon', 'D:\\projets\\kit-horizon')).toBe(false);
  });
});

describe('parseRecentProjects', () => {
  // covers: FCT-0010
  it('reads a valid file into recent projects, most recent first', () => {
    expect(parseRecentProjects(JSON.parse(recentsFile))).toEqual([kitHorizon, atlas, nova]);
  });

  // covers: FCT-0010
  it('orders the projects from the most recent to the oldest whatever the file order', () => {
    const file = { formatVersion: 1, projects: [nova, kitHorizon, atlas] };
    expect(parseRecentProjects(file)).toEqual([kitHorizon, atlas, nova]);
  });

  // covers: FCT-0010
  it('reads an empty list', () => {
    expect(parseRecentProjects({ formatVersion: 1, projects: [] })).toEqual([]);
  });

  // covers: FCT-0010
  it('keeps only the ten most recent projects of a longer file', () => {
    const projects = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12].map(projectOpenedOnDay);
    const parsed = parseRecentProjects({ formatVersion: 1, projects });
    expect(namesOf(parsed ?? [])).toEqual([
      'Kit 12',
      'Kit 11',
      'Kit 10',
      'Kit 09',
      'Kit 08',
      'Kit 07',
      'Kit 06',
      'Kit 05',
      'Kit 04',
      'Kit 03',
    ]);
  });

  // covers: FCT-0010
  it.each([
    ['null', null],
    ['a string', 'recent projects'],
    ['an array', [kitHorizon]],
    ['an object without projects', { formatVersion: 1 }],
    ['an object without formatVersion', { projects: [kitHorizon] }],
    ['an unknown format version', { formatVersion: 2, projects: [kitHorizon] }],
    ['a format version given as a string', { formatVersion: '1', projects: [kitHorizon] }],
    ['projects that are not a list', { formatVersion: 1, projects: { kitHorizon } }],
  ])('rejects %s as an unreadable file', (_label, value) => {
    expect(parseRecentProjects(value)).toBeNull();
  });

  // covers: FCT-0010
  it.each([
    ['an entry that is not an object', 'E:\\projets\\kit-horizon'],
    ['an entry without path', { name: 'Kit Horizon', openedAt: kitHorizon.openedAt }],
    ['an entry whose path is not a string', { ...kitHorizon, path: 42 }],
    ['an entry without name', { path: kitHorizon.path, openedAt: kitHorizon.openedAt }],
    ['an entry whose name is not a string', { ...kitHorizon, name: null }],
    ['an entry without openedAt', { path: kitHorizon.path, name: kitHorizon.name }],
    ['an entry whose openedAt is a number', { ...kitHorizon, openedAt: 1760175000000 }],
    ['an entry whose openedAt is not a date', { ...kitHorizon, openedAt: 'last tuesday' }],
  ])('rejects the whole file when it holds %s', (_label, entry) => {
    expect(parseRecentProjects({ formatVersion: 1, projects: [atlas, entry] })).toBeNull();
  });
});

describe('serializeRecentProjects', () => {
  // covers: FCT-0010
  it('writes the file format indented by two spaces with a final line break', () => {
    expect(serializeRecentProjects([kitHorizon, atlas, nova])).toBe(recentsFile);
  });

  // covers: FCT-0010
  it('writes an empty list', () => {
    expect(serializeRecentProjects([])).toBe('{\n  "formatVersion": 1,\n  "projects": []\n}\n');
  });

  // covers: FCT-0010
  it('round-trips through parseRecentProjects', () => {
    const written = serializeRecentProjects([kitHorizon, atlas, nova]);
    expect(parseRecentProjects(JSON.parse(written))).toEqual([kitHorizon, atlas, nova]);
  });
});

describe('recordRecentProject', () => {
  // covers: FCT-0010
  it('puts a newly opened project at the top of the list', () => {
    const opened: RecentProject = {
      path: 'E:\\projets\\orbite',
      name: 'Orbite',
      openedAt: '2026-10-11T11:00:00.000Z',
    };
    expect(recordRecentProject([kitHorizon, atlas, nova], opened)).toEqual([
      opened,
      kitHorizon,
      atlas,
      nova,
    ]);
  });

  // covers: FCT-0010
  it('records the first project in an empty list', () => {
    expect(recordRecentProject([], kitHorizon)).toEqual([kitHorizon]);
  });

  // covers: FCT-0010
  it('replaces the entry of a reopened project instead of listing it twice', () => {
    const reopened: RecentProject = { ...nova, openedAt: '2026-10-11T12:00:00.000Z' };
    expect(recordRecentProject([kitHorizon, atlas, nova], reopened)).toEqual([
      reopened,
      kitHorizon,
      atlas,
    ]);
  });

  // covers: FCT-0010
  it('recognises a reopened project whose path differs only by case', () => {
    const reopened: RecentProject = {
      path: 'e:\\Clients\\Atlas-Design-System',
      name: 'Atlas',
      openedAt: '2026-10-11T12:00:00.000Z',
    };
    expect(recordRecentProject([kitHorizon, atlas, nova], reopened)).toEqual([
      reopened,
      kitHorizon,
      nova,
    ]);
  });

  // covers: FCT-0010
  it('keeps the list ordered from the most recent to the oldest', () => {
    const middle: RecentProject = {
      path: 'E:\\projets\\orbite',
      name: 'Orbite',
      openedAt: '2026-10-01T08:00:00.000Z',
    };
    expect(namesOf(recordRecentProject([kitHorizon, atlas, nova], middle))).toEqual([
      'Kit Horizon',
      'Atlas',
      'Orbite',
      'Nova UI',
    ]);
  });

  // covers: FCT-0010
  it('drops the oldest project when an eleventh one is opened', () => {
    const tenProjects = [10, 9, 8, 7, 6, 5, 4, 3, 2, 1].map(projectOpenedOnDay);
    const recorded = recordRecentProject(tenProjects, kitHorizon);
    expect(namesOf(recorded)).toEqual([
      'Kit Horizon',
      'Kit 10',
      'Kit 09',
      'Kit 08',
      'Kit 07',
      'Kit 06',
      'Kit 05',
      'Kit 04',
      'Kit 03',
      'Kit 02',
    ]);
  });

  // covers: FCT-0010
  it('leaves the given list untouched', () => {
    const before = [kitHorizon, atlas];
    recordRecentProject(before, nova);
    expect(before).toEqual([kitHorizon, atlas]);
  });
});

describe('removeRecentProject', () => {
  // covers: FCT-0011
  it('removes the project from the list and keeps the others in order', () => {
    expect(removeRecentProject([kitHorizon, atlas, nova], atlas.path)).toEqual([kitHorizon, nova]);
  });

  // covers: FCT-0011
  it('removes a project whose path differs only by case', () => {
    expect(removeRecentProject([kitHorizon, atlas, nova], 'E:\\PROJETS\\KIT-HORIZON')).toEqual([
      atlas,
      nova,
    ]);
  });

  // covers: FCT-0011
  it('returns the same projects when the path is not in the list', () => {
    expect(removeRecentProject([kitHorizon, atlas], nova.path)).toEqual([kitHorizon, atlas]);
  });

  // covers: FCT-0011
  it('leaves the given list untouched', () => {
    const before = [kitHorizon, atlas];
    removeRecentProject(before, kitHorizon.path);
    expect(before).toEqual([kitHorizon, atlas]);
  });
});

describe('findRecentProject', () => {
  // covers: FCT-0011
  it('finds a recent project by its path', () => {
    expect(findRecentProject([kitHorizon, atlas, nova], atlas.path)).toEqual(atlas);
  });

  // covers: FCT-0011
  it('finds a recent project whose path differs only by case', () => {
    expect(findRecentProject([kitHorizon, atlas, nova], 'c:\\users\\camille\\documents\\NOVA-UI')).toEqual(nova);
  });

  // covers: FCT-0011
  it('returns undefined for a path that is not in the list', () => {
    expect(findRecentProject([kitHorizon, atlas], nova.path)).toBeUndefined();
    expect(findRecentProject([], nova.path)).toBeUndefined();
  });
});

describe('parentFolderOf', () => {
  it('gives the folder that holds a project', () => {
    expect(parentFolderOf('E:\\projets\\kit-horizon')).toBe('E:\\projets');
  });

  it('cuts at the last separator of a deep path', () => {
    expect(parentFolderOf('C:\\Users\\Camille\\Documents\\nova-ui')).toBe('C:\\Users\\Camille\\Documents');
  });

  it('accepts forward slashes as separators', () => {
    expect(parentFolderOf('E:/clients/atlas-design-system')).toBe('E:/clients');
  });

  it('has no parent to propose for a folder at the root of a drive', () => {
    expect(parentFolderOf('E:\\kit')).toBeNull();
    expect(parentFolderOf('E:/kit')).toBeNull();
  });

  it('has no parent to propose for a path without separator', () => {
    expect(parentFolderOf('kit-horizon')).toBeNull();
    expect(parentFolderOf('')).toBeNull();
  });
});
