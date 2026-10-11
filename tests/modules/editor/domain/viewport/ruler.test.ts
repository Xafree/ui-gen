import { describe, expect, it } from 'vitest';
import { rulerFor } from '../../../../../src/modules/editor/domain/viewport/ruler.js';

describe('rulerFor', () => {
  it('marks every 100 px at 100 % and shows five minor divisions per mark', () => {
    const ruler = rulerFor(320, 0, 1);

    expect(ruler.marks).toEqual([
      { position: 0, label: '0' },
      { position: 100, label: '100' },
      { position: 200, label: '200' },
      { position: 300, label: '300' },
    ]);
    expect(ruler.minor).toBe(20);
    expect(ruler.offset).toBe(0);
  });

  it('starts before the view when the world is shifted and writes negative values with a minus sign', () => {
    const ruler = rulerFor(300, 150, 1);

    expect(ruler.marks[0]).toEqual({ position: -50, label: '−200' });
    expect(ruler.marks.map((mark) => mark.label)).toEqual(['−200', '−100', '0', '100']);
    expect(ruler.offset).toBe(10);
  });

  it('widens the step when zooming out so that marks stay at least 56 px apart', () => {
    const ruler = rulerFor(600, 0, 0.25);

    expect(ruler.marks.map((mark) => mark.label)).toEqual(['0', '250', '500', '750', '1000', '1250', '1500', '1750', '2000', '2250']);
    expect(ruler.minor).toBe(12.5);
  });

  it('narrows the step when zooming in', () => {
    const ruler = rulerFor(200, 0, 4);

    expect(ruler.marks.map((mark) => mark.label)).toEqual(['0', '20', '40']);
  });
});
