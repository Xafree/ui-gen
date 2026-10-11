export interface ComponentSummary {
  readonly path: string;
  readonly tag: string;
  readonly name: string;
}

function capitalize(word: string): string {
  return word.charAt(0).toUpperCase() + word.slice(1);
}

export function componentDisplayName(tag: string, tagPrefix: string): string {
  const hasPrefix = tagPrefix !== '' && tag.startsWith(tagPrefix) && tag.length > tagPrefix.length;
  const words = (hasPrefix ? tag.slice(tagPrefix.length) : tag).split('-').filter((word) => word !== '');
  return words.length === 0 ? tag : words.map(capitalize).join(' ');
}

export function compareSummaries(left: ComponentSummary, right: ComponentSummary): number {
  if (left.tag !== right.tag) return left.tag < right.tag ? -1 : 1;
  if (left.path === right.path) return 0;
  return left.path < right.path ? -1 : 1;
}

export function samePath(left: string, right: string): boolean {
  return left.toLowerCase() === right.toLowerCase();
}
