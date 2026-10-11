import { describe, expect, it } from 'vitest';
import {
  type ComponentSource,
  proposeComponentsDir,
} from '../../../../../src/modules/project/domain/library/components-dir-proposal';

describe('proposeComponentsDir', () => {
  // covers: FCT-0021
  it('proposes the folder holding every declaring file', () => {
    const sources: ComponentSource[] = [
      { relativePath: 'src/components/hz-button.ts', tags: ['hz-button'] },
      { relativePath: 'src/components/hz-card.ts', tags: ['hz-card'] },
    ];
    expect(proposeComponentsDir(sources)).toBe('src/components');
  });

  // covers: FCT-0021
  it('proposes the longest common folder across nested folders', () => {
    const sources: ComponentSource[] = [
      { relativePath: 'lib/ui/forms/hz-input.ts', tags: ['hz-input'] },
      { relativePath: 'lib/ui/layout/grid/hz-grid.ts', tags: ['hz-grid'] },
      { relativePath: 'lib/ui/hz-button.ts', tags: ['hz-button'] },
    ];
    expect(proposeComponentsDir(sources)).toBe('lib/ui');
  });

  // covers: FCT-0021
  it('proposes the folder of a single declaring file', () => {
    expect(proposeComponentsDir([{ relativePath: 'packages/kit/src/hz-tab.ts', tags: ['hz-tab'] }])).toBe(
      'packages/kit/src',
    );
  });

  // covers: FCT-0021
  it('ignores files that declare no tag', () => {
    const sources: ComponentSource[] = [
      { relativePath: 'src/components/hz-button.ts', tags: ['hz-button'] },
      { relativePath: 'src/components/hz-card.ts', tags: ['hz-card', 'hz-card-header'] },
      { relativePath: 'src/utils/format.ts', tags: [] },
      { relativePath: 'index.ts', tags: [] },
    ];
    expect(proposeComponentsDir(sources)).toBe('src/components');
  });

  // covers: FCT-0021
  it('compares whole folder names, not characters', () => {
    const sources: ComponentSource[] = [
      { relativePath: 'src/comp/hz-a.ts', tags: ['hz-a'] },
      { relativePath: 'src/components/hz-b.ts', tags: ['hz-b'] },
    ];
    expect(proposeComponentsDir(sources)).toBe('src');
  });

  // covers: FCT-0021
  it('proposes the root as a dot when the common folder is the root', () => {
    const sources: ComponentSource[] = [
      { relativePath: 'src/hz-button.ts', tags: ['hz-button'] },
      { relativePath: 'lib/hz-card.ts', tags: ['hz-card'] },
    ];
    expect(proposeComponentsDir(sources)).toBe('.');
  });

  // covers: FCT-0021
  it('proposes the root as a dot for a declaring file at the root', () => {
    const sources: ComponentSource[] = [
      { relativePath: 'hz-button.ts', tags: ['hz-button'] },
      { relativePath: 'src/hz-card.ts', tags: ['hz-card'] },
    ];
    expect(proposeComponentsDir(sources)).toBe('.');
  });

  // covers: FCT-0021
  it('falls back to src/components when no file declares a tag', () => {
    const sources: ComponentSource[] = [
      { relativePath: 'lib/helpers.ts', tags: [] },
      { relativePath: 'lib/ui/theme.ts', tags: [] },
    ];
    expect(proposeComponentsDir(sources)).toBe('src/components');
  });

  // covers: FCT-0021
  it('falls back to src/components without any source', () => {
    expect(proposeComponentsDir([])).toBe('src/components');
  });
});
