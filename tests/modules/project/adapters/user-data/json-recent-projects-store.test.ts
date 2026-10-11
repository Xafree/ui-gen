import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from 'vitest';
import { JsonRecentProjectsStore } from '../../../../../src/modules/project/adapters/user-data/json-recent-projects-store';
import type { RecentProject } from '../../../../../src/modules/project/domain/recents/recent-projects';

const kitHorizon: RecentProject = {
  path: 'E:\\projets\\kit-horizon',
  name: 'Kit Horizon',
  openedAt: '2026-10-11T09:30:00.000Z',
};

const designSystem: RecentProject = {
  path: 'C:\\Users\\emman\\Documents\\Design System Éditions',
  name: 'Design System Éditions',
  openedAt: '2026-10-09T17:05:12.000Z',
};

const SAVED_FILE = `{
  "formatVersion": 1,
  "projects": [
    {
      "path": "E:\\\\projets\\\\kit-horizon",
      "name": "Kit Horizon",
      "openedAt": "2026-10-11T09:30:00.000Z"
    },
    {
      "path": "C:\\\\Users\\\\emman\\\\Documents\\\\Design System Éditions",
      "name": "Design System Éditions",
      "openedAt": "2026-10-09T17:05:12.000Z"
    }
  ]
}
`;

const OLDEST_FIRST_FILE = `{
  "formatVersion": 1,
  "projects": [
    { "path": "C:\\\\Users\\\\emman\\\\Documents\\\\Design System Éditions", "name": "Design System Éditions", "openedAt": "2026-10-09T17:05:12.000Z" },
    { "path": "E:\\\\projets\\\\kit-horizon", "name": "Kit Horizon", "openedAt": "2026-10-11T09:30:00.000Z" }
  ]
}
`;

const TRUNCATED_FILE = '{\n  "formatVersion": 1,\n  "projects": [\n    { "path": "E:\\\\projets\\\\kit-hor';

let root: string;
let file: string;
let backup: string;
let warnSpy: MockInstance<typeof console.warn>;

beforeEach(async () => {
  root = await mkdtemp(join(tmpdir(), 'uigen-recents-'));
  file = join(root, 'recent-projects.json');
  backup = join(root, 'recent-projects.json.bak');
  warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(async () => {
  vi.restoreAllMocks();
  await rm(root, { recursive: true, force: true });
});

describe('JsonRecentProjectsStore.load', () => {
  it('returns no project and no warning flag when the file does not exist', async () => {
    const store = new JsonRecentProjectsStore(join(root, 'UI-GEN', 'recent-projects.json'));

    await expect(store.load()).resolves.toEqual({ projects: [], unreadable: false });
    expect(await readdir(root)).toEqual([]);
  });

  // covers: FCT-0010
  it('returns the saved projects from the most recent to the oldest', async () => {
    await writeFile(file, OLDEST_FIRST_FILE, 'utf8');

    await expect(new JsonRecentProjectsStore(file).load()).resolves.toEqual({
      projects: [kitHorizon, designSystem],
      unreadable: false,
    });
    expect(warnSpy).not.toHaveBeenCalled();
  });

  it('flags invalid JSON as unreadable after copying it to the backup file', async () => {
    await writeFile(file, TRUNCATED_FILE, 'utf8');

    await expect(new JsonRecentProjectsStore(file).load()).resolves.toEqual({ projects: [], unreadable: true });
    expect(warnSpy).toHaveBeenCalled();
    expect(await readFile(backup, 'utf8')).toBe(TRUNCATED_FILE);
  });

  it('flags a file of an unknown format as unreadable after copying it to the backup file', async () => {
    const futureFormat = '{\n  "formatVersion": 2,\n  "projects": []\n}\n';
    await writeFile(file, futureFormat, 'utf8');

    await expect(new JsonRecentProjectsStore(file).load()).resolves.toEqual({ projects: [], unreadable: true });
    expect(warnSpy).toHaveBeenCalled();
    expect(await readFile(backup, 'utf8')).toBe(futureFormat);
  });

  it('replaces a previous backup with the newly unreadable content', async () => {
    await writeFile(backup, 'previous backup', 'utf8');
    await writeFile(file, TRUNCATED_FILE, 'utf8');

    await new JsonRecentProjectsStore(file).load();

    expect(await readFile(backup, 'utf8')).toBe(TRUNCATED_FILE);
  });

  it('flags an entry that cannot be read as a file as unreadable without rejecting', async () => {
    await mkdir(file);

    await expect(new JsonRecentProjectsStore(file).load()).resolves.toEqual({ projects: [], unreadable: true });
    expect(warnSpy).toHaveBeenCalled();
  });

  it('still flags the file as unreadable without rejecting when the backup cannot be written', async () => {
    await mkdir(backup);
    await writeFile(file, TRUNCATED_FILE, 'utf8');

    await expect(new JsonRecentProjectsStore(file).load()).resolves.toEqual({ projects: [], unreadable: true });
    expect(warnSpy).toHaveBeenCalled();
  });

  it('keeps the unreadable content in the backup after a later save', async () => {
    await writeFile(file, TRUNCATED_FILE, 'utf8');
    const store = new JsonRecentProjectsStore(file);

    await store.load();
    await store.save([kitHorizon]);

    expect(await readFile(backup, 'utf8')).toBe(TRUNCATED_FILE);
  });
});

describe('JsonRecentProjectsStore.save', () => {
  // covers: FCT-0010
  it('creates the missing parent folders and writes the serialized projects', async () => {
    const nestedFile = join(root, 'UI-GEN', 'state', 'recent-projects.json');

    await new JsonRecentProjectsStore(nestedFile).save([kitHorizon, designSystem]);

    expect(await readFile(nestedFile, 'utf8')).toBe(SAVED_FILE);
    expect(await readdir(join(root, 'UI-GEN', 'state'))).toEqual(['recent-projects.json']);
  });

  it('replaces the previous file and leaves no temporary file', async () => {
    await writeFile(file, TRUNCATED_FILE, 'utf8');

    await new JsonRecentProjectsStore(file).save([kitHorizon, designSystem]);

    expect(await readFile(file, 'utf8')).toBe(SAVED_FILE);
    expect(await readdir(root)).toEqual(['recent-projects.json']);
  });

  // covers: FCT-0010
  it('gives back on load what it saved', async () => {
    const store = new JsonRecentProjectsStore(file);

    await store.save([kitHorizon, designSystem]);

    await expect(store.load()).resolves.toEqual({ projects: [kitHorizon, designSystem], unreadable: false });
  });

  it('warns instead of rejecting and leaves no temporary file when a folder holds the file name', async () => {
    await mkdir(file);

    await expect(new JsonRecentProjectsStore(file).save([kitHorizon])).resolves.toBeUndefined();
    expect(warnSpy).toHaveBeenCalled();
    expect(await readdir(root)).toEqual(['recent-projects.json']);
    expect(await readdir(file)).toEqual([]);
  });

  it('warns instead of rejecting when a file stands where the parent folder is expected', async () => {
    await writeFile(join(root, 'UI-GEN'), 'not a folder', 'utf8');

    await expect(
      new JsonRecentProjectsStore(join(root, 'UI-GEN', 'recent-projects.json')).save([kitHorizon]),
    ).resolves.toBeUndefined();
    expect(warnSpy).toHaveBeenCalled();
    expect(await readdir(root)).toEqual(['UI-GEN']);
    expect(await readFile(join(root, 'UI-GEN'), 'utf8')).toBe('not a folder');
  });
});
