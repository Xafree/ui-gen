import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ComponentSession } from '../../../src/main/components/component-session';
import { ProjectSession } from '../../../src/main/session/project-session';
import type { OpenedProject } from '../../../src/modules/project/application/open/project-outcomes';
import { CHANGE_DEBOUNCE_MS } from '../../../src/modules/sync/application/components/component-changes';
import {
  ChangeRecorder,
  FakeAnalyzer,
  FakeLibrary,
  MemoryLastOpened,
} from '../../modules/sync/application/components/component-doubles';

function projectAt(path: string, componentsDir = 'src/components'): OpenedProject {
  return {
    path,
    name: 'Kit',
    packageName: '@kit/ui',
    version: '1.0.0',
    settings: { formatVersion: 1, tagPrefix: 'hz-', componentsDir },
  };
}

const KIT = 'E:\\kit';
const KIT_COMPONENTS = join(KIT, 'src', 'components');
const BADGE = join(KIT_COMPONENTS, 'badge', 'hz-badge.ts');
const CARD = join(KIT_COMPONENTS, 'hz-card.ts');

function setUp() {
  const project = new ProjectSession('E:\\documents');
  const library = new FakeLibrary()
    .add(BADGE, 'tag=hz-badge')
    .add(CARD, 'tag=hz-card')
    .add(join(KIT_COMPONENTS, 'index.ts'), 'export {}');
  const recorder = new ChangeRecorder();
  const lastOpened = new MemoryLastOpened();
  const session = new ComponentSession({
    project,
    library,
    analyzer: new FakeAnalyzer(),
    lastOpened,
    publish: recorder.publish,
  });
  return { project, library, recorder, lastOpened, session };
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('without an open project', () => {
  it('lists nothing, resolves nothing and remembers nothing', async () => {
    const { session } = setUp();

    expect(await session.list()).toEqual([]);
    expect(await session.resolve(CARD)).toBeNull();
    expect(await session.lastOpened()).toBeNull();
  });

  it('refuses to open a component', async () => {
    await expect(setUp().session.open(CARD)).rejects.toThrow('No project is open');
  });
});

// covers: FCT-0026
describe('with an open project', () => {
  it('lists the components of the components folder of the project, sorted by tag', async () => {
    const { project, session } = setUp();
    project.openProject(projectAt(KIT));

    expect((await session.list()).map(({ tag, name }) => [tag, name])).toEqual([
      ['hz-badge', 'Badge'],
      ['hz-card', 'Card'],
    ]);
  });

  it('resolves only the listed components', async () => {
    const { project, session } = setUp();
    project.openProject(projectAt(KIT));

    expect(await session.resolve(CARD.toUpperCase())).toBe(CARD);
    expect(await session.resolve(join(KIT_COMPONENTS, 'index.ts'))).toBeNull();
  });

  it('remembers the opened component for the project', async () => {
    const { project, session, lastOpened } = setUp();
    project.openProject(projectAt(KIT));

    expect(await session.lastOpened()).toBeNull();
    await session.open(CARD);

    expect(lastOpened.saved.get(KIT)).toBe(CARD);
    expect(await session.lastOpened()).toBe(CARD);
  });
});

// covers: FCT-0042
describe('following the opened component', () => {
  it('watches the components folder once, and publishes the external modifications', async () => {
    const { project, library, recorder, session } = setUp();
    project.openProject(projectAt(KIT));

    await session.open(CARD);
    await session.open(BADGE);
    library.add(BADGE, 'tag=hz-badge changed');
    library.emit(BADGE);
    await vi.advanceTimersByTimeAsync(CHANGE_DEBOUNCE_MS);

    expect(library.watched).toEqual([KIT_COMPONENTS]);
    expect(recorder.changes.map(({ path, text }) => [path, text])).toEqual([[BADGE, 'tag=hz-badge changed']]);
  });

  it('watches the other folder when another project is opened, and stops the previous watch', async () => {
    const { project, library, session } = setUp();
    project.openProject(projectAt(KIT));
    await session.open(CARD);

    project.openProject(projectAt('E:\\other', 'lib'));
    library.add(join('E:\\other', 'lib', 'hz-a.ts'), 'tag=hz-a');
    await session.open(join('E:\\other', 'lib', 'hz-a.ts'));

    expect(library.watched).toEqual([KIT_COMPONENTS, join('E:\\other', 'lib')]);
    expect(library.stops).toBe(1);
  });

  it('publishes nothing once the project is no longer the open one', async () => {
    const { project, library, recorder, session } = setUp();
    project.openProject(projectAt(KIT));
    await session.open(CARD);

    project.closeProject();
    library.add(CARD, 'tag=hz-card changed');
    library.emit(CARD);
    await vi.advanceTimersByTimeAsync(CHANGE_DEBOUNCE_MS);

    expect(recorder.changes).toEqual([]);
  });

  it('stops watching when disposed', async () => {
    const { project, library, session } = setUp();
    project.openProject(projectAt(KIT));
    await session.open(CARD);

    session.dispose();
    session.dispose();

    expect(library.stops).toBe(1);
  });
});
