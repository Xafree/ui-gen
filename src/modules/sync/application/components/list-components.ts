import { compareSummaries, componentDisplayName, samePath, type ComponentSummary } from '../../domain/library/component-summary.js';
import type { ComponentAnalyzer } from '../../ports/component-analyzer.js';
import type { ComponentLibrary } from '../../ports/component-library.js';
import type { ComponentDependencies, LibraryLocation } from './component-views.js';

type LibraryReading = Pick<ComponentDependencies, 'library' | 'analyzer'>;

async function readTag(library: ComponentLibrary, analyzer: ComponentAnalyzer, path: string): Promise<string | null> {
  try {
    const { tag } = await analyzer.readModel(await library.readText(path));
    return tag === '' ? null : tag;
  } catch (cause) {
    console.warn('A file of the components folder was skipped', cause);
    return null;
  }
}

export async function listComponents(
  { library, analyzer }: LibraryReading,
  { componentsDir, tagPrefix }: LibraryLocation,
): Promise<ComponentSummary[]> {
  const summaries: ComponentSummary[] = [];
  for (const path of await library.listFiles(componentsDir)) {
    const tag = await readTag(library, analyzer, path);
    if (tag !== null) summaries.push({ path, tag, name: componentDisplayName(tag, tagPrefix) });
  }
  return summaries.toSorted(compareSummaries);
}

export async function findListedComponent(
  { library, analyzer }: LibraryReading,
  { componentsDir }: LibraryLocation,
  path: string,
): Promise<string | null> {
  const listed = (await library.listFiles(componentsDir)).find((file) => samePath(file, path));
  if (listed === undefined) return null;
  return (await readTag(library, analyzer, listed)) === null ? null : listed;
}
