import { describe, expect, it } from 'vitest';
import {
  IpcRejectedError,
  readAdoptLibraryPayload,
  readCreateProjectPayload,
  readPathPayload,
} from '../../../src/main/ipc/payloads';

const MAX_PATH_LENGTH = 32_767;

const createPayload = {
  name: 'Kit Horizon',
  location: 'E:\\projets',
  packageName: '@kit-horizon/ui',
  tagPrefix: 'kh-',
};

const adoptPayload = {
  path: 'E:\\projets\\legacy-kit',
  tagPrefix: 'lk-',
  componentsDir: 'src/components',
};

function absolutePathOfLength(length: number): string {
  return `E:\\${'a'.repeat(length - 3)}`;
}

function withoutKey(payload: Readonly<Record<string, string>>, key: string): Record<string, string> {
  return Object.fromEntries(Object.entries(payload).filter(([name]) => name !== key));
}

class PathHolder {
  readonly path = 'E:\\projets\\kit-horizon';
}

const notPlainObjects: readonly [string, unknown][] = [
  ['null', null],
  ['undefined', undefined],
  ['a string', 'E:\\projets\\kit-horizon'],
  ['a number', 42],
  ['a boolean', true],
  ['an array', ['E:\\projets\\kit-horizon']],
  ['a function', () => 'E:\\projets\\kit-horizon'],
];

describe('IpcRejectedError', () => {
  it.each(['untrusted-sender', 'invalid-payload', 'unknown-channel'] as const)(
    'is an Error that keeps the %s reason',
    (reason) => {
      const error = new IpcRejectedError(reason);

      expect(error).toBeInstanceOf(Error);
      expect(error).toBeInstanceOf(IpcRejectedError);
      expect(error.reason).toBe(reason);
    },
  );
});

describe('readPathPayload', () => {
  it.each([
    ['a drive path with backslashes', 'E:\\projets\\kit-horizon'],
    ['a drive path with slashes', 'C:/Users/emma/Documents'],
    ['a drive root', 'D:\\'],
    ['a lowercase drive letter', 'e:\\projets\\kit-horizon'],
    ['a UNC path', '\\\\serveur\\partage\\kit-horizon'],
    ['a UNC share root', '\\\\serveur\\partage'],
  ])('accepts %s', (_label, path) => {
    expect(readPathPayload({ path })).toEqual({ path });
  });

  it('accepts a path of exactly 32 767 characters', () => {
    const path = absolutePathOfLength(MAX_PATH_LENGTH);

    expect(readPathPayload({ path })).toEqual({ path });
  });

  it('rejects a path longer than 32 767 characters', () => {
    expect(readPathPayload({ path: absolutePathOfLength(MAX_PATH_LENGTH + 1) })).toBeNull();
  });

  it.each(notPlainObjects)('rejects %s', (_label, payload) => {
    expect(readPathPayload(payload)).toBeNull();
  });

  it('rejects an object without the path key', () => {
    expect(readPathPayload({})).toBeNull();
  });

  it('rejects an object with an extra key', () => {
    expect(readPathPayload({ path: 'E:\\projets\\kit-horizon', force: true })).toBeNull();
  });

  it('rejects an own __proto__ key smuggled through JSON', () => {
    const payload: unknown = JSON.parse('{"path":"E:\\\\projets\\\\kit-horizon","__proto__":{"admin":true}}');

    expect(readPathPayload(payload)).toBeNull();
  });

  it('rejects a constructor key', () => {
    expect(readPathPayload({ path: 'E:\\projets\\kit-horizon', constructor: 'x' })).toBeNull();
  });

  it('rejects a path that is only inherited from the prototype', () => {
    const payload: unknown = Object.create({ path: 'E:\\projets\\kit-horizon' });

    expect(readPathPayload(payload)).toBeNull();
  });

  it('rejects a class instance', () => {
    expect(readPathPayload(new PathHolder())).toBeNull();
  });

  it.each([
    ['a number', 42],
    ['null', null],
    ['an array', ['E:\\projets']],
    ['an object', { value: 'E:\\projets' }],
    ['a boolean', false],
  ])('rejects a path that is %s', (_label, path) => {
    expect(readPathPayload({ path })).toBeNull();
  });

  it.each([
    ['an empty path', ''],
    ['a relative path', 'projets\\kit-horizon'],
    ['a dot-relative path', '.\\kit-horizon'],
    ['a parent-relative path', '..\\..\\Windows'],
    ['a drive-relative path', 'E:projets\\kit-horizon'],
    ['a bare drive letter', 'E:'],
    ['a POSIX path', '/home/emma/kit-horizon'],
    ['a root-relative Windows path', '\\projets\\kit-horizon'],
    ['a file URL', 'file:///E:/projets/kit-horizon'],
    ['a path with a NUL character', 'E:\\projets\\kit\u0000horizon'],
    ['a path ending with a NUL character', 'E:\\projets\\kit-horizon\u0000'],
  ])('rejects %s', (_label, path) => {
    expect(readPathPayload({ path })).toBeNull();
  });
});

// covers: FCT-0003
describe('readCreateProjectPayload', () => {
  // covers: FCT-0003
  it('accepts a complete creation request and returns its four fields', () => {
    expect(readCreateProjectPayload({ ...createPayload })).toEqual(createPayload);
  });

  // covers: FCT-0003
  it('accepts a location on a UNC share', () => {
    const payload = { ...createPayload, location: '\\\\serveur\\partage\\projets' };

    expect(readCreateProjectPayload(payload)).toEqual(payload);
  });

  // covers: FCT-0003
  it('leaves the business rules of name, package and prefix to the use case', () => {
    const payload = { ...createPayload, name: '   ', packageName: 'Not A Package', tagPrefix: 'KH' };

    expect(readCreateProjectPayload(payload)).toEqual(payload);
  });

  // covers: FCT-0003
  it.each([
    ['name', 200],
    ['packageName', 214],
    ['tagPrefix', 64],
  ] as const)('accepts a %s of exactly %i characters', (field, length) => {
    const payload = { ...createPayload, [field]: 'a'.repeat(length) };

    expect(readCreateProjectPayload(payload)).toEqual(payload);
  });

  // covers: FCT-0003
  it.each([
    ['name', 201],
    ['packageName', 215],
    ['tagPrefix', 65],
  ] as const)('rejects a %s longer than its limit (%i characters)', (field, length) => {
    expect(readCreateProjectPayload({ ...createPayload, [field]: 'a'.repeat(length) })).toBeNull();
  });

  // covers: FCT-0003
  it('accepts a location of exactly 32 767 characters and rejects a longer one', () => {
    const atLimit = { ...createPayload, location: absolutePathOfLength(MAX_PATH_LENGTH) };
    const overLimit = { ...createPayload, location: absolutePathOfLength(MAX_PATH_LENGTH + 1) };

    expect(readCreateProjectPayload(atLimit)).toEqual(atLimit);
    expect(readCreateProjectPayload(overLimit)).toBeNull();
  });

  // covers: FCT-0003
  it.each(notPlainObjects)('rejects %s', (_label, payload) => {
    expect(readCreateProjectPayload(payload)).toBeNull();
  });

  // covers: FCT-0003
  it.each(['name', 'location', 'packageName', 'tagPrefix'] as const)('rejects a request without %s', (field) => {
    expect(readCreateProjectPayload(withoutKey(createPayload, field))).toBeNull();
  });

  // covers: FCT-0003
  it('rejects a request with an extra key', () => {
    expect(readCreateProjectPayload({ ...createPayload, overwrite: true })).toBeNull();
  });

  // covers: FCT-0003
  it('rejects an own __proto__ key smuggled through JSON', () => {
    const payload: unknown = JSON.parse(
      '{"name":"Kit Horizon","location":"E:\\\\projets","packageName":"@kit-horizon/ui","tagPrefix":"kh-","__proto__":{"admin":true}}',
    );

    expect(readCreateProjectPayload(payload)).toBeNull();
  });

  // covers: FCT-0003
  it.each(['name', 'location', 'packageName', 'tagPrefix'] as const)('rejects a %s that is not a string', (field) => {
    expect(readCreateProjectPayload({ ...createPayload, [field]: 7 })).toBeNull();
    expect(readCreateProjectPayload({ ...createPayload, [field]: null })).toBeNull();
    expect(readCreateProjectPayload({ ...createPayload, [field]: ['kh-'] })).toBeNull();
  });

  // covers: FCT-0003
  it.each(['name', 'location', 'packageName', 'tagPrefix'] as const)('rejects a %s with a NUL character', (field) => {
    expect(readCreateProjectPayload({ ...createPayload, [field]: `${createPayload[field]}\u0000` })).toBeNull();
  });

  // covers: FCT-0003
  it.each([
    ['empty', ''],
    ['relative', 'projets'],
    ['drive-relative', 'E:projets'],
    ['POSIX', '/home/emma/projets'],
  ])('rejects a location that is %s', (_label, location) => {
    expect(readCreateProjectPayload({ ...createPayload, location })).toBeNull();
  });
});

// covers: FCT-0007
describe('readAdoptLibraryPayload', () => {
  // covers: FCT-0007
  it('accepts a complete adoption request and returns its three fields', () => {
    expect(readAdoptLibraryPayload({ ...adoptPayload })).toEqual(adoptPayload);
  });

  // covers: FCT-0007
  it('leaves the business rules of prefix and components folder to the use case', () => {
    const payload = { ...adoptPayload, tagPrefix: 'Not-Valid', componentsDir: '../outside' };

    expect(readAdoptLibraryPayload(payload)).toEqual(payload);
  });

  // covers: FCT-0007
  it('accepts a tagPrefix of 64 characters and rejects one of 65', () => {
    const atLimit = { ...adoptPayload, tagPrefix: 'a'.repeat(64) };

    expect(readAdoptLibraryPayload(atLimit)).toEqual(atLimit);
    expect(readAdoptLibraryPayload({ ...adoptPayload, tagPrefix: 'a'.repeat(65) })).toBeNull();
  });

  // covers: FCT-0007
  it('accepts a componentsDir of 1 024 characters and rejects one of 1 025', () => {
    const atLimit = { ...adoptPayload, componentsDir: 'a'.repeat(1024) };

    expect(readAdoptLibraryPayload(atLimit)).toEqual(atLimit);
    expect(readAdoptLibraryPayload({ ...adoptPayload, componentsDir: 'a'.repeat(1025) })).toBeNull();
  });

  // covers: FCT-0007
  it('rejects a path longer than 32 767 characters', () => {
    expect(readAdoptLibraryPayload({ ...adoptPayload, path: absolutePathOfLength(MAX_PATH_LENGTH + 1) })).toBeNull();
  });

  // covers: FCT-0007
  it.each(notPlainObjects)('rejects %s', (_label, payload) => {
    expect(readAdoptLibraryPayload(payload)).toBeNull();
  });

  // covers: FCT-0007
  it.each(['path', 'tagPrefix', 'componentsDir'] as const)('rejects a request without %s', (field) => {
    expect(readAdoptLibraryPayload(withoutKey(adoptPayload, field))).toBeNull();
  });

  // covers: FCT-0007
  it('rejects a request with an extra key', () => {
    expect(readAdoptLibraryPayload({ ...adoptPayload, name: 'Legacy Kit' })).toBeNull();
  });

  // covers: FCT-0007
  it('rejects an own __proto__ key smuggled through JSON', () => {
    const payload: unknown = JSON.parse(
      '{"path":"E:\\\\projets\\\\legacy-kit","tagPrefix":"lk-","componentsDir":"src/components","__proto__":{"admin":true}}',
    );

    expect(readAdoptLibraryPayload(payload)).toBeNull();
  });

  // covers: FCT-0007
  it.each(['path', 'tagPrefix', 'componentsDir'] as const)('rejects a %s that is not a string', (field) => {
    expect(readAdoptLibraryPayload({ ...adoptPayload, [field]: 7 })).toBeNull();
    expect(readAdoptLibraryPayload({ ...adoptPayload, [field]: null })).toBeNull();
    expect(readAdoptLibraryPayload({ ...adoptPayload, [field]: { value: 'x' } })).toBeNull();
  });

  // covers: FCT-0007
  it.each(['path', 'tagPrefix', 'componentsDir'] as const)('rejects a %s with a NUL character', (field) => {
    expect(readAdoptLibraryPayload({ ...adoptPayload, [field]: `${adoptPayload[field]}\u0000` })).toBeNull();
  });

  // covers: FCT-0007
  it.each([
    ['empty', ''],
    ['relative', 'projets\\legacy-kit'],
    ['drive-relative', 'E:legacy-kit'],
    ['POSIX', '/home/emma/legacy-kit'],
  ])('rejects a path that is %s', (_label, path) => {
    expect(readAdoptLibraryPayload({ ...adoptPayload, path })).toBeNull();
  });
});
