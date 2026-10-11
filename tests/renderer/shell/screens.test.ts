import { describe, expect, it } from 'vitest';
import { SCREENS, screenById, type ScreenId } from '../../../src/renderer/shell/screens.js';

describe('studio screens', () => {
  // covers: FCT-0014
  it('lists the five screens of the studio in the order of the top bar', () => {
    expect(SCREENS).toEqual([
      { id: 'editor', label: 'Éditeur', short: 'Éditeur', icon: 'cadre', lot: 1 },
      { id: 'playground', label: 'Playground', short: 'Playground', icon: 'play', lot: 3 },
      { id: 'component-import', label: 'Import de composants', short: 'Composants', icon: 'composants', lot: 7 },
      { id: 'token-import', label: 'Import de tokens', short: 'Tokens', icon: 'tokens', lot: 2 },
      { id: 'export', label: 'Export', short: 'Export', icon: 'package', lot: 5 },
    ]);
  });

  // covers: FCT-0015
  it.each<[ScreenId, string, string, number]>([
    ['editor', 'Éditeur', 'Éditeur', 1],
    ['playground', 'Playground', 'Playground', 3],
    ['component-import', 'Import de composants', 'Composants', 7],
    ['token-import', 'Import de tokens', 'Tokens', 2],
    ['export', 'Export', 'Export', 5],
  ])('finds the %s screen with its label, short label and lot', (id, label, short, lot) => {
    const screen = screenById(id);

    expect(screen.id).toBe(id);
    expect(screen.label).toBe(label);
    expect(screen.short).toBe(short);
    expect(screen.lot).toBe(lot);
  });
});
