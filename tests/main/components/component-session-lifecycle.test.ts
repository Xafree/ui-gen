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

const KIT = 'E:\\kit';
const KIT_COMPONENTS = join(KIT, 'src', 'components');
const CARD = join(KIT_COMPONENTS, 'hz-card.ts');

const KIT_PROJECT: OpenedProject = {
  path: KIT,
  name: 'Kit',
  packageName: '@kit/ui',
  version: '1.0.0',
  settings: { formatVersion: 1, tagPrefix: 'hz-', componentsDir: 'src/components' },
};

class FailingLibrary extends FakeLibrary {
  readonly stoppers: (() => void)[] = [];
  stopAtOnce = false;

  override watch(componentsDir: string, onChange: (path: string) => void, onStopped?: () => void) {
    const stop = super.watch(componentsDir, onChange);
    if (onStopped !== undefined) this.stoppers.push(onStopped);
    if (this.stopAtOnce) onStopped?.();
    return stop;
  }
}

function setUp() {
  const project = new ProjectSession('E:\\documents');
  const library = new FailingLibrary().add(CARD, 'tag=hz-card');
  const recorder = new ChangeRecorder();
  const session = new ComponentSession({
    project,
    library,
    analyzer: new FakeAnalyzer(),
    lastOpened: new MemoryLastOpened(),
    publish: recorder.publish,
  });
  project.openProject(KIT_PROJECT);
  return { project, library, recorder, session };
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('closing the project', () => {
  it('stops watching the components folder', async () => {
    const { project, library, session } = setUp();
    await session.open(CARD);

    project.closeProject();

    expect(library.stops).toBe(1);
    expect(library.isWatching).toBe(false);
  });

  it('watches again when the same project is reopened', async () => {
    const { project, library, session } = setUp();
    await session.open(CARD);
    project.closeProject();

    project.openProject(KIT_PROJECT);
    await session.open(CARD);

    expect(library.watched).toEqual([KIT_COMPONENTS, KIT_COMPONENTS]);
    expect(library.isWatching).toBe(true);
  });
});

describe('a watch that stopped by itself', () => {
  it('is replaced by a new one at the next open, which follows the modifications again', async () => {
    const { library, recorder, session } = setUp();
    await session.open(CARD);
    library.stoppers[0]?.();

    await session.open(CARD);
    library.add(CARD, 'tag=hz-card changed');
    library.emit(CARD);
    await vi.advanceTimersByTimeAsync(CHANGE_DEBOUNCE_MS);

    expect(library.watched).toEqual([KIT_COMPONENTS, KIT_COMPONENTS]);
    expect(library.stops).toBe(1);
    expect(recorder.changes.map(({ text }) => text)).toEqual(['tag=hz-card changed']);
  });

  it('is retried at every open while the components folder cannot be watched', async () => {
    const { library, session } = setUp();
    library.stopAtOnce = true;

    await session.open(CARD);
    await session.open(CARD);

    expect(library.watched).toEqual([KIT_COMPONENTS, KIT_COMPONENTS]);
  });

  it('keeps the watch while it runs', async () => {
    const { library, session } = setUp();

    await session.open(CARD);
    await session.open(CARD);

    expect(library.watched).toEqual([KIT_COMPONENTS]);
  });
});

describe('a modification written between the opening and the following of a component', () => {
  it('is published after the opening', async () => {
    const { library, recorder, session } = setUp();
    const read = library.readText.bind(library);
    library.readText = async (path) => {
      const text = await read(path);
      library.add(path, 'tag=hz-card written meanwhile');
      return text;
    };

    await session.open(CARD);
    await vi.advanceTimersByTimeAsync(0);

    expect(recorder.changes.map(({ text }) => text)).toEqual(['tag=hz-card written meanwhile']);
  });
});
