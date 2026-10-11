import { describe, expect, it } from 'vitest';
import { lastOpenedComponent, openComponent } from '../../../../../src/modules/sync/application/components/open-component';
import { BROKEN, FakeAnalyzer, FakeLibrary, MemoryLastOpened } from './component-doubles';

const LOCATION = { projectPath: 'E:\\kit', componentsDir: 'E:\\kit\\src\\components', tagPrefix: 'hz-' };
const BADGE = 'E:\\kit\\src\\components\\badge.ts';
const CARD = 'E:\\kit\\src\\components\\card.ts';

function setUp() {
  const library = new FakeLibrary().add(BADGE, 'tag=hz-badge').add(CARD, 'tag=hz-card');
  return { library, analyzer: new FakeAnalyzer(), lastOpened: new MemoryLastOpened() };
}

// covers: FCT-0028
describe('opening a component', () => {
  it('answers the exact text, its model and its highlighting', async () => {
    const opened = await openComponent(setUp(), LOCATION, BADGE);

    expect(opened).toMatchObject({ path: BADGE, text: 'tag=hz-badge', model: { tag: 'hz-badge' } });
    expect(opened.highlight).toHaveLength(1);
  });

  it('remembers the component as the last opened one of the project', async () => {
    const deps = setUp();

    await openComponent(deps, LOCATION, BADGE);
    await openComponent(deps, LOCATION, CARD);

    expect(deps.lastOpened.saved.get('E:\\kit')).toBe(CARD);
  });
});

// covers: FCT-0043
describe('opening a component that does not parse', () => {
  it('answers its exact text with the syntax error', async () => {
    const deps = setUp();
    const broken = `tag=hz-badge ${BROKEN}`;
    deps.library.add(BADGE, broken);

    const opened = await openComponent(deps, LOCATION, BADGE);

    expect(opened.text).toBe(broken);
    expect(opened.model.diagnostics).toEqual([expect.objectContaining({ severity: 'error', code: 'ts(1005)' })]);
  });
});

// covers: FCT-0026
describe('the last opened component', () => {
  it('is absent before any component was opened', async () => {
    expect(await lastOpenedComponent(setUp(), LOCATION)).toBeNull();
  });

  it('is the component opened last in the project', async () => {
    const deps = setUp();
    await openComponent(deps, LOCATION, BADGE);
    await openComponent(deps, LOCATION, CARD);

    expect(await lastOpenedComponent(deps, LOCATION)).toBe(CARD);
    expect(await lastOpenedComponent(deps, { ...LOCATION, projectPath: 'E:\\other' })).toBeNull();
  });

  it('is absent once its file left the library', async () => {
    const deps = setUp();
    await openComponent(deps, LOCATION, CARD);
    deps.library.files.delete(CARD);

    expect(await lastOpenedComponent(deps, LOCATION)).toBeNull();
  });
});
