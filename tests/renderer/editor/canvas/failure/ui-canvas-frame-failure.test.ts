import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  CANVAS_FAILED,
  CANVAS_READY,
  UiCanvasFrame,
  type CanvasFailedDetail,
} from '../../../../../src/renderer/editor/canvas/ui-canvas-frame.js';
import { ANSWERING_HARNESS_URL, openedComponent } from '../fake-harness.js';
import { FAILING_HARNESS_URL } from './failing-harness.js';

const harnessUrls = new Map<string, string>();

vi.mock('../../../../../src/modules/canvas/application/harness-url.js', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../../../../src/modules/canvas/application/harness-url.js')>()),
  buildHarnessUrl: ({ model }: { model: { version: string } }) => harnessUrls.get(model.version) ?? null,
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

function record(frame: UiCanvasFrame): { failures: CanvasFailedDetail[]; ready: number } {
  const seen = { failures: [] as CanvasFailedDetail[], ready: 0 };
  frame.addEventListener(CANVAS_FAILED, (event) => {
    if (event instanceof CustomEvent) seen.failures.push(event.detail as CanvasFailedDetail);
  });
  frame.addEventListener(CANVAS_READY, () => {
    seen.ready += 1;
  });
  return seen;
}

beforeEach(() => {
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
  harnessUrls.clear();
  harnessUrls.set('v1', ANSWERING_HARNESS_URL);
  harnessUrls.set('broken', FAILING_HARNESS_URL);
});

afterEach(() => {
  vi.restoreAllMocks();
  for (const frame of frames.splice(0)) frame.remove();
});

describe('the canvas frame when the component cannot be rendered', () => {
  it('announces the failure and drops the iframe when there is no previous rendering', async () => {
    const frame = await mountFrame();
    const seen = record(frame);

    frame.component = openedComponent('broken');

    await expect.poll(() => seen.failures).toEqual([{ kept: false }]);
    expect(seen.ready).toBe(0);
    expect(iframesOf(frame)).toHaveLength(0);
  });

  it('keeps the previous rendering and says so when a new version fails', async () => {
    const frame = await mountFrame();
    const seen = record(frame);
    frame.component = openedComponent('v1');
    await expect.poll(() => seen.ready).toBe(1);
    const [front] = iframesOf(frame);

    frame.component = openedComponent('broken');

    await expect.poll(() => seen.failures).toEqual([{ kept: true }]);
    expect(iframesOf(frame)).toEqual([front]);
    expect(front?.hasAttribute('data-pending')).toBe(false);
  });
});
