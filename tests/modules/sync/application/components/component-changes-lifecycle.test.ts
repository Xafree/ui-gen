import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  CHANGE_DEBOUNCE_MS,
  ComponentChangeFeed,
} from '../../../../../src/modules/sync/application/components/component-changes';
import { ChangeRecorder, FakeAnalyzer, FakeLibrary, modelOf } from './component-doubles';

const DIR = 'E:\\kit\\src\\components';
const BADGE = `${DIR}\\badge.ts`;
const READ = 'tag=hz-badge label=Pro';

class StoppableLibrary extends FakeLibrary {
  stoppedByError: (() => void) | null = null;

  override watch(componentsDir: string, onChange: (path: string) => void, onStopped?: () => void) {
    this.stoppedByError = onStopped ?? null;
    return super.watch(componentsDir, onChange);
  }
}

function setUp(onDisk: string) {
  const library = new StoppableLibrary().add(BADGE, onDisk);
  const recorder = new ChangeRecorder();
  const feed = new ComponentChangeFeed({ library, analyzer: new FakeAnalyzer(), publish: recorder.publish }, DIR);
  return { library, recorder, feed };
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('a watch that stops by itself', () => {
  it('is reported by the feed, so that its owner can replace it', () => {
    const { library, feed } = setUp(READ);
    expect(feed.isWatching).toBe(true);

    library.stoppedByError?.();

    expect(feed.isWatching).toBe(false);
  });

  it('is reported as stopped once the feed is stopped', () => {
    const { feed } = setUp(READ);

    feed.stop();

    expect(feed.isWatching).toBe(false);
  });
});

describe('a write between the reading of the component and the start of following it', () => {
  it('is published right after following, without waiting for another modification', async () => {
    const { feed, recorder } = setUp('tag=hz-badge label=Team');

    feed.follow(BADGE, READ, modelOf(READ));
    await vi.advanceTimersByTimeAsync(0);

    expect(recorder.changes.map(({ text }) => text)).toEqual(['tag=hz-badge label=Team']);
  });

  it('publishes nothing when the file did not change', async () => {
    const { feed, recorder } = setUp(READ);

    feed.follow(BADGE, READ, modelOf(READ));
    await vi.advanceTimersByTimeAsync(CHANGE_DEBOUNCE_MS);

    expect(recorder.changes).toEqual([]);
  });
});
