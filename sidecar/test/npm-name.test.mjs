import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { validatePackageName, validateVersion } from '../npm-name.mjs';

const refused = (result) => {
  assert.equal(result.ok, false);
  assert.equal(typeof result.reason, 'string');
  assert.notEqual(result.reason.trim(), '');
};

describe('validatePackageName', () => {
  // covers: E2-1.4
  it('accepts registry package names', () => {
    for (const name of ['react', '@mui/material', 'lit', 'a'.repeat(214)]) {
      assert.deepEqual(validatePackageName(name), { ok: true }, name);
    }
  });

  // covers: E2-1.4
  it('refuses anything that is not a registry package name', () => {
    const names = [
      '../x',
      'file:../x',
      'git+https://example.com/x.git',
      'github:user/repo',
      'npm:react@1',
      'x.tgz',
      '-x',
      'Mui',
      'a b',
      'a\nb',
      '',
      'a'.repeat(215),
    ];
    for (const name of names) {
      refused(validatePackageName(name));
    }
  });

  // covers: E2-1.4
  it('refuses a value that is not a string', () => {
    for (const value of [undefined, null, 42, {}]) {
      refused(validatePackageName(value));
    }
  });
});

describe('validateVersion', () => {
  // covers: E2-1.4
  it('accepts exact semver versions and latest', () => {
    for (const version of ['7.3.11', '1.0.0-beta.2+build.5', 'latest', `1.0.0-${'a'.repeat(58)}`]) {
      assert.deepEqual(validateVersion(version, { allowLatest: true }), { ok: true }, version);
    }
  });

  // covers: E2-1.4
  it('refuses ranges, tags other than latest, partial versions and long strings', () => {
    const versions = ['^7.0.0', '~1.2.3', '>=1', '*', '1.x', 'next', '1.0', '1.0.0 || 2.0.0', '', `1.0.0-${'a'.repeat(59)}`];
    for (const version of versions) {
      refused(validateVersion(version, { allowLatest: true }));
    }
  });

  // covers: E2-1.4
  it('refuses latest when it is not allowed, and still accepts an exact version', () => {
    refused(validateVersion('latest', { allowLatest: false }));
    assert.deepEqual(validateVersion('7.3.11', { allowLatest: false }), { ok: true });
  });

  // covers: E2-1.4
  it('refuses a value that is not a string', () => {
    for (const value of [undefined, null, 7, {}]) {
      refused(validateVersion(value, { allowLatest: true }));
    }
  });
});
