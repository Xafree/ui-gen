import { isRecord } from '../json-value.js';
import { TAG_PREFIX_PATTERN } from '../naming/tag-prefix.js';
import { isValidComponentsDir } from './components-dir.js';

export const UIGEN_SETTINGS_FILE = 'uigen.json';
export const CURRENT_FORMAT_VERSION = 1;
export const DEFAULT_COMPONENTS_DIR = 'src/components';

export interface UiGenSettings {
  readonly formatVersion: typeof CURRENT_FORMAT_VERSION;
  readonly tagPrefix: string;
  readonly componentsDir: string;
}

export type SettingsErrorCode =
  | 'invalid-json'
  | 'not-an-object'
  | 'unknown-field'
  | 'missing-field'
  | 'invalid-format-version'
  | 'newer-format-version'
  | 'invalid-tag-prefix'
  | 'invalid-components-dir';

export interface SettingsError {
  readonly code: SettingsErrorCode;
  readonly field: string | null;
}

export type SettingsParseResult =
  | { readonly ok: true; readonly settings: UiGenSettings }
  | { readonly ok: false; readonly errors: readonly SettingsError[] };

interface FieldRule {
  readonly field: keyof UiGenSettings;
  readonly accepts: (value: unknown) => boolean;
  readonly errorCode: (value: unknown) => SettingsErrorCode;
}

function isCurrentFormatVersion(value: unknown): value is typeof CURRENT_FORMAT_VERSION {
  return value === CURRENT_FORMAT_VERSION;
}

function isTagPrefix(value: unknown): value is string {
  return typeof value === 'string' && TAG_PREFIX_PATTERN.test(value);
}

function isComponentsDir(value: unknown): value is string {
  return typeof value === 'string' && isValidComponentsDir(value);
}

function formatVersionErrorCode(value: unknown): SettingsErrorCode {
  const isNewer = typeof value === 'number' && Number.isInteger(value) && value > CURRENT_FORMAT_VERSION;
  return isNewer ? 'newer-format-version' : 'invalid-format-version';
}

const FIELD_RULES: readonly FieldRule[] = [
  { field: 'formatVersion', accepts: isCurrentFormatVersion, errorCode: formatVersionErrorCode },
  { field: 'tagPrefix', accepts: isTagPrefix, errorCode: () => 'invalid-tag-prefix' },
  { field: 'componentsDir', accepts: isComponentsDir, errorCode: () => 'invalid-components-dir' },
];

const SCHEMA_FIELDS: readonly string[] = FIELD_RULES.map(({ field }) => field);

function unknownFieldErrors(record: Readonly<Record<string, unknown>>): SettingsError[] {
  return Object.keys(record)
    .filter((key) => !SCHEMA_FIELDS.includes(key))
    .map((field): SettingsError => ({ code: 'unknown-field', field }));
}

function schemaFieldErrors(record: Readonly<Record<string, unknown>>): SettingsError[] {
  return FIELD_RULES.flatMap(({ field, accepts, errorCode }): SettingsError[] => {
    if (!(field in record)) return [{ code: 'missing-field', field }];
    const value = record[field];
    return accepts(value) ? [] : [{ code: errorCode(value), field }];
  });
}

export function parseUiGenSettings(value: unknown): SettingsParseResult {
  if (!isRecord(value)) return { ok: false, errors: [{ code: 'not-an-object', field: null }] };
  const { formatVersion, tagPrefix, componentsDir } = value;
  const unknownFields = unknownFieldErrors(value);
  if (
    unknownFields.length === 0 &&
    isCurrentFormatVersion(formatVersion) &&
    isTagPrefix(tagPrefix) &&
    isComponentsDir(componentsDir)
  ) {
    return { ok: true, settings: { formatVersion, tagPrefix, componentsDir } };
  }
  return { ok: false, errors: [...unknownFields, ...schemaFieldErrors(value)] };
}

export function serializeUiGenSettings(settings: UiGenSettings): string {
  const { formatVersion, tagPrefix, componentsDir } = settings;
  return `${JSON.stringify({ formatVersion, tagPrefix, componentsDir }, null, 2)}\n`;
}
