import { tagPrefixOf } from '../naming/tag-prefix.js';

function countByPrefix(tags: readonly string[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const tag of tags) {
    const prefix = tagPrefixOf(tag);
    if (prefix !== null) counts.set(prefix, (counts.get(prefix) ?? 0) + 1);
  }
  return counts;
}

export function detectMajorityTagPrefix(tags: readonly string[]): string | null {
  const [majority] = [...countByPrefix(tags)].sort(
    ([leftPrefix, leftCount], [rightPrefix, rightCount]) =>
      rightCount - leftCount || leftPrefix.localeCompare(rightPrefix),
  );
  return majority?.[0] ?? null;
}
