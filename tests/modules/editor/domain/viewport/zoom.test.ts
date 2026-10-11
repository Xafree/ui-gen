import { describe, expect, it } from 'vitest';
import {
  nextZoomStep,
  panBy,
  zoomAfterWheel,
  zoomAround,
  zoomPercent,
  type CanvasView,
} from '../../../../../src/modules/editor/domain/viewport/zoom.js';

describe('zoom steps', () => {
  // covers: FCT-0034
  it('goes up by 125, 150, 200, 300 and 400 % from 100 %, and stops at 400 %', () => {
    const seen: number[] = [];
    let zoom = 1;
    for (let click = 0; click < 6; click += 1) {
      zoom = nextZoomStep(zoom, 1);
      seen.push(zoomPercent(zoom));
    }
    expect(seen).toEqual([125, 150, 200, 300, 400, 400]);
  });

  // covers: FCT-0034
  it('goes down by 75, 67, 50, 33 and 25 % from 100 %, and stops at 25 %', () => {
    const seen: number[] = [];
    let zoom = 1;
    for (let click = 0; click < 6; click += 1) {
      zoom = nextZoomStep(zoom, -1);
      seen.push(zoomPercent(zoom));
    }
    expect(seen).toEqual([75, 67, 50, 33, 25, 25]);
  });

  it('jumps to the next step above or below a zoom that sits between two steps', () => {
    expect(nextZoomStep(1.1, 1)).toBe(1.25);
    expect(nextZoomStep(1.1, -1)).toBe(1);
  });
});

describe('zoom around a point', () => {
  // covers: FCT-0034
  it('keeps the point under the pointer still and never leaves 25 to 400 %', () => {
    const view: CanvasView = { x: 40, y: 20, zoom: 1 };

    expect(zoomAround(view, 2, 140, 120)).toEqual({ x: -60, y: -80, zoom: 2 });
    expect(zoomAround(view, 40, 0, 0).zoom).toBe(4);
    expect(zoomAround(view, 0.01, 0, 0).zoom).toBe(0.25);
  });

  // covers: FCT-0034
  it('zooms in on a wheel pushed up and out on a wheel pulled down', () => {
    expect(zoomAfterWheel(1, -120, 0)).toBeGreaterThan(1);
    expect(zoomAfterWheel(1, 120, 0)).toBeLessThan(1);
    expect(zoomAfterWheel(1, -3, 1)).toBeCloseTo(zoomAfterWheel(1, -48, 0), 10);
  });
});

describe('panning', () => {
  it('moves the view without changing its zoom', () => {
    expect(panBy({ x: 1, y: 2, zoom: 2 }, 10, -4)).toEqual({ x: 11, y: -2, zoom: 2 });
  });
});
