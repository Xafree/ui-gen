export type ScreenSizeId = 'mobile' | 'tablette' | 'ordinateur';

export interface ScreenSize {
  readonly id: ScreenSizeId;
  readonly label: string;
  readonly width: number;
}

export const SCREEN_SIZES: readonly ScreenSize[] = [
  { id: 'mobile', label: 'Mobile', width: 375 },
  { id: 'tablette', label: 'Tablette', width: 768 },
  { id: 'ordinateur', label: 'Ordinateur', width: 1024 },
];

const NO_BREAK_SPACE = String.fromCodePoint(0xa0);
const NARROW_NO_BREAK_SPACE = String.fromCodePoint(0x202f);

export const DEFAULT_SCREEN_SIZE: ScreenSizeId = 'ordinateur';

export function screenSizeById(id: ScreenSizeId): ScreenSize {
  const size = SCREEN_SIZES.find((candidate) => candidate.id === id);
  if (size === undefined) throw new Error(`Unknown screen size: ${id}`);
  return size;
}

export function formatPixels(width: number): string {
  return `${new Intl.NumberFormat('fr-FR').format(width).replaceAll(NARROW_NO_BREAK_SPACE, NO_BREAK_SPACE)}${NO_BREAK_SPACE}px`;
}
