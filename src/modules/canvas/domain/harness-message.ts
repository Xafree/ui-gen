export interface NodeRect {
  readonly id: string;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

export type HarnessMessage =
  | { readonly type: 'ready' }
  | { readonly type: 'rects'; readonly rects: readonly NodeRect[] }
  | { readonly type: 'hit'; readonly id: string | null }
  | { readonly type: 'error' };

export const MAX_RECT_VALUE = 100_000;

type UnknownRecord = Readonly<Record<string, unknown>>;

const TYPE_ONLY_KEYS = ['type'] as const;
const RECTS_KEYS = ['type', 'rects'] as const;
const HIT_KEYS = ['type', 'id'] as const;
const RECT_KEYS = ['id', 'x', 'y', 'width', 'height'] as const;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasExactKeys(record: UnknownRecord, expected: readonly string[]): boolean {
  const own = Object.keys(record);
  return own.length === expected.length && expected.every((key) => own.includes(key));
}

function isKnownId(value: unknown, knownIds: ReadonlySet<string>): value is string {
  return typeof value === 'string' && knownIds.has(value);
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function clamped(value: number): number {
  return Math.min(Math.max(value, -MAX_RECT_VALUE), MAX_RECT_VALUE);
}

function readRect(value: unknown, knownIds: ReadonlySet<string>): NodeRect | null {
  if (!isRecord(value) || !hasExactKeys(value, RECT_KEYS)) return null;
  const { id, x, y, width, height } = value;
  if (!isKnownId(id, knownIds)) return null;
  const isFiniteBox = isFiniteNumber(x) && isFiniteNumber(y) && isFiniteNumber(width) && isFiniteNumber(height);
  return isFiniteBox
    ? { id, x: clamped(x), y: clamped(y), width: clamped(width), height: clamped(height) }
    : null;
}

function readRects(value: unknown, knownIds: ReadonlySet<string>): NodeRect[] | null {
  if (!Array.isArray(value)) return null;
  const candidates: readonly unknown[] = value;
  const rects: NodeRect[] = [];
  for (const candidate of candidates) {
    const rect = readRect(candidate, knownIds);
    if (rect === null) return null;
    rects.push(rect);
  }
  return rects;
}

function readRectsMessage(message: UnknownRecord, knownIds: ReadonlySet<string>): HarnessMessage | null {
  if (!hasExactKeys(message, RECTS_KEYS)) return null;
  const rects = readRects(message['rects'], knownIds);
  return rects === null ? null : { type: 'rects', rects };
}

function readHitMessage(message: UnknownRecord, knownIds: ReadonlySet<string>): HarnessMessage | null {
  if (!hasExactKeys(message, HIT_KEYS)) return null;
  const id = message['id'];
  return id === null || isKnownId(id, knownIds) ? { type: 'hit', id } : null;
}

export function readHarnessMessage(data: unknown, knownIds: ReadonlySet<string>): HarnessMessage | null {
  if (!isRecord(data)) return null;
  switch (data['type']) {
    case 'ready':
      return hasExactKeys(data, TYPE_ONLY_KEYS) ? { type: 'ready' } : null;
    case 'rects':
      return readRectsMessage(data, knownIds);
    case 'hit':
      return readHitMessage(data, knownIds);
    case 'error':
      return hasExactKeys(data, TYPE_ONLY_KEYS) ? { type: 'error' } : null;
    default:
      return null;
  }
}
