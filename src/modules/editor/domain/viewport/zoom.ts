export interface CanvasView {
  readonly x: number;
  readonly y: number;
  readonly zoom: number;
}

export const ZOOM_MIN = 0.25;
export const ZOOM_MAX = 4;
export const ZOOM_STEPS: readonly number[] = [0.25, 0.33, 0.5, 0.67, 0.75, 1, 1.25, 1.5, 2, 3, 4];

const STEP_TOLERANCE = 0.001;
const WHEEL_SENSITIVITY = 0.0022;
const LINE_HEIGHT_PX = 16;
const LINE_DELTA_MODE = 1;

export function clampZoom(zoom: number): number {
  return Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, zoom));
}

export function zoomPercent(zoom: number): number {
  return Math.round(zoom * 100);
}

export function nextZoomStep(zoom: number, direction: 1 | -1): number {
  const next =
    direction > 0
      ? ZOOM_STEPS.find((step) => step > zoom + STEP_TOLERANCE)
      : ZOOM_STEPS.findLast((step) => step < zoom - STEP_TOLERANCE);
  return next ?? zoom;
}

export function zoomAround(view: CanvasView, zoom: number, centerX: number, centerY: number): CanvasView {
  const next = clampZoom(zoom);
  const worldX = (centerX - view.x) / view.zoom;
  const worldY = (centerY - view.y) / view.zoom;
  return { x: centerX - worldX * next, y: centerY - worldY * next, zoom: next };
}

export function zoomAfterWheel(zoom: number, deltaY: number, deltaMode: number): number {
  const unit = deltaMode === LINE_DELTA_MODE ? LINE_HEIGHT_PX : 1;
  return zoom * Math.exp(-deltaY * unit * WHEEL_SENSITIVITY);
}

export function panBy(view: CanvasView, deltaX: number, deltaY: number): CanvasView {
  return { ...view, x: view.x + deltaX, y: view.y + deltaY };
}
