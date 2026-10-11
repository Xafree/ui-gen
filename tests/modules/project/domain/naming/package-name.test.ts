import { describe, expect, it } from 'vitest';
import {
  projectFolderName,
  suggestPackageName,
  validatePackageName,
  type PackageNameError,
} from '../../../../../src/modules/project/domain/naming/package-name';

describe('suggestPackageName', () => {
  // covers: FCT-0002
  it('proposes a scoped ui package from the project name', () => {
    expect(suggestPackageName('Kit Horizon')).toBe('@kit-horizon/ui');
  });

  // covers: FCT-0002
  it('lowercases a single-word name', () => {
    expect(suggestPackageName('Atlas')).toBe('@atlas/ui');
  });

  // covers: FCT-0002
  it('removes accents from the words', () => {
    expect(suggestPackageName('Équipe Café')).toBe('@equipe-cafe/ui');
  });

  // covers: FCT-0002
  it('ignores surrounding and repeated spaces between words', () => {
    expect(suggestPackageName('  Kit   Horizon  ')).toBe('@kit-horizon/ui');
  });

  // covers: FCT-0002
  it('keeps digits as part of the words', () => {
    expect(suggestPackageName('Kit 2')).toBe('@kit-2/ui');
  });

  // covers: FCT-0002
  it('keeps an already dashed name apart from its case', () => {
    expect(suggestPackageName('Kit-Horizon')).toBe('@kit-horizon/ui');
  });

  // covers: FCT-0002
  it('proposes a name that passes validation', () => {
    expect(validatePackageName(suggestPackageName('Kit Horizon'))).toBeNull();
  });

  it.each(['', '   ', '---', '!?'])('proposes nothing for %j, which has no letter nor digit', (name) => {
    expect(suggestPackageName(name)).toBe('');
  });
});

describe('validatePackageName', () => {
  // covers: FCT-0002
  it.each(['@kit-horizon/ui', '@atlas/components', 'my-lib', 'kit2', '@kit.horizon/ui_core'])(
    'accepts the edited npm name %j',
    (value) => {
      expect(validatePackageName(value)).toBeNull();
    },
  );

  // covers: FCT-0002
  it('refuses an empty name as empty', () => {
    const expected: PackageNameError = 'empty';
    expect(validatePackageName('')).toBe(expected);
  });

  // covers: FCT-0002
  it.each([
    'Kit-Horizon',
    '@Kit-Horizon/ui',
    '@kit-horizon/UI',
    'kit horizon',
    '@kit-horizon',
    '@/ui',
    '@kit-horizon/',
    'kit/ui',
    '@kit/horizon/ui',
    '.kit',
    '_kit',
    'kit!',
    'kité',
  ])('refuses %j as invalid', (value) => {
    expect(validatePackageName(value)).toBe('invalid');
  });
});

describe('projectFolderName', () => {
  it('drops the leading at sign and the trailing ui of the package', () => {
    expect(projectFolderName('@kit-horizon/ui')).toBe('kit-horizon');
  });

  it('keeps an unscoped package name as is', () => {
    expect(projectFolderName('my-lib')).toBe('my-lib');
  });

  it('turns the remaining slash into a dash', () => {
    expect(projectFolderName('@atlas/components')).toBe('atlas-components');
  });

  it('only drops ui when it is the whole last segment', () => {
    expect(projectFolderName('@atlas/ui-kit')).toBe('atlas-ui-kit');
  });
});
