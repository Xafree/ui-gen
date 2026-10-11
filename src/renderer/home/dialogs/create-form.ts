import { projectFolderName, suggestPackageName, validatePackageName } from '../../../modules/project/domain/naming/package-name.js';
import { validateProjectName } from '../../../modules/project/domain/naming/project-name.js';
import { suggestTagPrefix, validateTagPrefix, type TagPrefixError } from '../../../modules/project/domain/naming/tag-prefix.js';

export interface CreateFormInput {
  readonly name: string;
  readonly location: string;
  readonly packageName: string;
  readonly tagPrefix: string;
  readonly packageEdited: boolean;
  readonly prefixEdited: boolean;
  readonly nameTouched: boolean;
}

export interface CreateFormView {
  readonly packageName: string;
  readonly tagPrefix: string;
  readonly nameError: string | null;
  readonly packageError: string | null;
  readonly prefixError: string | null;
  readonly showPackageReset: boolean;
  readonly showPrefixReset: boolean;
  readonly previewPath: string | null;
  readonly previewTag: string;
  readonly previewTagPending: boolean;
  readonly canSubmit: boolean;
}

const FALLBACK_PACKAGE = '@kit-horizon/ui';
const FALLBACK_PREFIX = 'kh-';
const TRAILING_SEPARATORS = /[\\/]+$/;

export function tagPrefixMessage(error: TagPrefixError, value: string, suggestion: string): string {
  const example = suggestion !== '' && validateTagPrefix(suggestion) === null ? suggestion : FALLBACK_PREFIX;
  switch (error) {
    case 'empty':
      return `Indiquez un préfixe, par exemple ${example}.`;
    case 'uppercase':
      return `Le préfixe s'écrit en minuscules : ${value.toLowerCase()} au lieu de ${value}.`;
    case 'not-starting-with-letter':
      return `Le préfixe commence par une lettre, par exemple ${example}.`;
    case 'invalid-characters':
      return 'Utilisez seulement des lettres minuscules, des chiffres et des tirets.';
    case 'missing-trailing-dash':
      return `Ajoutez un tiret à la fin : ${value}-.`;
    case 'double-dash':
      return 'Retirez le double tiret : un seul tiret sépare le préfixe du nom.';
  }
}

function nameMessage(name: string, touched: boolean): string | null {
  const error = validateProjectName(name);
  if (error === 'empty') return touched ? 'Indiquez le nom du projet.' : null;
  return error === 'no-alphanumeric' ? 'Le nom doit contenir au moins une lettre ou un chiffre.' : null;
}

function packageMessage(value: string, suggestion: string): string | null {
  const example = suggestion === '' ? FALLBACK_PACKAGE : suggestion;
  const error = validatePackageName(value);
  if (error === 'empty') return `Indiquez un nom de package, par exemple ${example}.`;
  return error === 'invalid' ? `Nom de package npm invalide : minuscules, sans espace, par exemple ${example}.` : null;
}

function previewTagText(tagPrefix: string, prefixError: string | null): string {
  if (tagPrefix !== '' && prefixError === null) return `<${tagPrefix}button>`;
  return tagPrefix === '' ? 'Indiquez un préfixe' : 'Corrigez le préfixe';
}

export function deriveCreateForm(input: CreateFormInput): CreateFormView {
  const suggestedPackage = suggestPackageName(input.name);
  const suggestedPrefix = suggestTagPrefix(input.name);
  const packageName = input.packageEdited ? input.packageName : suggestedPackage;
  const tagPrefix = input.prefixEdited ? input.tagPrefix : suggestedPrefix;
  const nameError = nameMessage(input.name, input.nameTouched);
  const packageError = input.packageEdited || packageName !== '' ? packageMessage(packageName, suggestedPackage) : null;
  const prefixCode = input.prefixEdited || tagPrefix !== '' ? validateTagPrefix(tagPrefix) : null;
  const prefixError = prefixCode === null ? null : tagPrefixMessage(prefixCode, tagPrefix, suggestedPrefix);
  const previewPath =
    packageName !== '' && packageError === null
      ? `${input.location.replace(TRAILING_SEPARATORS, '')}\\${projectFolderName(packageName)}`
      : null;
  return {
    packageName,
    tagPrefix,
    nameError,
    packageError,
    prefixError,
    showPackageReset: input.packageEdited && suggestedPackage !== '' && packageName !== suggestedPackage,
    showPrefixReset: input.prefixEdited && suggestedPrefix !== '' && tagPrefix !== suggestedPrefix,
    previewPath,
    previewTag: previewTagText(tagPrefix, prefixError),
    previewTagPending: tagPrefix === '' || prefixError !== null,
    canSubmit:
      input.name.trim() !== '' &&
      nameError === null &&
      packageError === null &&
      prefixError === null &&
      packageName !== '' &&
      tagPrefix !== '',
  };
}
