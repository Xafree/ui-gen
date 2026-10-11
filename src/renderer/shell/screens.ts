import type { IconName } from '../icons/icon-paths.js';

export type ScreenId = 'editor' | 'playground' | 'component-import' | 'token-import' | 'export';

export interface StudioScreen {
  readonly id: ScreenId;
  readonly label: string;
  readonly short: string;
  readonly icon: IconName;
  readonly lot: number;
}

export const SCREENS: readonly StudioScreen[] = [
  { id: 'editor', label: 'Éditeur', short: 'Éditeur', icon: 'cadre', lot: 1 },
  { id: 'playground', label: 'Playground', short: 'Playground', icon: 'play', lot: 3 },
  { id: 'component-import', label: 'Import de composants', short: 'Composants', icon: 'composants', lot: 7 },
  { id: 'token-import', label: 'Import de tokens', short: 'Tokens', icon: 'tokens', lot: 2 },
  { id: 'export', label: 'Export', short: 'Export', icon: 'package', lot: 5 },
];

export function screenById(id: ScreenId): StudioScreen {
  const screen = SCREENS.find((candidate) => candidate.id === id);
  if (screen === undefined) throw new Error(`Unknown studio screen: ${id}`);
  return screen;
}
