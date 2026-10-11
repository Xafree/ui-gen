import { protocol } from 'electron';
import { CANVAS_SCHEME } from '../../modules/canvas/domain/canvas-address.js';

export function registerCanvasScheme(): void {
  protocol.registerSchemesAsPrivileged([
    {
      scheme: CANVAS_SCHEME,
      privileges: { standard: true, secure: true, supportFetchAPI: true, corsEnabled: true },
    },
  ]);
}

export function handleCanvasProtocol(handler: (request: Request) => Promise<Response>): void {
  protocol.handle(CANVAS_SCHEME, handler);
}
