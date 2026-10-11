import type { Session, WebContents } from 'electron';
import { CANVAS_SCHEME, HARNESS_PAGE_URL } from '../../modules/canvas/domain/canvas-address.js';

export function isHarnessNavigation(target: string): boolean {
  try {
    const url = new URL(target);
    return `${url.protocol}//${url.host}${url.pathname}` === HARNESS_PAGE_URL;
  } catch {
    return false;
  }
}

export function isCanvasRequester(requestingUrl: string, isMainFrame: boolean | undefined): boolean {
  return isMainFrame === false || requestingUrl.startsWith(`${CANVAS_SCHEME}:`);
}

export function guardCanvasFrames(contents: WebContents): void {
  contents.setWindowOpenHandler(() => ({ action: 'deny' }));
  contents.on('will-frame-navigate', (event) => {
    if (event.isMainFrame || isHarnessNavigation(event.url)) return;
    event.preventDefault();
  });
}

export function denyCanvasPermissions(studioSession: Session): void {
  studioSession.setPermissionRequestHandler((_contents, _permission, callback, details) => {
    callback(!isCanvasRequester(details.requestingUrl, details.isMainFrame));
  });
  studioSession.setPermissionCheckHandler((_contents, _permission, requestingOrigin, details) => {
    return !isCanvasRequester(details.requestingUrl ?? requestingOrigin, details.isMainFrame);
  });
}
