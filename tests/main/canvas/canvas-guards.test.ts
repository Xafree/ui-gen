import type { Session, WebContents } from 'electron';
import { describe, expect, it } from 'vitest';
import {
  denyCanvasPermissions,
  guardCanvasFrames,
  isCanvasRequester,
  isHarnessNavigation,
} from '../../../src/main/canvas/canvas-guards';

const HARNESS = 'uigen-canvas://canvas/__studio/harness.html';

interface NavigationEvent {
  readonly url: string;
  readonly isMainFrame: boolean;
  prevented: boolean;
  preventDefault(): void;
}

function navigation(url: string, isMainFrame: boolean): NavigationEvent {
  return {
    url,
    isMainFrame,
    prevented: false,
    preventDefault() {
      this.prevented = true;
    },
  };
}

describe('harness navigations', () => {
  it('recognizes the harness page, whatever its query', () => {
    expect(isHarnessNavigation(HARNESS)).toBe(true);
    expect(isHarnessNavigation(`${HARNESS}?component=src%2Fa.ts&dark=1`)).toBe(true);
  });

  it.each([
    'https://example.com/',
    'uigen-canvas://canvas/src/components/a.ts',
    'uigen-canvas://elsewhere/__studio/harness.html',
    'uigen-canvas://canvas/__studio/harness.js',
    'file:///E:/secret.html',
    'not a url',
  ])('does not recognize %s', (target) => {
    expect(isHarnessNavigation(target)).toBe(false);
  });
});

describe('guarding the frames of the studio window', () => {
  function guarded() {
    const handlers = new Map<string, (event: NavigationEvent) => void>();
    let windowOpenHandler: (() => { action: string }) | null = null;
    const contents = {
      setWindowOpenHandler: (handler: () => { action: string }) => {
        windowOpenHandler = handler;
      },
      on: (name: string, handler: (event: NavigationEvent) => void) => {
        handlers.set(name, handler);
      },
    };
    guardCanvasFrames(contents as unknown as WebContents);
    return { handlers, windowOpen: () => windowOpenHandler?.() };
  }

  it('denies every new window', () => {
    expect(guarded().windowOpen()).toEqual({ action: 'deny' });
  });

  it('lets a frame load the harness page and nothing else', () => {
    const { handlers } = guarded();
    const navigate = handlers.get('will-frame-navigate');
    const harness = navigation(`${HARNESS}?component=a`, false);
    const external = navigation('https://example.com/', false);
    const project = navigation('uigen-canvas://canvas/src/a.ts', false);

    navigate?.(harness);
    navigate?.(external);
    navigate?.(project);

    expect([harness.prevented, external.prevented, project.prevented]).toEqual([false, true, true]);
  });

  it('leaves the navigations of the main frame to their own guard', () => {
    const navigate = guarded().handlers.get('will-frame-navigate');
    const main = navigation('https://example.com/', true);

    navigate?.(main);

    expect(main.prevented).toBe(false);
  });
});

describe('permissions asked by the canvas', () => {
  it.each([
    ['a sandboxed subframe', 'uigen-canvas://canvas/__studio/harness.html', false, true],
    ['a subframe with another url', 'https://example.com/', false, true],
    ['a document of the scheme', 'uigen-canvas://canvas/', true, true],
    ['the studio itself', 'file:///E:/app/index.html', true, false],
    ['the studio without frame information', 'file:///E:/app/index.html', undefined, false],
  ])('recognizes %s', (_label, url, isMainFrame, expected) => {
    expect(isCanvasRequester(url, isMainFrame)).toBe(expected);
  });

  it('denies requests and checks of the canvas, and leaves the studio alone', () => {
    let request: ((...args: unknown[]) => void) | null = null;
    let check: ((...args: unknown[]) => boolean) | null = null;
    const studioSession = {
      setPermissionRequestHandler: (handler: (...args: unknown[]) => void) => {
        request = handler;
      },
      setPermissionCheckHandler: (handler: (...args: unknown[]) => boolean) => {
        check = handler;
      },
    };
    denyCanvasPermissions(studioSession as unknown as Session);

    const answers: boolean[] = [];
    const answer = (granted: boolean): void => {
      answers.push(granted);
    };
    request?.(null, 'geolocation', answer, { requestingUrl: HARNESS, isMainFrame: false });
    request?.(null, 'clipboard-read', answer, { requestingUrl: 'file:///E:/app/index.html', isMainFrame: true });

    expect(answers).toEqual([false, true]);
    expect(check?.(null, 'media', 'null', { requestingUrl: HARNESS, isMainFrame: false })).toBe(false);
    expect(check?.(null, 'media', 'null', { isMainFrame: false })).toBe(false);
    expect(check?.(null, 'clipboard-read', 'file://', { requestingUrl: 'file:///E:/app/index.html', isMainFrame: true })).toBe(
      true,
    );
  });
});
