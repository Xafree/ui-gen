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
    window.addEventListener('message', this.#accept);
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
    window.removeEventListener('message', this.#accept);
    this.#port?.close();
    this.#port = null;
    for (const waiter of this.#hitWaiters.splice(0)) waiter(null);
    this.iframe.remove();
  }

  readonly #accept = (event: MessageEvent<unknown>): void => {
    if (this.#port !== null || event.source !== this.iframe.contentWindow) return;
    const [received, ...extra] = event.ports;
    if (received === undefined || extra.length > 0 || !isConnectMessage(event.data)) return;
    this.#port = received;
    received.onmessage = (portEvent: MessageEvent<unknown>) => {
      const message = readHarnessMessage(portEvent.data, this.#knownIds);
      if (message === null) console.warn('The canvas sent an unreadable message');
      else this.#listener(this, message);
    };
  };
}

function isConnectMessage(data: unknown): boolean {
  return typeof data === 'object' && data !== null && Reflect.get(data, 'type') === 'connect';
}
