import { afterEach, describe, expect, it, vi } from 'vitest';
import { findListedComponent, listComponents } from '../../../../../src/modules/sync/application/components/list-components';
import { FakeAnalyzer, FakeLibrary } from './component-doubles';

const LOCATION = { projectPath: 'E:\\kit', componentsDir: 'E:\\kit\\src\\components', tagPrefix: 'hz-' };

afterEach(() => {
  vi.restoreAllMocks();
});

// covers: FCT-0026
describe('listing the components of a library', () => {
  it('sorts the components by tag and gives each a readable name', async () => {
    const library = new FakeLibrary()
      .add('E:\\kit\\src\\components\\z.ts', 'tag=hz-pricing-card')
      .add('E:\\kit\\src\\components\\badge\\a.ts', 'tag=hz-plan-badge')
      .add('E:\\kit\\src\\components\\m.ts', 'tag=hz-feature-list');

    const listed = await listComponents({ library, analyzer: new FakeAnalyzer() }, LOCATION);

    expect(listed).toEqual([
      { path: 'E:\\kit\\src\\components\\m.ts', tag: 'hz-feature-list', name: 'Feature List' },
      { path: 'E:\\kit\\src\\components\\badge\\a.ts', tag: 'hz-plan-badge', name: 'Plan Badge' },
      { path: 'E:\\kit\\src\\components\\z.ts', tag: 'hz-pricing-card', name: 'Pricing Card' },
    ]);
  });

  it('leaves out the files that declare no component', async () => {
    const library = new FakeLibrary()
      .add('E:\\kit\\src\\components\\index.ts', "export * from './a.js';")
      .add('E:\\kit\\src\\components\\a.ts', 'tag=hz-a');

    const listed = await listComponents({ library, analyzer: new FakeAnalyzer() }, LOCATION);

    expect(listed.map(({ tag }) => tag)).toEqual(['hz-a']);
  });

  it('skips a file that cannot be analysed and says so', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const library = new FakeLibrary().add('E:\\kit\\src\\components\\a.ts', 'tag=hz-a');
    const analyzer = new FakeAnalyzer();
    analyzer.failWith = new Error('process stopped');

    const listed = await listComponents({ library, analyzer }, LOCATION);

    expect(listed).toEqual([]);
    expect(warn).toHaveBeenCalledOnce();
  });
});

// covers: FCT-0027
describe('listing an empty library', () => {
  it('lists nothing', async () => {
    expect(await listComponents({ library: new FakeLibrary(), analyzer: new FakeAnalyzer() }, LOCATION)).toEqual([]);
  });
});

describe('finding a listed component', () => {
  const library = new FakeLibrary()
    .add('E:\\kit\\src\\components\\a.ts', 'tag=hz-a')
    .add('E:\\kit\\src\\components\\index.ts', 'export {}');
  const deps = { library, analyzer: new FakeAnalyzer() };

  it('answers the listed spelling of the path, whatever the case', async () => {
    expect(await findListedComponent(deps, LOCATION, 'e:\\KIT\\src\\components\\A.ts')).toBe(
      'E:\\kit\\src\\components\\a.ts',
    );
  });

  it('refuses a file that is not in the library, or that is not a component', async () => {
    expect(await findListedComponent(deps, LOCATION, 'E:\\kit\\src\\components\\missing.ts')).toBeNull();
    expect(await findListedComponent(deps, LOCATION, 'E:\\kit\\src\\components\\index.ts')).toBeNull();
    expect(await findListedComponent(deps, LOCATION, 'E:\\kit\\uigen.json')).toBeNull();
  });
});
