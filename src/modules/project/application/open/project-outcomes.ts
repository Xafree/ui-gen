import type { PackageNameError } from '../../domain/naming/package-name.js';
import type { ProjectNameError } from '../../domain/naming/project-name.js';
import type { TagPrefixError } from '../../domain/naming/tag-prefix.js';
import type { UiGenSettings } from '../../domain/settings/uigen-settings.js';
import type { Clock } from '../../ports/clock.js';
import type { ProjectFileSystem } from '../../ports/project-file-system.js';
import type { RecentProjectsStore } from '../../ports/recent-projects-store.js';

export interface ProjectDependencies {
  readonly fileSystem: ProjectFileSystem;
  readonly recentProjects: RecentProjectsStore;
  readonly clock: Clock;
}

export interface OpenedProject {
  readonly path: string;
  readonly name: string;
  readonly packageName: string;
  readonly version: string;
  readonly settings: UiGenSettings;
}

export type RequestField = 'name' | 'location' | 'packageName' | 'tagPrefix' | 'componentsDir' | 'path';

export type RequestErrorCode =
  | ProjectNameError
  | PackageNameError
  | TagPrefixError
  | 'invalid-components-dir'
  | 'not-granted'
  | 'not-pending';

export interface RequestError {
  readonly field: RequestField;
  readonly code: RequestErrorCode;
}
