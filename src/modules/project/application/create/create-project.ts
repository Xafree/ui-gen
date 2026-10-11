import { createLibraryManifest, NEW_PROJECT_VERSION, PACKAGE_MANIFEST_FILE } from '../../domain/library/library-manifest.js';
import { projectFolderName, validatePackageName } from '../../domain/naming/package-name.js';
import { validateProjectName } from '../../domain/naming/project-name.js';
import { validateTagPrefix } from '../../domain/naming/tag-prefix.js';
import {
  CURRENT_FORMAT_VERSION,
  DEFAULT_COMPONENTS_DIR,
  serializeUiGenSettings,
  UIGEN_SETTINGS_FILE,
  type UiGenSettings,
} from '../../domain/settings/uigen-settings.js';
import { ProjectWriteError } from '../../ports/project-file-system.js';
import type { OpenedProject, ProjectDependencies, RequestError } from '../open/project-outcomes.js';
import { rememberProject } from '../recents/remember-project.js';

export interface CreateProjectRequest {
  readonly name: string;
  readonly location: string;
  readonly packageName: string;
  readonly tagPrefix: string;
}

export type CreateProjectOutcome =
  | { readonly kind: 'created'; readonly project: OpenedProject }
  | { readonly kind: 'invalid-request'; readonly errors: readonly RequestError[] }
  | { readonly kind: 'folder-not-empty'; readonly path: string; readonly entryCount: number }
  | { readonly kind: 'write-failed'; readonly path: string; readonly code: string };

function validateRequest(request: CreateProjectRequest): RequestError[] {
  const errors: RequestError[] = [];
  const nameError = validateProjectName(request.name);
  if (nameError !== null) errors.push({ field: 'name', code: nameError });
  if (request.location.trim() === '') errors.push({ field: 'location', code: 'empty' });
  const packageError = validatePackageName(request.packageName);
  if (packageError !== null) errors.push({ field: 'packageName', code: packageError });
  const prefixError = validateTagPrefix(request.tagPrefix);
  if (prefixError !== null) errors.push({ field: 'tagPrefix', code: prefixError });
  return errors;
}

export async function createProject(
  deps: ProjectDependencies,
  request: CreateProjectRequest,
): Promise<CreateProjectOutcome> {
  const errors = validateRequest(request);
  if (errors.length > 0) return { kind: 'invalid-request', errors };
  const path = deps.fileSystem.childPath(request.location, projectFolderName(request.packageName));
  const settings: UiGenSettings = {
    formatVersion: CURRENT_FORMAT_VERSION,
    tagPrefix: request.tagPrefix,
    componentsDir: DEFAULT_COMPONENTS_DIR,
  };
  try {
    const state = await deps.fileSystem.readFolderState(path);
    if (state.kind === 'not-empty') return { kind: 'folder-not-empty', path, entryCount: state.entryCount };
    if (state.kind === 'not-a-folder') return { kind: 'write-failed', path, code: 'EEXIST' };
    await deps.fileSystem.createFolderAtomically(path, [
      { kind: 'file', relativePath: PACKAGE_MANIFEST_FILE, content: createLibraryManifest(request.packageName) },
      { kind: 'file', relativePath: UIGEN_SETTINGS_FILE, content: serializeUiGenSettings(settings) },
      { kind: 'folder', relativePath: DEFAULT_COMPONENTS_DIR },
    ]);
  } catch (error) {
    return { kind: 'write-failed', path, code: error instanceof ProjectWriteError ? error.code : 'UNKNOWN' };
  }
  const name = request.name.trim();
  await rememberProject(deps, { path, name });
  return {
    kind: 'created',
    project: { path, name, packageName: request.packageName, version: NEW_PROJECT_VERSION, settings },
  };
}
