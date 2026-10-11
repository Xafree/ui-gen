import { toAsciiWords } from './slug.js';

export const TAG_PREFIX_PATTERN = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*-$/;

export type TagPrefixError =
  | 'empty'
  | 'uppercase'
  | 'not-starting-with-letter'
  | 'invalid-characters'
  | 'missing-trailing-dash'
  | 'double-dash';

export function suggestTagPrefix(projectName: string): string {
  const words = toAsciiWords(projectName).filter((word) => /^[a-z]/.test(word));
  const [first] = words;
  if (first === undefined) return '';
  const initials = words.length === 1 ? first.slice(0, 2) : words.map((word) => word.charAt(0)).join('');
  return `${initials}-`;
}

export function validateTagPrefix(value: string): TagPrefixError | null {
  if (value === '') return 'empty';
  if (/[A-Z]/.test(value)) return 'uppercase';
  if (!/^[a-z]/.test(value)) return 'not-starting-with-letter';
  if (!/^[a-z0-9-]+$/.test(value)) return 'invalid-characters';
  if (!value.endsWith('-')) return 'missing-trailing-dash';
  if (value.includes('--')) return 'double-dash';
  return null;
}

export function tagPrefixOf(tag: string): string | null {
  const dash = tag.indexOf('-');
  return dash <= 0 ? null : tag.slice(0, dash + 1);
}
