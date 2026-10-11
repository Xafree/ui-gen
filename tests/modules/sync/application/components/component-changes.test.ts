import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  CHANGE_DEBOUNCE_MS,
  ComponentChangeFeed,
} from '../../../../../src/modules/sync/application/components/component-changes';
import { BROKEN, ChangeRecorder, FakeAnalyzer, FakeLibrary, modelOf } from './component-doubles';

const DIR = 'E:\\kit\\src\\components';
const BADGE = `${DIR}\\badge.ts`;
const READABLE = 'tag=hz-badge label=Pro';

interface Harness {
  readonly library: FakeLibrary;
  readonly analyzer: FakeAnalyzer;
  readonly recorder: ChangeRecorder;
  readonly feed: ComponentChangeFeed;
}

function setUp(initial = READABLE): Harness {
  const library = new FakeLibrary().add(BADGE, initial);
  const analyzer = new FakeAnalyzer();
  const recorder = new ChangeRecorder();
  const feed = new ComponentChangeFeed({ library, analyzer, publish: recorder.publish }, DIR);
  feed.follow(BADGE, initial, modelOf(initial));
  return { library, analyzer, recorder, feed };
}

async function writeExternally({ library }: Harness, text: string, path = BADGE): Promise<void> {
  library.add(path, text);
  library.emit(path);
  await vi.advanceTimersByTimeAsync(CHANGE_DEBOUNCE_MS);
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('watching the components folder', () => {
  it('starts one native watch on the components folder', () => {
    expect(setUp().library.watched).toEqual([DIR]);
  });

  it('stops watching, and publishes nothing afterwards', async () => {
    const harness = setUp();
    harness.feed.stop();

    await writeExternally(harness, 'tag=hz-badge label=Late');

    expect(harness.library.stops).toBe(1);
    expect(harness.library.isWatching).toBe(false);
    expect(harness.recorder.changes).toEqual([]);
  });
});

// covers: FCT-0042
describe('an external modification of the followed component', () => {
  it('publishes the new version once per write', async () => {
    const harness = setUp();

    await writeExternally(harness, 'tag=hz-badge label=Team');
    await writeExternally(harness, 'tag=hz-badge label=Max');

    expect(harness.recorder.changes.map(({ text }) => text)).toEqual(['tag=hz-badge label=Team', 'tag=hz-badge label=Max']);
    expect(harness.recorder.changes[0]).toMatchObject({
      path: BADGE,
      model: { version: 'tag=hz-badge label=Team' },
      diagnostics: [],
    });
    expect(harness.recorder.changes[0]?.highlight).toHaveLength(1);
  });

  it('coalesces a burst of writes into the last version', async () => {
    const harness = setUp();

    for (const label of ['A', 'B', 'C']) {
      harness.library.add(BADGE, `tag=hz-badge label=${label}`);
      harness.library.emit(BADGE);
      await vi.advanceTimersByTimeAsync(CHANGE_DEBOUNCE_MS - 10);
    }
    await vi.advanceTimersByTimeAsync(CHANGE_DEBOUNCE_MS);

    expect(harness.recorder.changes.map(({ text }) => text)).toEqual(['tag=hz-badge label=C']);
  });

  it('ignores an event whose content did not change', async () => {
    const harness = setUp();

    await writeExternally(harness, READABLE);

    expect(harness.recorder.changes).toEqual([]);
    expect(harness.analyzer.analyzed).toEqual([]);
  });

  it('ignores the other files of the folder', async () => {
    const harness = setUp();

    await writeExternally(harness, 'tag=hz-card changed', `${DIR}\\card.ts`);

    expect(harness.recorder.changes).toEqual([]);
  });

  it('matches the followed file whatever the case of the reported path', async () => {
    const harness = setUp();
    harness.library.add(BADGE, 'tag=hz-badge label=Case');
    harness.library.emit(BADGE.toUpperCase());
    await vi.advanceTimersByTimeAsync(CHANGE_DEBOUNCE_MS);

    expect(harness.recorder.changes).toHaveLength(1);
  });

  it('publishes nothing before a component is followed', async () => {
    const library = new FakeLibrary().add(BADGE, READABLE);
    const recorder = new ChangeRecorder();
    const feed = new ComponentChangeFeed({ library, analyzer: new FakeAnalyzer(), publish: recorder.publish }, DIR);

    library.emit(BADGE);
    await vi.advanceTimersByTimeAsync(CHANGE_DEBOUNCE_MS);
    feed.stop();

    expect(recorder.changes).toEqual([]);
  });

  it('forgets a pending modification when another component is followed', async () => {
    const harness = setUp();
    harness.library.add(BADGE, 'tag=hz-badge label=Pending');
    harness.library.emit(BADGE);

    harness.feed.follow(`${DIR}\\card.ts`, 'tag=hz-card', modelOf('tag=hz-card'));
    await vi.advanceTimersByTimeAsync(CHANGE_DEBOUNCE_MS);

    expect(harness.recorder.changes).toEqual([]);
  });

  it('warns and keeps going when the file cannot be read', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const harness = setUp();
    harness.library.files.delete(BADGE);
    harness.library.emit(BADGE);
    await vi.advanceTimersByTimeAsync(CHANGE_DEBOUNCE_MS);
    expect(warn).toHaveBeenCalledOnce();

    await writeExternally(harness, 'tag=hz-badge label=Back');

    expect(harness.recorder.changes).toHaveLength(1);
  });

  it('drops a version that a newer write already replaced while it was analysed', async () => {
    const harness = setUp();
    let release: () => void = () => undefined;
    harness.analyzer.pause = new Promise<void>((resolve) => {
      release = resolve;
    });
    harness.library.add(BADGE, 'tag=hz-badge label=Old');
    harness.library.emit(BADGE);
    await vi.advanceTimersByTimeAsync(CHANGE_DEBOUNCE_MS);

    harness.analyzer.pause = null;
    await writeExternally(harness, 'tag=hz-badge label=New');
    release();
    await vi.advanceTimersByTimeAsync(0);

    expect(harness.recorder.changes.map(({ text }) => text)).toEqual(['tag=hz-badge label=New']);
  });
});

// covers: FCT-0043
describe('an external modification that does not parse', () => {
  it('keeps the model of the last readable version and reports the new errors', async () => {
    const harness = setUp();
    const broken = `tag=hz-badge ${BROKEN}`;

    await writeExternally(harness, broken);

    const [change] = harness.recorder.changes;
    expect(change).toMatchObject({ path: BADGE, text: broken, model: { version: READABLE } });
    expect(change?.diagnostics).toEqual([expect.objectContaining({ severity: 'error', code: 'ts(1005)' })]);
  });

  it('keeps the last readable model through several broken versions, then follows the repair', async () => {
    const harness = setUp();

    await writeExternally(harness, `tag=hz-badge ${BROKEN} 1`);
    await writeExternally(harness, `tag=hz-badge ${BROKEN} 2`);
    await writeExternally(harness, 'tag=hz-badge label=Fixed');

    const versions = harness.recorder.changes.map(({ model }) => model.version);
    expect(versions).toEqual([READABLE, READABLE, 'tag=hz-badge label=Fixed']);
    expect(harness.recorder.changes[2]?.diagnostics).toEqual([]);
  });

  it('shows the broken model when no readable version ever existed', async () => {
    const broken = `tag=hz-badge ${BROKEN}`;
    const harness = setUp(broken);

    await writeExternally(harness, `${broken} again`);

    expect(harness.recorder.changes[0]?.model.version).toBe(`${broken} again`);
  });
});
