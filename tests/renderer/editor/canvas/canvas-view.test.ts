import { afterEach, describe, expect, it, vi } from 'vitest';
import type { HarnessMessage } from '../../../../src/modules/canvas/domain/harness-message.js';
import { CanvasView } from '../../../../src/renderer/editor/canvas/canvas-view.js';
import { ANSWERING_HARNESS_URL, SILENT_HARNESS_URL } from './fake-harness.js';

const views: CanvasView[] = [];

function mount(url: string, messages: HarnessMessage[]): CanvasView {
  const view = new CanvasView(url, new Set(['0']), (source, message) => {
    expect(source).toBe(view);
    messages.push(message);
  });
  views.push(view);
  document.body.append(view.iframe);
  return view;
}

afterEach(() => {
  vi.restoreAllMocks();
  for (const view of views.splice(0)) view.dispose();
});

describe('one canvas iframe', () => {
  it('runs sandboxed with scripts only and points at the given document', () => {
    const view = mount(SILENT_HARNESS_URL, []);

    expect(view.iframe.getAttribute('sandbox')).toBe('allow-scripts');
    expect(view.iframe.getAttribute('src')).toBe(SILENT_HARNESS_URL);
    expect(view.url).toBe(SILENT_HARNESS_URL);
  });

  it('passes on the messages of its harness and refuses the unreadable ones without their content', async () => {
    const warned = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const messages: HarnessMessage[] = [];

    mount(ANSWERING_HARNESS_URL, messages);

    await expect.poll(() => messages.length).toBe(2);
    expect(messages[0]).toEqual({ type: 'ready' });
    expect(messages[1]).toEqual({ type: 'rects', rects: [{ id: '0', x: 1, y: 2, width: 3, height: 4 }] });
    expect(warned).toHaveBeenCalledTimes(2);
    expect(warned).toHaveBeenCalledWith('The canvas sent an unreadable message');
  });

  it('asks the harness what is under a point', async () => {
    vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const messages: HarnessMessage[] = [];
    const view = new CanvasView(ANSWERING_HARNESS_URL, new Set(['0']), (source, message) => {
      messages.push(message);
      if (message.type === 'hit') source.settleHit(message.id);
    });
    views.push(view);
    document.body.append(view.iframe);
    await expect.poll(() => messages.length).toBe(2);

    await expect(view.hitTest(5, 6)).resolves.toBe('0');
  });

  it('answers null to a hit test while no harness is connected', async () => {
    const view = new CanvasView(SILENT_HARNESS_URL, new Set(), () => undefined);

    await expect(view.hitTest(1, 1)).resolves.toBeNull();
  });

  it('releases a hit test that is still waiting when it is disposed', async () => {
    vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const messages: HarnessMessage[] = [];
    const view = mount(ANSWERING_HARNESS_URL, messages);
    await expect.poll(() => messages.length).toBe(2);

    const pending = view.hitTest(1, 1);
    view.dispose();

    await expect(pending).resolves.toBeNull();
    expect(view.iframe.isConnected).toBe(false);
  });
});
