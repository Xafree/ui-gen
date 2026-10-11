import { describe, expect, it } from 'vitest';
import {
  CURRENT_FORMAT_VERSION,
  DEFAULT_COMPONENTS_DIR,
  UIGEN_SETTINGS_FILE,
  parseUiGenSettings,
  serializeUiGenSettings,
  type SettingsError,
  type SettingsParseResult,
  type UiGenSettings,
} from '../../../../../src/modules/project/domain/settings/uigen-settings';

const kitHorizonFile: unknown = JSON.parse(`{
  "formatVersion": 1,
  "tagPrefix": "kh-",
  "componentsDir": "src/components"
}
`);

function errorsOf(result: SettingsParseResult): readonly SettingsError[] {
  if (result.ok) throw new Error('expected the settings to be rejected');
  return result.errors;
}

function settingsOf(result: SettingsParseResult): UiGenSettings {
  if (!result.ok) throw new Error(`expected valid settings, got ${JSON.stringify(result.errors)}`);
  return result.settings;
}

describe('uigen.json constants', () => {
  // covers: FCT-0003
  it('names the settings file uigen.json', () => {
    expect(UIGEN_SETTINGS_FILE).toBe('uigen.json');
  });

  // covers: FCT-0003
  it('knows format version 1', () => {
    expect(CURRENT_FORMAT_VERSION).toBe(1);
  });

  // covers: FCT-0003
  it('puts components in src/components by default', () => {
    expect(DEFAULT_COMPONENTS_DIR).toBe('src/components');
  });
});

describe('parseUiGenSettings', () => {
  // covers: FCT-0006
  it('reads a valid version 1 file into typed settings', () => {
    expect(settingsOf(parseUiGenSettings(kitHorizonFile))).toEqual({
      formatVersion: 1,
      tagPrefix: 'kh-',
      componentsDir: 'src/components',
    });
  });

  // covers: FCT-0006
  it('accepts a multi-segment prefix and the project root as components directory', () => {
    expect(
      settingsOf(parseUiGenSettings({ formatVersion: 1, tagPrefix: 'acme-ui-', componentsDir: '.' })),
    ).toEqual({ formatVersion: 1, tagPrefix: 'acme-ui-', componentsDir: '.' });
  });

  // covers: FCT-0006
  it.each([
    ['null', null],
    ['an array', [{ formatVersion: 1, tagPrefix: 'kh-', componentsDir: 'src/components' }]],
    ['a string', '{"formatVersion":1}'],
    ['a number', 1],
    ['a boolean', true],
    ['undefined', undefined],
  ])('rejects %s as a whole', (_label, value) => {
    expect(errorsOf(parseUiGenSettings(value))).toEqual([{ code: 'not-an-object', field: null }]);
  });

  // covers: FCT-0006
  it('rejects an unknown field and names it', () => {
    expect(
      errorsOf(
        parseUiGenSettings({ formatVersion: 1, tagPrefix: 'kh-', componentsDir: 'src/components', tagprefix: 'kh-' }),
      ),
    ).toEqual([{ code: 'unknown-field', field: 'tagprefix' }]);
  });

  // covers: FCT-0006
  it('names every unknown field', () => {
    const errors = errorsOf(
      parseUiGenSettings({
        formatVersion: 1,
        tagPrefix: 'kh-',
        componentsDir: 'src/components',
        theme: 'dark',
        recentProjects: [],
      }),
    );
    expect(errors).toHaveLength(2);
    expect(errors).toEqual(
      expect.arrayContaining([
        { code: 'unknown-field', field: 'theme' },
        { code: 'unknown-field', field: 'recentProjects' },
      ]),
    );
  });

  // covers: FCT-0006
  it.each([
    ['formatVersion', { tagPrefix: 'kh-', componentsDir: 'src/components' }],
    ['tagPrefix', { formatVersion: 1, componentsDir: 'src/components' }],
    ['componentsDir', { formatVersion: 1, tagPrefix: 'kh-' }],
  ])('rejects a file without %s and names it', (field, value) => {
    expect(errorsOf(parseUiGenSettings(value))).toEqual([{ code: 'missing-field', field }]);
  });

  // covers: FCT-0006
  it('reports every missing field of an empty object', () => {
    const errors = errorsOf(parseUiGenSettings({}));
    expect(errors).toHaveLength(3);
    expect(errors).toEqual(
      expect.arrayContaining([
        { code: 'missing-field', field: 'formatVersion' },
        { code: 'missing-field', field: 'tagPrefix' },
        { code: 'missing-field', field: 'componentsDir' },
      ]),
    );
  });

  // covers: FCT-0006
  it.each([
    ['the string "1"', '1'],
    ['zero', 0],
    ['a negative number', -1],
    ['a fraction', 1.5],
    ['null', null],
    ['NaN', Number.NaN],
  ])('rejects %s as format version', (_label, formatVersion) => {
    expect(
      errorsOf(parseUiGenSettings({ formatVersion, tagPrefix: 'kh-', componentsDir: 'src/components' })),
    ).toEqual([{ code: 'invalid-format-version', field: 'formatVersion' }]);
  });

  // covers: FCT-0006
  it.each([2, 42])('refuses format version %d written by a newer studio', (formatVersion) => {
    expect(
      errorsOf(parseUiGenSettings({ formatVersion, tagPrefix: 'kh-', componentsDir: 'src/components' })),
    ).toEqual([{ code: 'newer-format-version', field: 'formatVersion' }]);
  });

  // covers: FCT-0006
  it.each([
    ['an empty prefix', ''],
    ['a prefix without trailing dash', 'kh'],
    ['an uppercase prefix', 'KH-'],
    ['a prefix starting with a digit', '1k-'],
    ['a prefix starting with a dash', '-kh-'],
    ['a prefix with a double dash', 'kh--'],
    ['a prefix with an inner double dash', 'acme--ui-'],
    ['a prefix with an underscore', 'k_h-'],
    ['a prefix with a space', 'k h-'],
    ['a number', 12],
    ['null', null],
  ])('rejects %s as tag prefix', (_label, tagPrefix) => {
    expect(
      errorsOf(parseUiGenSettings({ formatVersion: 1, tagPrefix, componentsDir: 'src/components' })),
    ).toEqual([{ code: 'invalid-tag-prefix', field: 'tagPrefix' }]);
  });

  // covers: FCT-0006
  it.each([
    ['an empty path', ''],
    ['an absolute path', '/src/components'],
    ['a drive-letter path', 'C:/kit/src/components'],
    ['a backslash path', 'src\\components'],
    ['a path climbing out of the project', '../components'],
    ['a path with an empty segment', 'src//components'],
    ['a path with a dot segment', './src/components'],
    ['an array of segments', ['src', 'components']],
    ['null', null],
  ])('rejects %s as components directory', (_label, componentsDir) => {
    expect(errorsOf(parseUiGenSettings({ formatVersion: 1, tagPrefix: 'kh-', componentsDir }))).toEqual([
      { code: 'invalid-components-dir', field: 'componentsDir' },
    ]);
  });

  // covers: FCT-0006
  it('reports all the errors of a file, not only the first', () => {
    const errors = errorsOf(
      parseUiGenSettings({ formatVersion: '1', tagPrefix: 'KH', componentsDir: '../x', theme: 'dark' }),
    );
    expect(errors).toHaveLength(4);
    expect(errors).toEqual(
      expect.arrayContaining([
        { code: 'invalid-format-version', field: 'formatVersion' },
        { code: 'invalid-tag-prefix', field: 'tagPrefix' },
        { code: 'invalid-components-dir', field: 'componentsDir' },
        { code: 'unknown-field', field: 'theme' },
      ]),
    );
  });

  // covers: FCT-0006
  it('reports a missing field together with an invalid one and an unknown one', () => {
    const errors = errorsOf(parseUiGenSettings({ formatVersion: 1, tagPrefix: 'kh--', extra: true }));
    expect(errors).toHaveLength(3);
    expect(errors).toEqual(
      expect.arrayContaining([
        { code: 'invalid-tag-prefix', field: 'tagPrefix' },
        { code: 'missing-field', field: 'componentsDir' },
        { code: 'unknown-field', field: 'extra' },
      ]),
    );
  });

  // covers: FCT-0006
  it('never reports invalid-json, which belongs to reading the file', () => {
    const results = [null, 'not json {', {}, { formatVersion: 'x', tagPrefix: 0, componentsDir: 0, a: 1 }].map(
      (value) => parseUiGenSettings(value),
    );
    for (const result of results) {
      expect(errorsOf(result).map((error) => error.code)).not.toContain('invalid-json');
    }
  });

  // covers: FCT-0006
  it('leaves the given value untouched', () => {
    const value = { formatVersion: 1, tagPrefix: 'kh-', componentsDir: 'src/components', theme: 'dark' };
    parseUiGenSettings(value);
    expect(value).toEqual({ formatVersion: 1, tagPrefix: 'kh-', componentsDir: 'src/components', theme: 'dark' });
  });
});

describe('serializeUiGenSettings', () => {
  // covers: FCT-0003
  it('writes two-space indented JSON in schema order with a final newline', () => {
    expect(serializeUiGenSettings({ formatVersion: 1, tagPrefix: 'kh-', componentsDir: 'src/components' })).toBe(
      '{\n  "formatVersion": 1,\n  "tagPrefix": "kh-",\n  "componentsDir": "src/components"\n}\n',
    );
  });

  // covers: FCT-0003
  it('keeps the schema order whatever the order of the given object', () => {
    const settings: UiGenSettings = { componentsDir: 'lib/ui', tagPrefix: 'acme-ui-', formatVersion: 1 };
    expect(serializeUiGenSettings(settings)).toBe(
      '{\n  "formatVersion": 1,\n  "tagPrefix": "acme-ui-",\n  "componentsDir": "lib/ui"\n}\n',
    );
  });

  // covers: FCT-0003
  it('writes only the schema fields', () => {
    const withExtra = { formatVersion: 1, tagPrefix: 'kh-', componentsDir: '.', theme: 'dark' } as const;
    expect(serializeUiGenSettings(withExtra)).toBe(
      '{\n  "formatVersion": 1,\n  "tagPrefix": "kh-",\n  "componentsDir": "."\n}\n',
    );
  });

  // covers: FCT-0003
  // covers: FCT-0006
  it('produces a file that opens back into the same settings', () => {
    const settings: UiGenSettings = { formatVersion: 1, tagPrefix: 'kh-', componentsDir: 'src/components' };
    const reread: unknown = JSON.parse(serializeUiGenSettings(settings));
    expect(settingsOf(parseUiGenSettings(reread))).toEqual(settings);
  });
});
