import { describe, expect, it } from 'vitest';
import { createStudioPublisher, type StudioContents } from '../../../src/main/components/studio-publisher';

class FakeContents implements StudioContents {
  readonly sent: [string, unknown][] = [];
  destroyed = false;

  isDestroyed(): boolean {
    return this.destroyed;
  }

  send(channel: string, payload: unknown): void {
    this.sent.push([channel, payload]);
  }
}

describe('publishing to the studio window', () => {
  it('sends the payload on the channel', () => {
    const contents = new FakeContents();

    createStudioPublisher(() => contents)('component:status', 'ready');

    expect(contents.sent).toEqual([['component:status', 'ready']]);
  });

  it('sends nothing while the window does not exist or once it is destroyed', () => {
    const contents = new FakeContents();
    contents.destroyed = true;

    createStudioPublisher(() => null)('component:status', 'ready');
    createStudioPublisher(() => contents)('component:status', 'ready');

    expect(contents.sent).toEqual([]);
  });
});
