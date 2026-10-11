export interface Box {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

export interface Point {
  readonly x: number;
  readonly y: number;
}

export interface Guide {
  readonly axis: 'horizontal' | 'vertical';
  readonly x: number;
  readonly y: number;
  readonly length: number;
  readonly label: number;
}

const SMALL_SIDE = 28;
const GUIDE_MIN_LENGTH = 10;

export function toViewportBox(rect: Box, origin: Point, zoom: number): Box {
  return { x: origin.x + rect.x * zoom, y: origin.y + rect.y * zoom, width: rect.width * zoom, height: rect.height * zoom };
}

export function sizeLabel(rect: Box): string {
  return `${String(Math.round(rect.width))} × ${String(Math.round(rect.height))}`;
}

export function isSmall(box: Box): boolean {
  return box.width < SMALL_SIDE || box.height < SMALL_SIDE;
}

function guide(axis: Guide['axis'], x: number, y: number, length: number, zoom: number): Guide[] {
  const label = Math.round(length / zoom);
  return length > GUIDE_MIN_LENGTH && label >= 1 ? [{ axis, x, y, length, label }] : [];
}

export function selectionGuides(selected: Box, parent: Box | null, previous: Box | null, zoom: number): Guide[] {
  const centerX = selected.x + selected.width / 2;
  const bottom = selected.y + selected.height;
  if (parent === null) return [];
  const previousBottom = previous === null ? null : previous.y + previous.height;
  return [
    ...guide('horizontal', parent.x, selected.y + selected.height / 2, selected.x - parent.x, zoom),
    ...guide('vertical', centerX, bottom, parent.y + parent.height - bottom, zoom),
    ...(previousBottom === null ? [] : guide('vertical', centerX, previousBottom, selected.y - previousBottom, zoom)),
  ];
}
