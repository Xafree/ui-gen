import { mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { JsonLastOpenedStore } from '../../../../../src/modules/sync/adapters/user-data/json-last-opened-store';

let folder: string;
let file: string;

beforeEach(async () => {
  folder = await mkdtemp(join(tmpdir(), 'ui-gen-last-opened-'));
  file = join(folder, 'nested', 'last-opened-components.json');
});

afterEach(async () => {
  vi.restoreAllMocks();
  await rm(folder, { recursive: true, force: true, maxRetries: 3 });
});

// covers: FCT-0026
describe('the last opened components file', () => {
  it('has nothing to say before the first save', async () => {
    expect(await new JsonLastOpenedStore(file).load('E:\\kit')).toBeNull();
  });

  it('keeps one component per project across store instances', async () => {
    const store = new JsonLastOpenedStore(file);

    await store.save('E:\\kit', 'E:\\kit\\src\\a.ts');
    await store.save('E:\\other', 'E:\\other\\b.ts');
    await store.save('E:\\kit', 'E:\\kit\\src\\c.ts');

    const reopened = new JsonLastOpenedStore(file);
    expect(await reopened.load('E:\\kit')).toBe('E:\\kit\\src\\c.ts');
    expect(await reopened.load('e:\\other')).toBe('E:\\other\\b.ts');
    expect(await reopened.load('E:\\unknown')).toBeNull();
  });

  it('serialises concurrent saves without losing one', async () => {
    const store = new JsonLastOpenedStore(file);

    await Promise.all([store.save('E:\\a', 'E:\\a\\1.ts'), store.save('E:\\b', 'E:\\b\\2.ts')]);

    expect(await store.load('E:\\a')).toBe('E:\\a\\1.ts');
    expect(await store.load('E:\\b')).toBe('E:\\b\\2.ts');
  });

  it('leaves no temporary file behind', async () => {
    await new JsonLastOpenedStore(file).save('E:\\kit', 'E:\\kit\\a.ts');

    expect(await readdir(join(folder, 'nested'))).toEqual(['last-opened-components.json']);
  });

  it.each([
    ['is not JSON', 'not json'],
    ['has an unexpected shape', '{"formatVersion":9,"projects":{}}'],
  ])('starts over when the file %s', async (_label, content) => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const store = new JsonLastOpenedStore(file);
    await store.save('E:\\seed', 'E:\\seed\\a.ts');
    await writeFile(file, content, 'utf8');

    expect(await store.load('E:\\seed')).toBeNull();
    await store.save('E:\\kit', 'E:\\kit\\a.ts');

    expect(JSON.parse(await readFile(file, 'utf8'))).toEqual({
      formatVersion: 1,
      projects: { 'e:\\kit': 'E:\\kit\\a.ts' },
    });
    expect(warn).toHaveBeenCalled();
  });

  it('warns and keeps going when the file cannot be written', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const store = new JsonLastOpenedStore(join(folder, 'nested', 'blocked'));
    await store.save('E:\\kit', 'E:\\kit\\a.ts');
    await rm(join(folder, 'nested', 'blocked'), { recursive: true, force: true });
    await writeFile(join(folder, 'nested', 'file'), '', 'utf8');
    const blocked = new JsonLastOpenedStore(join(folder, 'nested', 'file', 'last.json'));

    await blocked.save('E:\\kit', 'E:\\kit\\a.ts');

    expect(await blocked.load('E:\\kit')).toBeNull();
    expect(warn).toHaveBeenCalled();
  });
});
