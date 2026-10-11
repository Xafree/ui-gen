export interface RulerMark {
  readonly position: number;
  readonly label: string;
}

export interface Ruler {
  readonly marks: readonly RulerMark[];
  readonly minor: number;
  readonly offset: number;
}

const STEPS = [5, 10, 20, 25, 50, 100, 200, 250, 500, 1000];
const WIDEST_STEP = 1000;
const MIN_MARK_SPACING_PX = 56;
const MINOR_DIVISIONS = 5;

function rounded(value: number, digits: number): number {
  const scale = 10 ** digits;
  return Math.round(value * scale) / scale;
}

export function rulerFor(length: number, offset: number, zoom: number): Ruler {
  const step = STEPS.find((candidate) => candidate * zoom >= MIN_MARK_SPACING_PX) ?? WIDEST_STEP;
  const minor = rounded((step * zoom) / MINOR_DIVISIONS, 2);
  const marks: RulerMark[] = [];
  const first = Math.floor(-offset / zoom / step) * step;
  const last = (length - offset) / zoom;
  for (let value = first; value <= last; value += step) {
    marks.push({ position: rounded(offset + value * zoom, 1), label: String(value).replace('-', '−') });
  }
  return { marks, minor, offset: rounded(((offset % minor) + minor) % minor, 2) };
}
