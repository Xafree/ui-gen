import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { NodeRect } from '../../../../src/modules/canvas/domain/harness-message.js';
import { CANVAS_RECTS, UiCanvasFrame, type CanvasRectsDetail } from '../../../../src/renderer/editor/canvas/ui-canvas-frame.js';
import { openedComponent } from './fake-harness.js';

const LAYER_STATE_ECHO = `
addEventListener('message', (event) => {
  const port = event.ports[0];
  port.onmessage = (message) => {
    if (message.data.type === 'layer-state') {
      port.postMessage({ type: 'rects', rects: [{ id: '0', x: message.data.hidden.length, y: message.data.locked.length, width: 1, height: 1 }] });
    }
  };
  port.postMessage({ type: 'ready' });
});
`;

const ECHO_URL = `data:text/html;charset=utf-8,${encodeURIComponent(`<script>${LAYER_STATE_ECHO}</script>`)}`;

vi.mock('../../../../src/modules/canvas/application/harness-url.js', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../../../src/modules/canvas/application/harness-url.js')>()),
  buildHarnessUrl: () => ECHO_URL,
}));

const frames: UiCanvasFrame[] = [];

function recordRects(frame: UiCanvasFrame): NodeRect[][] {
  const received: NodeRect[][] = [];
  frame.addEventListener(CANVAS_RECTS, (event) => {
    if (event instanceof CustomEvent) received.push((event.detail as CanvasRectsDetail).rects.slice());
  });
  return received;
}

beforeEach(() => {
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
  for (const frame of frames.splice(0)) frame.remove();
});

describe('the canvas frame layer state', () => {
  // covers: FCT-0037
  it('sends the hidden and locked layers to the harness once the rendering is ready, then whenever they change', async () => {
    const frame = new UiCanvasFrame();
    frame.projectPath = 'E:/kit';
    frame.hiddenLayers = new Set(['0', 'a']);
    frame.lockedLayers = new Set(['b']);
    document.body.append(frame);
    frames.push(frame);
    const rects = recordRects(frame);

    frame.component = openedComponent('v1');
    await expect.poll(() => rects.at(-1)).toEqual([{ id: '0', x: 2, y: 1, width: 1, height: 1 }]);

    frame.hiddenLayers = new Set(['0']);
    frame.lockedLayers = new Set();
    await expect.poll(() => rects.at(-1)).toEqual([{ id: '0', x: 1, y: 0, width: 1, height: 1 }]);
  });
});
