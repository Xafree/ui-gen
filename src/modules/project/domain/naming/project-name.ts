import { toAsciiWords } from './slug.js';

const SCOPED_PACKAGE = /^@([^/]+)\/.+$/;
const NAME_SEPARATORS = /[-_.]+/;

export type ProjectNameError = 'empty' | 'no-alphanumeric';

export function validateProjectName(value: string): ProjectNameError | null {
  if (value.trim() === '') return 'empty';
  return toAsciiWords(value).length === 0 ? 'no-alphanumeric' : null;
}

export function deriveProjectName(packageName: string): string {
  const base = SCOPED_PACKAGE.exec(packageName)?.[1] ?? packageName;
  return base
    .split(NAME_SEPARATORS)
    .filter((word) => word !== '')
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');
}
