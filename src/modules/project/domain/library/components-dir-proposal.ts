import { DEFAULT_COMPONENTS_DIR } from '../settings/uigen-settings.js';

export interface ComponentSource {
  readonly relativePath: string;
  readonly tags: readonly string[];
}

function commonLeadingSegments(left: readonly string[], right: readonly string[]): string[] {
  const shared: string[] = [];
  for (const [index, segment] of left.entries()) {
    if (right[index] !== segment) break;
    shared.push(segment);
  }
  return shared;
}

export function proposeComponentsDir(sources: readonly ComponentSource[]): string {
  const folders = sources
    .filter((source) => source.tags.length > 0)
    .map((source) => source.relativePath.split('/').slice(0, -1));
  const [first, ...others] = folders;
  if (first === undefined) return DEFAULT_COMPONENTS_DIR;
  const common = others.reduce(commonLeadingSegments, first);
  return common.length === 0 ? '.' : common.join('/');
}
