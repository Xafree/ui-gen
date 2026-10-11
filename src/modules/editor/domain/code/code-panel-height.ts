export const DEFAULT_CODE_HEIGHT = 272;
export const MIN_CODE_HEIGHT = 120;
const MAX_CODE_HEIGHT = 600;
const MIN_MAX_CODE_HEIGHT = 160;
const STAGE_MARGIN = 180;
const KEY_STEP = 16;

export function maxCodeHeight(stageHeight: number): number {
  return Math.max(MIN_MAX_CODE_HEIGHT, Math.min(MAX_CODE_HEIGHT, stageHeight - STAGE_MARGIN));
}

export function clampCodeHeight(height: number, maximum: number): number {
  return Math.round(Math.min(Math.max(height, MIN_CODE_HEIGHT), maximum));
}

export function codeHeightAfterKey(key: string, current: number, maximum: number): number | null {
  switch (key) {
    case 'ArrowUp':
      return clampCodeHeight(current + KEY_STEP, maximum);
    case 'ArrowDown':
      return clampCodeHeight(current - KEY_STEP, maximum);
    case 'Home':
      return MIN_CODE_HEIGHT;
    case 'End':
      return maximum;
    default:
      return null;
  }
}
