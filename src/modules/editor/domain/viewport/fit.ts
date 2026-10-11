import { clampZoom, type CanvasView } from './zoom.js';

export interface FitRequest {
  readonly viewportWidth: number;
  readonly viewportHeight: number;
  readonly frameWidth: number;
  readonly frameHeight: number;
}

const PADDING_TOP = 64;
const PADDING_BOTTOM = 72;
const PADDING_SIDE = 32;
const READABLE_FLOOR = 0.6;

export function fitView({ viewportWidth, viewportHeight, frameWidth, frameHeight }: FitRequest): CanvasView | null {
  if (viewportWidth <= 0 || viewportHeight <= 0 || frameWidth <= 0) return null;
  const widthFit = (viewportWidth - PADDING_SIDE * 2) / frameWidth;
  const heightFit = (viewportHeight - PADDING_TOP - PADDING_BOTTOM) / frameHeight;
  const zoom = clampZoom(Math.min(1, widthFit, Math.max(heightFit, Math.min(widthFit, READABLE_FLOOR))));
  const room = viewportHeight - PADDING_TOP - PADDING_BOTTOM;
  return {
    zoom,
    x: Math.round((viewportWidth - frameWidth * zoom) / 2),
    y: Math.round(PADDING_TOP + Math.max(0, (room - frameHeight * zoom) / 2)),
  };
}
