import { findListedComponent } from './list-components.js';
import type { ComponentDependencies, LibraryLocation, OpenedComponent } from './component-views.js';

export async function openComponent(
  { library, analyzer, lastOpened }: ComponentDependencies,
  { projectPath }: LibraryLocation,
  path: string,
): Promise<OpenedComponent> {
  const text = await library.readText(path);
  const { model, highlight } = await analyzer.analyze(text);
  await lastOpened.save(projectPath, path);
  return { path, text, model, highlight };
}

export async function lastOpenedComponent(
  deps: ComponentDependencies,
  location: LibraryLocation,
): Promise<string | null> {
  const saved = await deps.lastOpened.load(location.projectPath);
  return saved === null ? null : findListedComponent(deps, location, saved);
}
