import { validateTagPrefix } from '../../domain/naming/tag-prefix.js';
import { isValidComponentsDir } from '../../domain/settings/components-dir.js';
import {
  CURRENT_FORMAT_VERSION,
  serializeUiGenSettings,
  UIGEN_SETTINGS_FILE,
  type UiGenSettings,
} from '../../domain/settings/uigen-settings.js';
import { ProjectWriteError } from '../../ports/project-file-system.js';
import { rememberProject } from '../recents/remember-project.js';
import { openProjectFolder } from './open-project-folder.js';
import type { OpenProjectOutcome, ProjectDependencies, RequestError } from './project-outcomes.js';

export interface AdoptLibraryRequest {
  readonly path: string;
  readonly tagPrefix: string;
  readonly componentsDir: string;
}

function validateRequest(request: AdoptLibraryRequest): RequestError[] {
  const errors: RequestError[] = [];
  const prefixError = validateTagPrefix(request.tagPrefix);
  if (prefixError !== null) errors.push({ field: 'tagPrefix', code: prefixError });
  if (!isValidComponentsDir(request.componentsDir)) {
    errors.push({ field: 'componentsDir', code: 'invalid-components-dir' });
  }
  return errors;
}

export async function adoptLibrary(
  deps: ProjectDependencies,
  request: AdoptLibraryRequest,
): Promise<OpenProjectOutcome> {
  const errors = validateRequest(request);
  if (errors.length > 0) return { kind: 'invalid-request', errors };
  const current = await openProjectFolder(deps, request.path);
  if (current.kind !== 'needs-adoption') return current;
  const { proposal } = current;
  const settings: UiGenSettings = {
    formatVersion: CURRENT_FORMAT_VERSION,
    tagPrefix: request.tagPrefix,
    componentsDir: request.componentsDir,
  };
  try {
    await deps.fileSystem.writeFileAtomically(proposal.path, UIGEN_SETTINGS_FILE, serializeUiGenSettings(settings));
  } catch (error) {
    return {
      kind: 'write-failed',
      path: proposal.path,
      code: error instanceof ProjectWriteError ? error.code : 'UNKNOWN',
    };
  }
  await rememberProject(deps, { path: proposal.path, name: proposal.name });
  return {
    kind: 'opened',
    project: {
      path: proposal.path,
      name: proposal.name,
      packageName: proposal.packageName,
      version: proposal.version,
      settings,
    },
  };
}
