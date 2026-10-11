import { describe, expect, it } from 'vitest';
import {
  lastOpenedOf,
  parseLastOpened,
  serializeLastOpened,
  withLastOpened,
} from '../../../../../src/modules/sync/domain/library/last-opened';

// covers: FCT-0026
describe('last opened components', () => {
  it('remembers one component per project, whatever the case of the project path', () => {
    const opened = withLastOpened(withLastOpened({}, 'E:\\Kit', 'E:\\Kit\\a.ts'), 'E:\\Other', 'E:\\Other\\b.ts');

    expect(lastOpenedOf(opened, 'e:\\kit')).toBe('E:\\Kit\\a.ts');
    expect(lastOpenedOf(opened, 'E:\\Other')).toBe('E:\\Other\\b.ts');
    expect(lastOpenedOf(opened, 'E:\\Unknown')).toBeNull();
  });

  it('replaces the component of a project without touching the given map', () => {
    const before = withLastOpened({}, 'E:\\Kit', 'E:\\Kit\\a.ts');
    const after = withLastOpened(before, 'E:\\Kit', 'E:\\Kit\\b.ts');

    expect(lastOpenedOf(before, 'E:\\Kit')).toBe('E:\\Kit\\a.ts');
    expect(lastOpenedOf(after, 'E:\\Kit')).toBe('E:\\Kit\\b.ts');
  });

  it('survives a round trip through its file format', () => {
    const opened = withLastOpened({}, 'E:\\Kit', 'E:\\Kit\\a.ts');

    expect(parseLastOpened(JSON.parse(serializeLastOpened(opened)))).toEqual(opened);
  });

  it.each([
    null,
    [],
    'text',
    { formatVersion: 2, projects: {} },
    { formatVersion: 1 },
    { formatVersion: 1, projects: [] },
    { formatVersion: 1, projects: { 'e:\\kit': 4 } },
    { formatVersion: 1, projects: { 'e:\\kit': '' } },
  ])('refuses the unexpected shape %j', (value) => {
    expect(parseLastOpened(value)).toBeNull();
  });
});
