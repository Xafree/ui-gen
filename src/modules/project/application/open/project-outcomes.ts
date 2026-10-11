import type { PackageNameError } from '../../domain/naming/package-name.js';
import type { ProjectNameError } from '../../domain/naming/project-name.js';
import type { TagPrefixError } from '../../domain/naming/tag-prefix.js';
import type { SettingsError, UiGenSettings } from '../../domain/settings/uigen-settings.js';
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

export interface AdoptionProposal {
  readonly path: string;
  readonly name: string;
  readonly packageName: string;
  readonly version: string;
  readonly componentCount: number;
  readonly componentsDir: string;
  readonly tagPrefix: string | null;
  readonly exampleTag: string | null;
}

export type NotLitLibraryReason = 'missing-package-json' | 'invalid-package-json' | 'missing-lit-dependency';

export type OpenProjectOutcome =
  | { readonly kind: 'opened'; readonly project: OpenedProject }
  | { readonly kind: 'needs-adoption'; readonly proposal: AdoptionProposal }
  | { readonly kind: 'not-a-lit-library'; readonly path: string; readonly reason: NotLitLibraryReason }
  | { readonly kind: 'invalid-settings'; readonly path: string; readonly errors: readonly SettingsError[] }
  | { readonly kind: 'missing'; readonly path: string }
  | { readonly kind: 'invalid-request'; readonly errors: readonly RequestError[] }
  | { readonly kind: 'write-failed'; readonly path: string; readonly code: string };
