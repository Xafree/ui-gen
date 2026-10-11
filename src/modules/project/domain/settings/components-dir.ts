const DRIVE_LETTER = /^[A-Za-z]:/;
const PROJECT_ROOT = '.';

function isPlainSegment(segment: string): boolean {
  return segment !== '' && segment !== '.' && segment !== '..';
}

export function isValidComponentsDir(value: string): boolean {
  if (value === PROJECT_ROOT) return true;
  if (value.includes('\\') || DRIVE_LETTER.test(value)) return false;
  return value.split('/').every(isPlainSegment);
}
