import type { CreateProjectRequest } from '../../modules/project/application/create/create-project.js';
import type { AdoptLibraryRequest } from '../../modules/project/application/open/adopt-library.js';

export type IpcRejectionReason = 'untrusted-sender' | 'invalid-payload' | 'unknown-channel';

export class IpcRejectedError extends Error {
  readonly reason: IpcRejectionReason;

  constructor(reason: IpcRejectionReason) {
    super(`IPC message rejected (${reason})`);
    this.name = 'IpcRejectedError';
    this.reason = reason;
  }
}

export interface PathPayload {
  readonly path: string;
}

const NUL = '\u0000';
const MAX_PATH_LENGTH = 32_767;
export const MAX_NAME_LENGTH = 200;
const MAX_PACKAGE_NAME_LENGTH = 214;
const MAX_TAG_PREFIX_LENGTH = 64;
const MAX_COMPONENTS_DIR_LENGTH = 1_024;
const DRIVE_PATH = /^[A-Za-z]:[\\/]/;
const UNC_SHARE_PATH = /^\\\\[^\\/]+\\[^\\/]+/;
const DEVICE_NAMESPACE_PATH = /^\\\\[.?]\\/;

function isPlainObject(value: unknown): value is Readonly<Record<string, unknown>> {
  if (typeof value !== 'object' || value === null) return false;
  const prototype: unknown = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

export function readExactObject(payload: unknown, keys: readonly string[]): Readonly<Record<string, unknown>> | null {
  if (!isPlainObject(payload)) return null;
  const ownKeys = Reflect.ownKeys(payload);
  const hasExactKeys = ownKeys.length === keys.length && keys.every((key) => Object.hasOwn(payload, key));
  return hasExactKeys ? payload : null;
}

export function readText(value: unknown, maxLength: number): string | null {
  return typeof value === 'string' && value.length <= maxLength && !value.includes(NUL) ? value : null;
}

function isAbsoluteWindowsPath(path: string): boolean {
  return DRIVE_PATH.test(path) || (UNC_SHARE_PATH.test(path) && !DEVICE_NAMESPACE_PATH.test(path));
}

export function readAbsolutePath(value: unknown): string | null {
  const path = readText(value, MAX_PATH_LENGTH);
  return path !== null && isAbsoluteWindowsPath(path) ? path : null;
}

export function readPathPayload(payload: unknown): PathPayload | null {
  const record = readExactObject(payload, ['path']);
  if (record === null) return null;
  const path = readAbsolutePath(record['path']);
  return path === null ? null : { path };
}

export function readCreateProjectPayload(payload: unknown): CreateProjectRequest | null {
  const record = readExactObject(payload, ['name', 'location', 'packageName', 'tagPrefix']);
  if (record === null) return null;
  const name = readText(record['name'], MAX_NAME_LENGTH);
  const location = readAbsolutePath(record['location']);
  const packageName = readText(record['packageName'], MAX_PACKAGE_NAME_LENGTH);
  const tagPrefix = readText(record['tagPrefix'], MAX_TAG_PREFIX_LENGTH);
  if (name === null || location === null || packageName === null || tagPrefix === null) return null;
  return { name, location, packageName, tagPrefix };
}

export function readAdoptLibraryPayload(payload: unknown): AdoptLibraryRequest | null {
  const record = readExactObject(payload, ['path', 'tagPrefix', 'componentsDir']);
  if (record === null) return null;
  const path = readAbsolutePath(record['path']);
  const tagPrefix = readText(record['tagPrefix'], MAX_TAG_PREFIX_LENGTH);
  const componentsDir = readText(record['componentsDir'], MAX_COMPONENTS_DIR_LENGTH);
  if (path === null || tagPrefix === null || componentsDir === null) return null;
  return { path, tagPrefix, componentsDir };
}
