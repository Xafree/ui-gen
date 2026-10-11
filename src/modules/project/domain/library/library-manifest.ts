import { isRecord } from '../json-value.js';

export const PACKAGE_MANIFEST_FILE = 'package.json';
export const NEW_PROJECT_VERSION = '0.1.0';
export const LIT_VERSION_RANGE = '^3.3.3';

export interface LibraryManifest {
  readonly name: string;
  readonly version: string;
  readonly dependsOnLit: boolean;
}

function declaresLit(section: unknown): boolean {
  return isRecord(section) && typeof section['lit'] === 'string';
}

export function readLibraryManifest(value: unknown): LibraryManifest | null {
  if (!isRecord(value)) return null;
  const name = value['name'];
  const version = value['version'];
  if (typeof name !== 'string' || name === '') return null;
  return {
    name,
    version: typeof version === 'string' ? version : '',
    dependsOnLit: declaresLit(value['dependencies']) || declaresLit(value['peerDependencies']),
  };
}

export function createLibraryManifest(packageName: string): string {
  const manifest = {
    name: packageName,
    version: NEW_PROJECT_VERSION,
    type: 'module',
    dependencies: { lit: LIT_VERSION_RANGE },
  };
  return `${JSON.stringify(manifest, null, 2)}\n`;
}
