import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { NodeRect } from '../../../../src/modules/canvas/domain/harness-message.js';
import {
  CANVAS_READY,
  CANVAS_RECTS,
  RENDER_TIMEOUT_MS,
  UiCanvasFrame,
  type CanvasRectsDetail,
} from '../../../../src/renderer/editor/canvas/ui-canvas-frame.js';
import { ANSWERING_HARNESS_URL, SILENT_HARNESS_URL, openedComponent } from './fake-harness.js';

const harnessUrls = new Map<string, string>();

vi.mock('../../../../src/modules/canvas/application/harness-url.js', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../../../src/modules/canvas/application/harness-url.js')>()),
  buildHarnessUrl: ({ model, dark }: { model: { version: string }; dark: boolean }) =>
    harnessUrls.get(`${model.version}${dark ? ':dark' : ''}`) ?? null,
}));

const frames: UiCanvasFrame[] = [];

async function mountFrame(): Promise<UiCanvasFrame> {
  const frame = new UiCanvasFrame();
  frame.projectPath = 'E:/kit';
  document.body.append(frame);
  frames.push(frame);
  await frame.updateComplete;
  return frame;
}

function iframesOf(frame: UiCanvasFrame): HTMLIFrameElement[] {
  return [...(frame.shadowRoot?.querySelectorAll('iframe') ?? [])];
}

function recordRects(frame: UiCanvasFrame): NodeRect[][] {
  const received: NodeRect[][] = [];
  frame.addEventListener(CANVAS_RECTS, (event) => {
    if (event instanceof CustomEvent) received.push((event.detail as CanvasRectsDetail).rects.slice());
  });
  return received;
}

function countReady(frame: UiCanvasFrame): { count: number } {
  const ready = { count: 0 };
  frame.addEventListener(CANVAS_READY, () => {
    ready.count += 1;
  });
  return ready;
}

beforeEach(() => {
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
  harnessUrls.clear();
  harnessUrls.set('v1', ANSWERING_HARNESS_URL);
  harnessUrls.set('v1:dark', `${ANSWERING_HARNESS_URL}#dark`);
  harnessUrls.set('v2', `${ANSWERING_HARNESS_URL}#second`);
  harnessUrls.set('silent', SILENT_HARNESS_URL);
});

afterEach(() => {
  vi.restoreAllMocks();
  for (const frame of frames.splice(0)) frame.remove();
});

describe('the canvas frame', () => {
  it('shows nothing without a component', async () => {
    const frame = await mountFrame();

    expect(iframesOf(frame)).toHaveLength(0);
    await expect(frame.hitTest(0, 0)).resolves.toBeNull();
  });

  it('renders the component in one sandboxed iframe and announces it once ready', async () => {
    const frame = await mountFrame();
    const ready = countReady(frame);
    const rects = recordRects(frame);

    frame.component = openedComponent('v1');
    await frame.updateComplete;

    const [iframe] = iframesOf(frame);
    expect(iframesOf(frame)).toHaveLength(1);
    expect(iframe?.getAttribute('sandbox')).toBe('allow-scripts');
    expect(iframe?.hasAttribute('data-pending')).toBe(false);
    await expect.poll(() => ready.count).toBe(1);
    await expect.poll(() => rects.length).toBeGreaterThan(0);
    expect(rects[0]).toEqual([{ id: '0', x: 1, y: 2, width: 3, height: 4 }]);
  });

  it('answers a hit test with the node under the point', async () => {
    const frame = await mountFrame();
    const ready = countReady(frame);
    frame.component = openedComponent('v1');
    await expect.poll(() => ready.count).toBe(1);

    await expect(frame.hitTest(10, 10)).resolves.toBe('0');
  });

  it('renders a new version in a hidden iframe and swaps it in when ready, then drops the old one', async () => {
    const frame = await mountFrame();
    const ready = countReady(frame);
    frame.component = openedComponent('v1');
    await expect.poll(() => ready.count).toBe(1);
    const [first] = iframesOf(frame);

    frame.component = openedComponent('v2');
    await frame.updateComplete;

    const during = iframesOf(frame);
    expect(during).toHaveLength(2);
    expect(during[1]?.hasAttribute('data-pending')).toBe(true);
    await expect.poll(() => ready.count).toBe(2);
    const after = iframesOf(frame);
    expect(after).toHaveLength(1);
    expect(after[0]).not.toBe(first);
    expect(after[0]?.hasAttribute('data-pending')).toBe(false);
    expect(first?.isConnected).toBe(false);
  });

  it('does not render the same version twice, and drops a pending version when the front one comes back', async () => {
    const frame = await mountFrame();
    const ready = countReady(frame);
    frame.component = openedComponent('v1');
    await expect.poll(() => ready.count).toBe(1);

    frame.component = openedComponent('v1');
    await frame.updateComplete;
    expect(iframesOf(frame)).toHaveLength(1);

    harnessUrls.set('v2', `${SILENT_HARNESS_URL}#never`);
    frame.component = openedComponent('v2');
    await frame.updateComplete;
    expect(iframesOf(frame)).toHaveLength(2);

    frame.component = openedComponent('v1');
    await frame.updateComplete;
    expect(iframesOf(frame)).toHaveLength(1);
  });

  it('keeps the previous rendering when the new version never answers', async () => {
    const frame = await mountFrame();
    const ready = countReady(frame);
    frame.component = openedComponent('v1');
    await expect.poll(() => ready.count).toBe(1);
    const timers: (() => void)[] = [];
    const realSetTimeout = globalThis.setTimeout.bind(globalThis);
    vi.spyOn(globalThis, 'setTimeout').mockImplementation(((callback: () => void, delay?: number) => {
      if (delay === RENDER_TIMEOUT_MS) {
        timers.push(callback);
        return 0;
      }
      return realSetTimeout(callback, delay);
    }) as typeof setTimeout);
    harnessUrls.set('v2', `${SILENT_HARNESS_URL}#never`);

    frame.component = openedComponent('v2');
    await frame.updateComplete;
    expect(iframesOf(frame)).toHaveLength(2);
    timers[0]?.();

    expect(iframesOf(frame)).toHaveLength(1);
    expect(ready.count).toBe(1);
    expect(console.warn).toHaveBeenCalledWith('The canvas did not answer in time; the previous rendering is kept');
  });

  it('passes the dark preview to the rendered document, now and for the next version', async () => {
    const frame = await mountFrame();
    const ready = countReady(frame);
    const rects = recordRects(frame);
    frame.component = openedComponent('v1');
    await expect.poll(() => ready.count).toBe(1);

    frame.darkPreview = true;
    await frame.updateComplete;

    await expect.poll(() => rects.at(-1)?.[0]?.x).toBe(7);
    frame.darkPreview = false;
    await expect.poll(() => rects.at(-1)?.[0]?.x).toBe(3);
  });

  it('applies the dark preview chosen while a version was still loading', async () => {
    const frame = await mountFrame();
    const ready = countReady(frame);
    const rects = recordRects(frame);
    frame.darkPreview = true;
    frame.component = openedComponent('v1');
    harnessUrls.set('v1', ANSWERING_HARNESS_URL);
    harnessUrls.set('v1:dark', ANSWERING_HARNESS_URL);

    await expect.poll(() => ready.count).toBe(1);

    await expect.poll(() => rects.some(([rect]) => rect?.x === 7)).toBe(true);
  });

  it('shows nothing for a component outside the project, and clears the canvas when the component goes', async () => {
    const frame = await mountFrame();
    const ready = countReady(frame);
    frame.component = openedComponent('unknown');
    await frame.updateComplete;
    expect(iframesOf(frame)).toHaveLength(0);

    frame.component = openedComponent('v1');
    await expect.poll(() => ready.count).toBe(1);
    frame.component = null;
    await frame.updateComplete;

    expect(iframesOf(frame)).toHaveLength(0);
  });

  it('releases its iframes when removed and renders again when attached again', async () => {
    const frame = await mountFrame();
    const ready = countReady(frame);
    frame.component = openedComponent('v1');
    await expect.poll(() => ready.count).toBe(1);
    const [first] = iframesOf(frame);

    frame.remove();
    expect(first?.isConnected).toBe(false);
    document.body.append(frame);
    await frame.updateComplete;

    await expect.poll(() => iframesOf(frame).length).toBe(1);
    expect(iframesOf(frame)[0]).not.toBe(first);
  });
});
