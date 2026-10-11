import { describe, expect, it } from 'vitest';
import { fitView } from '../../../../../src/modules/editor/domain/viewport/fit.js';

describe('fitting the frame to the view', () => {
  // covers: FCT-0034
  it('shrinks a frame wider than the view so that it fits horizontally, centered', () => {
    const view = fitView({ viewportWidth: 800, viewportHeight: 600, frameWidth: 1024, frameHeight: 300 });

    expect(view?.zoom).toBeCloseTo(0.71875, 10);
    expect(view?.x).toBe(32);
    expect(view?.y).toBeGreaterThanOrEqual(64);
  });

  // covers: FCT-0034
  it('never magnifies a frame that already fits', () => {
    expect(fitView({ viewportWidth: 2000, viewportHeight: 1000, frameWidth: 375, frameHeight: 300 })?.zoom).toBe(1);
  });

  // covers: FCT-0034
  it('keeps a tall frame readable instead of shrinking it below 60 %', () => {
    expect(fitView({ viewportWidth: 2000, viewportHeight: 400, frameWidth: 375, frameHeight: 3000 })?.zoom).toBe(0.6);
  });

  // covers: FCT-0034
  it('never goes below 25 %', () => {
    expect(fitView({ viewportWidth: 100, viewportHeight: 400, frameWidth: 4000, frameHeight: 100 })?.zoom).toBe(0.25);
  });

  it('has nothing to fit in an empty view or around an empty frame', () => {
    expect(fitView({ viewportWidth: 0, viewportHeight: 600, frameWidth: 1024, frameHeight: 300 })).toBeNull();
    expect(fitView({ viewportWidth: 800, viewportHeight: 0, frameWidth: 1024, frameHeight: 300 })).toBeNull();
    expect(fitView({ viewportWidth: 800, viewportHeight: 600, frameWidth: 0, frameHeight: 300 })).toBeNull();
  });
});
