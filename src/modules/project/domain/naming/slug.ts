const COMBINING_MARKS = /[\u0300-\u036f]/g;
const WORD_SEPARATORS = /[^a-z0-9]+/;

export function toAsciiWords(value: string): string[] {
  return value
    .normalize('NFD')
    .replaceAll(COMBINING_MARKS, '')
    .toLowerCase()
    .split(WORD_SEPARATORS)
    .filter((word) => word !== '');
}

export function toSlug(value: string): string {
  return toAsciiWords(value).join('-');
}
