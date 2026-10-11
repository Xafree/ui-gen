export type HarnessCommand =
  | { readonly type: 'hit-test'; readonly x: number; readonly y: number }
  | { readonly type: 'dark-preview'; readonly enabled: boolean }
  | { readonly type: 'layer-state'; readonly hidden: readonly string[]; readonly locked: readonly string[] };

type UnknownRecord = Readonly<Record<string, unknown>>;

const MAX_LAYER_IDS = 10_000;
const MAX_LAYER_ID_LENGTH = 512;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasExactKeys(record: UnknownRecord, expected: readonly string[]): boolean {
  const own = Object.keys(record);
  return own.length === expected.length && expected.every((key) => own.includes(key));
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function readLayerIds(value: unknown): string[] | null {
  if (!Array.isArray(value) || value.length > MAX_LAYER_IDS) return null;
  const candidates: readonly unknown[] = value;
  const ids: string[] = [];
  for (const candidate of candidates) {
    if (typeof candidate !== 'string' || candidate.length > MAX_LAYER_ID_LENGTH) return null;
    ids.push(candidate);
  }
  return ids;
}

function readLayerState(data: UnknownRecord): HarnessCommand | null {
  if (!hasExactKeys(data, ['type', 'hidden', 'locked'])) return null;
  const hidden = readLayerIds(data['hidden']);
  const locked = readLayerIds(data['locked']);
  return hidden === null || locked === null ? null : { type: 'layer-state', hidden, locked };
}

export function readHarnessCommand(data: unknown): HarnessCommand | null {
  if (!isRecord(data)) return null;
  switch (data['type']) {
    case 'hit-test': {
      const { x, y } = data;
      return hasExactKeys(data, ['type', 'x', 'y']) && isFiniteNumber(x) && isFiniteNumber(y) ? { type: 'hit-test', x, y } : null;
    }
    case 'dark-preview': {
      const { enabled } = data;
      return hasExactKeys(data, ['type', 'enabled']) && typeof enabled === 'boolean' ? { type: 'dark-preview', enabled } : null;
    }
    case 'layer-state':
      return readLayerState(data);
    default:
      return null;
  }
}
