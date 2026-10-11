import { describe, expect, it } from 'vitest';
import {
  readAdoptLibraryPayload,
  readCreateProjectPayload,
  readPathPayload,
} from '../../../src/main/ipc/payloads';

const createPayload = {
  name: 'Kit Horizon',
  location: String.raw`E:\projets`,
  packageName: '@kit-horizon/ui',
  tagPrefix: 'kh-',
};

const adoptPayload = {
  path: String.raw`E:\projets\legacy-kit`,
  tagPrefix: 'lk-',
  componentsDir: 'src/components',
};

const notAbsoluteWindowsPaths: readonly [string, string][] = [
  ['an extended-length drive path', String.raw`\\?\C:\projets\kit-horizon`],
  ['an extended-length UNC path', String.raw`\\?\UNC\serveur\partage\kit-horizon`],
  ['a device path', String.raw`\\.\C:\projets`],
  ['a named pipe path', String.raw`\\.\pipe\ui-gen`],
  ['a UNC path without a share', String.raw`\\serveur`],
  ['a UNC path with an empty share', '\\\\serveur\\'],
  ['a UNC path without a server', String.raw`\\\partage`],
  ['a UNC path written with slashes', '//serveur/partage/kit-horizon'],
];

describe('absolute path rule', () => {
  // covers: FCT-0003, FCT-0007
  it.each(notAbsoluteWindowsPaths)('refuses %s in every request that carries a path', (_label, path) => {
    expect(readPathPayload({ path })).toBeNull();
    expect(readCreateProjectPayload({ ...createPayload, location: path })).toBeNull();
    expect(readAdoptLibraryPayload({ ...adoptPayload, path })).toBeNull();
  });
});

describe('plain object rule', () => {
  it('accepts a dictionary that has no prototype and returns an ordinary object', () => {
    const received: unknown = Object.setPrototypeOf({ path: String.raw`E:\projets` }, null);

    expect(readPathPayload(received)).toStrictEqual({ path: String.raw`E:\projets` });
  });

  it('returns a new object, not the received one', () => {
    const received = { path: String.raw`E:\projets` };

    const read = readPathPayload(received);

    expect(read).toEqual(received);
    expect(read).not.toBe(received);
  });

  it('rejects a key that is hidden from enumeration', () => {
    const received = { path: String.raw`E:\projets` };
    Object.defineProperty(received, 'hidden', { value: true, enumerable: false });

    expect(readPathPayload(received)).toBeNull();
  });

  it('rejects a symbol key', () => {
    expect(readPathPayload({ path: String.raw`E:\projets`, [Symbol('extra')]: true })).toBeNull();
  });

  it('returns the value it validated even when the field changes between two reads', () => {
    let reads = 0;
    const received = {
      get path(): string {
        reads += 1;
        return reads === 1 ? String.raw`E:\projets` : 'not an absolute path';
      },
    };

    expect(readPathPayload(received)).toEqual({ path: String.raw`E:\projets` });
  });
});
