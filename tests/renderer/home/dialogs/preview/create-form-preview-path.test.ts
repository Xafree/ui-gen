import { describe, expect, it } from 'vitest';
import { deriveCreateForm } from '../../../../../src/renderer/home/dialogs/create-form.js';

function previewPathIn(location: string): string | null {
  return deriveCreateForm({
    name: 'Kit Horizon',
    location,
    packageName: '',
    tagPrefix: '',
    packageEdited: false,
    prefixEdited: false,
    nameTouched: false,
  }).previewPath;
}

describe('deriveCreateForm preview path separators', () => {
  it.each([
    ['one trailing backslash', 'E:\\projets\\', 'E:\\projets\\kit-horizon'],
    ['one trailing slash', 'E:/projets/', 'E:/projets\\kit-horizon'],
    ['trailing separators of both kinds', 'E:\\projets\\/\\', 'E:\\projets\\kit-horizon'],
    ['a drive root', 'E:\\', 'E:\\kit-horizon'],
    ['an empty location', '', '\\kit-horizon'],
    ['a location made of separators only', '\\//', '\\kit-horizon'],
  ])('puts a single backslash before the project folder for %s', (_label, location, expected) => {
    expect(previewPathIn(location)).toBe(expected);
  });

  it('keeps the separators inside the location', () => {
    expect(previewPathIn('E:\\\\projets//clients\\')).toBe('E:\\\\projets//clients\\kit-horizon');
  });
});
