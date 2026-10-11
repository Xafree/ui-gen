import type { HarnessCommand } from '../../../modules/canvas/domain/harness-command.js';
import { readHarnessMessage, type HarnessMessage } from '../../../modules/canvas/domain/harness-message.js';

export type CanvasViewListener = (view: CanvasView, message: HarnessMessage) => void;

export class CanvasView {
  readonly iframe: HTMLIFrameElement;
  readonly url: string;
  readonly #knownIds: ReadonlySet<string>;
  readonly #listener: CanvasViewListener;
  readonly #hitWaiters: ((id: string | null) => void)[] = [];
  #port: MessagePort | null = null;

  constructor(url: string, knownIds: ReadonlySet<string>, listener: CanvasViewListener) {
    this.url = url;
    this.#knownIds = knownIds;
    this.#listener = listener;
    this.iframe = document.createElement('iframe');
    this.iframe.setAttribute('sandbox', 'allow-scripts');
    this.iframe.title = 'Rendu du composant';
    this.iframe.addEventListener('load', this.#connect);
    this.iframe.setAttribute('src', url);
  }

  command(command: HarnessCommand): void {
    this.#port?.postMessage(command);
  }

  hitTest(x: number, y: number): Promise<string | null> {
    return new Promise((resolve) => {
      if (this.#port === null) {
        resolve(null);
        return;
      }
      this.#hitWaiters.push(resolve);
      this.command({ type: 'hit-test', x, y });
    });
  }

  settleHit(id: string | null): void {
    this.#hitWaiters.shift()?.(id);
  }

  dispose(): void {
    this.iframe.removeEventListener('load', this.#connect);
    this.#port?.close();
    this.#port = null;
    for (const waiter of this.#hitWaiters.splice(0)) waiter(null);
    this.iframe.remove();
  }

  readonly #connect = (): void => {
    if (this.#port !== null) return;
    const channel = new MessageChannel();
    this.#port = channel.port1;
    channel.port1.onmessage = (event: MessageEvent<unknown>) => {
      const message = readHarnessMessage(event.data, this.#knownIds);
      if (message === null) console.warn('The canvas sent an unreadable message');
      else this.#listener(this, message);
    };
    this.iframe.contentWindow?.postMessage('connect', '*', [channel.port2]);
  };
}
