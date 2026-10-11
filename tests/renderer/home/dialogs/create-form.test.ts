import { describe, expect, it } from 'vitest';
import type { TagPrefixError } from '../../../../src/modules/project/domain/naming/tag-prefix.js';
import {
  deriveCreateForm,
  tagPrefixMessage,
  type CreateFormInput,
  type CreateFormView,
} from '../../../../src/renderer/home/dialogs/create-form.js';

function input(overrides: Partial<CreateFormInput>): CreateFormInput {
  return {
    name: '',
    location: 'E:\\projets',
    packageName: '',
    tagPrefix: '',
    packageEdited: false,
    prefixEdited: false,
    nameTouched: false,
    ...overrides,
  };
}

function withPrefix(tagPrefix: string): CreateFormView {
  return deriveCreateForm(input({ name: 'Kit Horizon', packageName: '@kit-horizon/ui', tagPrefix, prefixEdited: true }));
}

describe('deriveCreateForm proposals', () => {
  // covers: FCT-0002
  it('proposes @kit-horizon/ui and kh- for « Kit Horizon », with the project folder and the example tag', () => {
    const view = deriveCreateForm(input({ name: 'Kit Horizon' }));

    expect(view).toEqual({
      packageName: '@kit-horizon/ui',
      tagPrefix: 'kh-',
      nameError: null,
      packageError: null,
      prefixError: null,
      showPackageReset: false,
      showPrefixReset: false,
      previewPath: 'E:\\projets\\kit-horizon',
      previewTag: '<kh-button>',
      previewTagPending: false,
      canSubmit: true,
    });
  });

  // covers: FCT-0002
  it('keeps proposing from the name while the fields were not edited, whatever they held before', () => {
    const view = deriveCreateForm(input({ name: 'Lumen UI', packageName: '@kit-horizon/ui', tagPrefix: 'kh-' }));

    expect(view.packageName).toBe('@lumen-ui/ui');
    expect(view.tagPrefix).toBe('lu-');
    expect(view.previewPath).toBe('E:\\projets\\lumen-ui');
    expect(view.previewTag).toBe('<lu-button>');
  });

  // covers: FCT-0002
  it('removes accents from the proposals and takes the two first letters of a single word', () => {
    expect(deriveCreateForm(input({ name: 'Éclat Café' }))).toMatchObject({
      packageName: '@eclat-cafe/ui',
      tagPrefix: 'ec-',
    });
    expect(deriveCreateForm(input({ name: 'Lumen' }))).toMatchObject({
      packageName: '@lumen/ui',
      tagPrefix: 'lu-',
    });
  });

  // covers: FCT-0002
  it('keeps the package name and the prefix the user edited, and offers to take the proposal back', () => {
    const view = deriveCreateForm(
      input({
        name: 'Kit Horizon',
        packageName: '@horizon/ui',
        tagPrefix: 'hz-',
        packageEdited: true,
        prefixEdited: true,
      }),
    );

    expect(view).toEqual({
      packageName: '@horizon/ui',
      tagPrefix: 'hz-',
      nameError: null,
      packageError: null,
      prefixError: null,
      showPackageReset: true,
      showPrefixReset: true,
      previewPath: 'E:\\projets\\horizon',
      previewTag: '<hz-button>',
      previewTagPending: false,
      canSubmit: true,
    });
  });

  // covers: FCT-0002
  it('does not offer to take the proposal back when the edited value equals it', () => {
    const view = deriveCreateForm(
      input({
        name: 'Kit Horizon',
        packageName: '@kit-horizon/ui',
        tagPrefix: 'kh-',
        packageEdited: true,
        prefixEdited: true,
      }),
    );

    expect(view.showPackageReset).toBe(false);
    expect(view.showPrefixReset).toBe(false);
  });

  // covers: FCT-0002
  it('does not offer to take the proposal back when the name gives no proposal', () => {
    const view = deriveCreateForm(
      input({ name: '', packageName: '@horizon/ui', tagPrefix: 'hz-', packageEdited: true, prefixEdited: true }),
    );

    expect(view.packageName).toBe('@horizon/ui');
    expect(view.tagPrefix).toBe('hz-');
    expect(view.showPackageReset).toBe(false);
    expect(view.showPrefixReset).toBe(false);
  });

  it('builds the project folder in the chosen location', () => {
    expect(deriveCreateForm(input({ name: 'Kit Horizon', location: 'D:\\clients' })).previewPath).toBe(
      'D:\\clients\\kit-horizon',
    );
  });
});

describe('deriveCreateForm empty and invalid name', () => {
  it('shows no error on a fresh form, asks for a name in the preview and forbids the creation', () => {
    expect(deriveCreateForm(input({}))).toEqual({
      packageName: '',
      tagPrefix: '',
      nameError: null,
      packageError: null,
      prefixError: null,
      showPackageReset: false,
      showPrefixReset: false,
      previewPath: null,
      previewTag: 'Indiquez un préfixe',
      previewTagPending: true,
      canSubmit: false,
    });
  });

  it('asks for the project name once the name field was left empty', () => {
    expect(deriveCreateForm(input({ nameTouched: true })).nameError).toBe('Indiquez le nom du projet.');
    const blank = deriveCreateForm(input({ name: '   ', nameTouched: true }));
    expect(blank.nameError).toBe('Indiquez le nom du projet.');
    expect(blank.canSubmit).toBe(false);
  });

  it('forbids the creation of a blank name even before the field was left', () => {
    const view = deriveCreateForm(input({ name: '   ' }));

    expect(view.nameError).toBeNull();
    expect(view.canSubmit).toBe(false);
  });

  it('refuses a name without any letter or digit', () => {
    const view = deriveCreateForm(input({ name: '!!! ---' }));

    expect(view.nameError).toBe('Le nom doit contenir au moins une lettre ou un chiffre.');
    expect(view.packageError).toBeNull();
    expect(view.prefixError).toBeNull();
    expect(view.previewPath).toBeNull();
    expect(view.canSubmit).toBe(false);
  });

  it('forbids the creation when the name gives no prefix, without blaming the untouched prefix', () => {
    const view = deriveCreateForm(input({ name: '2026' }));

    expect(view.packageName).toBe('@2026/ui');
    expect(view.tagPrefix).toBe('');
    expect(view.prefixError).toBeNull();
    expect(view.previewTag).toBe('Indiquez un préfixe');
    expect(view.previewTagPending).toBe(true);
    expect(view.canSubmit).toBe(false);
  });
});

describe('deriveCreateForm package name', () => {
  it('asks for a package name with the proposal as example when the user emptied it', () => {
    const view = deriveCreateForm(input({ name: 'Lumen UI', packageName: '', packageEdited: true }));

    expect(view.packageName).toBe('');
    expect(view.packageError).toBe('Indiquez un nom de package, par exemple @lumen-ui/ui.');
    expect(view.showPackageReset).toBe(true);
    expect(view.canSubmit).toBe(false);
  });

  it('refuses an invalid npm package name with the proposal as example', () => {
    const view = deriveCreateForm(input({ name: 'Lumen UI', packageName: 'Lumen UI', packageEdited: true }));

    expect(view.packageError).toBe('Nom de package npm invalide : minuscules, sans espace, par exemple @lumen-ui/ui.');
    expect(view.canSubmit).toBe(false);
  });

  it('falls back to @kit-horizon/ui as example when the name gives no proposal', () => {
    expect(deriveCreateForm(input({ packageName: '', packageEdited: true })).packageError).toBe(
      'Indiquez un nom de package, par exemple @kit-horizon/ui.',
    );
    expect(deriveCreateForm(input({ packageName: 'Mon Kit', packageEdited: true })).packageError).toBe(
      'Nom de package npm invalide : minuscules, sans espace, par exemple @kit-horizon/ui.',
    );
  });
});

describe('deriveCreateForm tag prefix', () => {
  // covers: FCT-0005
  it('explains that the prefix is written in lower case', () => {
    const view = withPrefix('KH-');

    expect(view.prefixError).toBe("Le préfixe s'écrit en minuscules : kh- au lieu de KH-.");
    expect(view.canSubmit).toBe(false);
  });

  // covers: FCT-0005
  it('explains that the prefix starts with a letter', () => {
    expect(withPrefix('1kh-').prefixError).toBe('Le préfixe commence par une lettre, par exemple kh-.');
    expect(withPrefix('-kh-').prefixError).toBe('Le préfixe commence par une lettre, par exemple kh-.');
    expect(withPrefix('-kh-').canSubmit).toBe(false);
  });

  // covers: FCT-0005
  it('explains which characters a prefix may hold', () => {
    const view = withPrefix('k_h-');

    expect(view.prefixError).toBe('Utilisez seulement des lettres minuscules, des chiffres et des tirets.');
    expect(withPrefix('kh é-').prefixError).toBe(
      'Utilisez seulement des lettres minuscules, des chiffres et des tirets.',
    );
    expect(view.canSubmit).toBe(false);
  });

  // covers: FCT-0005
  it('asks for the trailing dash', () => {
    const view = withPrefix('kh');

    expect(view.prefixError).toBe('Ajoutez un tiret à la fin : kh-.');
    expect(view.canSubmit).toBe(false);
  });

  // covers: FCT-0005
  it('refuses two dashes in a row, at the end or in the middle', () => {
    expect(withPrefix('kh--').prefixError).toBe('Retirez le double tiret : un seul tiret sépare le préfixe du nom.');
    expect(withPrefix('kh--').canSubmit).toBe(false);
    expect(withPrefix('k--h-').prefixError).toBe('Retirez le double tiret : un seul tiret sépare le préfixe du nom.');
    expect(withPrefix('k--h-').canSubmit).toBe(false);
  });

  // covers: FCT-0005
  it('asks for a prefix when the user emptied it', () => {
    const view = withPrefix('');

    expect(view.prefixError).toBe('Indiquez un préfixe, par exemple kh-.');
    expect(view.previewTag).toBe('Indiquez un préfixe');
    expect(view.previewTagPending).toBe(true);
    expect(view.canSubmit).toBe(false);
  });

  // covers: FCT-0005
  it('asks to fix the prefix in the example tag while it is invalid', () => {
    const view = withPrefix('kh');

    expect(view.previewTag).toBe('Corrigez le préfixe');
    expect(view.previewTagPending).toBe(true);
  });

  // covers: FCT-0005
  it('accepts a prefix with digits and inner dashes', () => {
    const view = withPrefix('kh2-ui-');

    expect(view.prefixError).toBeNull();
    expect(view.previewTag).toBe('<kh2-ui-button>');
    expect(view.previewTagPending).toBe(false);
    expect(view.canSubmit).toBe(true);
  });
});

describe('tagPrefixMessage', () => {
  const cases: readonly (readonly [TagPrefixError, string, string])[] = [
    ['empty', '', 'Indiquez un préfixe, par exemple lu-.'],
    ['uppercase', 'Lu-', "Le préfixe s'écrit en minuscules : lu- au lieu de Lu-."],
    ['not-starting-with-letter', '9lu-', 'Le préfixe commence par une lettre, par exemple lu-.'],
    ['invalid-characters', 'l.u-', 'Utilisez seulement des lettres minuscules, des chiffres et des tirets.'],
    ['missing-trailing-dash', 'lu', 'Ajoutez un tiret à la fin : lu-.'],
    ['double-dash', 'lu--', 'Retirez le double tiret : un seul tiret sépare le préfixe du nom.'],
  ];

  // covers: FCT-0005
  it.each(cases)('explains the rule broken by %s', (error, value, expected) => {
    expect(tagPrefixMessage(error, value, 'lu-')).toBe(expected);
  });

  // covers: FCT-0005
  it('gives kh- as example when there is no proposal', () => {
    expect(tagPrefixMessage('empty', '', '')).toBe('Indiquez un préfixe, par exemple kh-.');
    expect(tagPrefixMessage('not-starting-with-letter', '-', '')).toBe(
      'Le préfixe commence par une lettre, par exemple kh-.',
    );
  });
});
