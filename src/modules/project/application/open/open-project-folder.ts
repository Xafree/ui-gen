import { proposeComponentsDir } from '../../domain/library/components-dir-proposal.js';
import { findDeclaredTags } from '../../domain/library/declared-tags.js';
import { PACKAGE_MANIFEST_FILE, readLibraryManifest, type LibraryManifest } from '../../domain/library/library-manifest.js';
import { detectMajorityTagPrefix } from '../../domain/library/majority-prefix.js';
import { deriveProjectName } from '../../domain/naming/project-name.js';
import { tagPrefixOf } from '../../domain/naming/tag-prefix.js';
import { findRecentProject } from '../../domain/recents/recent-projects.js';
import { parseUiGenSettings, UIGEN_SETTINGS_FILE } from '../../domain/settings/uigen-settings.js';
import { rememberProject } from '../recents/remember-project.js';
import type {
  AdoptionProposal,
  NotLitLibraryReason,
  OpenProjectOutcome,
  ProjectDependencies,
} from './project-outcomes.js';

const WINDOWS_SEPARATOR = '\\';

function notLitLibrary(path: string, reason: NotLitLibraryReason): OpenProjectOutcome {
  return { kind: 'not-a-lit-library', path, reason };
}

async function displayName(deps: ProjectDependencies, path: string, packageName: string): Promise<string> {
  const { projects } = await deps.recentProjects.load();
  return findRecentProject(projects, path)?.name ?? deriveProjectName(packageName);
}

async function proposeAdoption(
  deps: ProjectDependencies,
  path: string,
  name: string,
  manifest: LibraryManifest,
): Promise<AdoptionProposal> {
  const files = await deps.fileSystem.listSourceFiles(path);
  const sources = files.map((file) => ({
    relativePath: file.relativePath.replaceAll(WINDOWS_SEPARATOR, '/'),
    tags: findDeclaredTags(file.content),
  }));
  const tags = sources.flatMap((source) => source.tags);
  const tagPrefix = detectMajorityTagPrefix(tags);
  return {
    path,
    name,
    packageName: manifest.name,
    version: manifest.version,
    componentCount: tags.length,
    componentsDir: proposeComponentsDir(sources),
    tagPrefix,
    exampleTag: tags.find((tag) => tagPrefixOf(tag) === tagPrefix) ?? null,
  };
}

export async function openProjectFolder(deps: ProjectDependencies, path: string): Promise<OpenProjectOutcome> {
  const folder = await deps.fileSystem.readFolderState(path);
  if (folder.kind === 'missing' || folder.kind === 'not-a-folder') return { kind: 'missing', path };
  const manifestFile = await deps.fileSystem.readJsonFile(path, PACKAGE_MANIFEST_FILE);
  if (manifestFile.kind !== 'ok') {
    return notLitLibrary(path, manifestFile.kind === 'missing' ? 'missing-package-json' : 'invalid-package-json');
  }
  const manifest = readLibraryManifest(manifestFile.value);
  if (manifest === null) return notLitLibrary(path, 'invalid-package-json');
  if (!manifest.dependsOnLit) return notLitLibrary(path, 'missing-lit-dependency');
  const name = await displayName(deps, path, manifest.name);
  const settingsFile = await deps.fileSystem.readJsonFile(path, UIGEN_SETTINGS_FILE);
  if (settingsFile.kind === 'missing') {
    return { kind: 'needs-adoption', proposal: await proposeAdoption(deps, path, name, manifest) };
  }
  if (settingsFile.kind === 'invalid-json') {
    return { kind: 'invalid-settings', path, errors: [{ code: 'invalid-json', field: null }] };
  }
  const parsed = parseUiGenSettings(settingsFile.value);
  if (!parsed.ok) return { kind: 'invalid-settings', path, errors: parsed.errors };
  await rememberProject(deps, { path, name });
  return {
    kind: 'opened',
    project: { path, name, packageName: manifest.name, version: manifest.version, settings: parsed.settings },
  };
}
