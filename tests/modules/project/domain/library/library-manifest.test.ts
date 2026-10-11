import { describe, expect, it } from 'vitest';
import {
  LIT_VERSION_RANGE,
  NEW_PROJECT_VERSION,
  PACKAGE_MANIFEST_FILE,
  createLibraryManifest,
  readLibraryManifest,
} from '../../../../../src/modules/project/domain/library/library-manifest';

describe('library manifest constants', () => {
  // covers: FCT-0009
  it('names the npm manifest file and the versions of a new project', () => {
    expect(PACKAGE_MANIFEST_FILE).toBe('package.json');
    expect(NEW_PROJECT_VERSION).toBe('0.1.0');
    expect(LIT_VERSION_RANGE).toBe('^3.3.3');
  });
});

describe('readLibraryManifest', () => {
  // covers: FCT-0009
  it('reads a Lit library declaring lit in dependencies', () => {
    const manifest: unknown = {
      name: '@acme/kit',
      version: '1.2.3',
      type: 'module',
      dependencies: { lit: '^3.3.3' },
    };
    expect(readLibraryManifest(manifest)).toEqual({ name: '@acme/kit', version: '1.2.3', dependsOnLit: true });
  });

  // covers: FCT-0009
  it('reads a Lit library declaring lit in peerDependencies', () => {
    const manifest: unknown = {
      name: 'acme-kit',
      version: '0.0.1',
      peerDependencies: { lit: '>=3' },
      devDependencies: { vitest: '^3.0.0' },
    };
    expect(readLibraryManifest(manifest)).toEqual({ name: 'acme-kit', version: '0.0.1', dependsOnLit: true });
  });

  // covers: FCT-0009
  it('reports a package that does not depend on lit', () => {
    const manifest: unknown = { name: 'react-kit', version: '2.0.0', dependencies: { react: '^19.0.0' } };
    expect(readLibraryManifest(manifest)).toEqual({ name: 'react-kit', version: '2.0.0', dependsOnLit: false });
  });

  // covers: FCT-0009
  it('does not count lit declared only in devDependencies', () => {
    const manifest: unknown = { name: 'demo', version: '1.0.0', devDependencies: { lit: '^3.3.3' } };
    expect(readLibraryManifest(manifest)?.dependsOnLit).toBe(false);
  });

  // covers: FCT-0009
  it('reports no lit dependency when dependencies are missing', () => {
    expect(readLibraryManifest({ name: 'bare', version: '1.0.0' })).toEqual({
      name: 'bare',
      version: '1.0.0',
      dependsOnLit: false,
    });
  });

  // covers: FCT-0009
  it.each([
    ['a number', { name: 'kit', dependencies: { lit: 3 } }],
    ['null', { name: 'kit', dependencies: { lit: null } }],
    ['an object', { name: 'kit', peerDependencies: { lit: { version: '3' } } }],
  ])('does not count a lit entry that is %s', (_label, manifest) => {
    expect(readLibraryManifest(manifest)?.dependsOnLit).toBe(false);
  });

  // covers: FCT-0009
  it.each([
    ['a string', { name: 'kit', dependencies: 'lit' }],
    ['null', { name: 'kit', dependencies: null }],
    ['a number', { name: 'kit', peerDependencies: 42 }],
  ])('does not count dependencies that are %s', (_label, manifest) => {
    expect(readLibraryManifest(manifest)?.dependsOnLit).toBe(false);
  });

  // covers: FCT-0009
  it('uses an empty version when the version is missing', () => {
    expect(readLibraryManifest({ name: 'kit', dependencies: { lit: '^3.0.0' } })).toEqual({
      name: 'kit',
      version: '',
      dependsOnLit: true,
    });
  });

  // covers: FCT-0009
  it.each([
    ['null', null],
    ['undefined', undefined],
    ['a string', '{"name":"kit"}'],
    ['a number', 42],
    ['a boolean', true],
  ])('returns null for a manifest that is %s', (_label, value) => {
    expect(readLibraryManifest(value)).toBeNull();
  });

  // covers: FCT-0009
  it.each([
    ['missing', { version: '1.0.0', dependencies: { lit: '^3.3.3' } }],
    ['empty', { name: '', version: '1.0.0', dependencies: { lit: '^3.3.3' } }],
    ['a number', { name: 12, version: '1.0.0', dependencies: { lit: '^3.3.3' } }],
    ['null', { name: null, version: '1.0.0', dependencies: { lit: '^3.3.3' } }],
  ])('returns null when the name is %s', (_label, value) => {
    expect(readLibraryManifest(value)).toBeNull();
  });
});

describe('createLibraryManifest', () => {
  // covers: FCT-0009
  it('writes the package.json of a new Lit project, two-space indented with a final newline', () => {
    const expected = [
      '{',
      '  "name": "@acme/kit",',
      '  "version": "0.1.0",',
      '  "type": "module",',
      '  "dependencies": {',
      '    "lit": "^3.3.3"',
      '  }',
      '}',
      '',
    ].join('\n');
    expect(createLibraryManifest('@acme/kit')).toBe(expected);
  });

  // covers: FCT-0009
  it('produces a manifest that reads back as a Lit library', () => {
    const parsed: unknown = JSON.parse(createLibraryManifest('my-kit'));
    expect(readLibraryManifest(parsed)).toEqual({ name: 'my-kit', version: '0.1.0', dependsOnLit: true });
  });
});
