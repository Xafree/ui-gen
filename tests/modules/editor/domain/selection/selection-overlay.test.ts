import { describe, expect, it } from 'vitest';
import {
  isSmall,
  selectionGuides,
  sizeLabel,
  toViewportBox,
} from '../../../../../src/modules/editor/domain/selection/selection-overlay';

describe('selection overlay', () => {
  // covers: FCT-0036
  it('places a rectangle of the canvas in the view from the frame origin and the zoom', () => {
    expect(toViewportBox({ x: 10, y: 20, width: 100, height: 40 }, { x: 300, y: 50 }, 0.5)).toEqual({ x: 305, y: 60, width: 50, height: 20 });
  });

  // covers: FCT-0036
  it('writes the CSS size, rounded', () => {
    expect(sizeLabel({ x: 0, y: 0, width: 65.48, height: 21.6 })).toBe('65 × 22');
  });

  // covers: FCT-0036
  it('calls small a box under 28 px on one side', () => {
    expect(isSmall({ x: 0, y: 0, width: 27, height: 100 })).toBe(true);
    expect(isSmall({ x: 0, y: 0, width: 100, height: 27 })).toBe(true);
    expect(isSmall({ x: 0, y: 0, width: 28, height: 28 })).toBe(false);
  });

  describe('guides', () => {
    const parent = { x: 0, y: 0, width: 400, height: 300 };
    const selected = { x: 40, y: 100, width: 100, height: 50 };

    // covers: FCT-0036
    it('measures the distance to the left and to the bottom of the parent, in CSS pixels', () => {
      expect(selectionGuides(selected, parent, null, 2)).toEqual([
        { axis: 'horizontal', x: 0, y: 125, length: 40, label: 20 },
        { axis: 'vertical', x: 90, y: 150, length: 150, label: 75 },
      ]);
    });

    // covers: FCT-0036
    it('measures the gap to the previous sibling', () => {
      const previous = { x: 40, y: 20, width: 100, height: 40 };

      expect(selectionGuides(selected, parent, previous, 1)).toContainEqual({ axis: 'vertical', x: 90, y: 60, length: 40, label: 40 });
    });

    // covers: FCT-0036
    it('draws nothing for a distance of 10 px or less, nor without a parent', () => {
      expect(selectionGuides({ x: 10, y: 285, width: 100, height: 10 }, parent, null, 1)).toEqual([]);
      expect(selectionGuides(selected, null, { x: 0, y: 0, width: 10, height: 10 }, 1)).toEqual([]);
    });
  });
});
