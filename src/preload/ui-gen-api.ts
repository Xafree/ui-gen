import type {
  CreateProjectOutcome,
  CreateProjectRequest,
} from '../modules/project/application/create/create-project.js';
import type { AdoptLibraryRequest } from '../modules/project/application/open/adopt-library.js';
import type { OpenProjectOutcome } from '../modules/project/application/open/project-outcomes.js';
import type {
  RecentProjectsListing,
  RecentProjectView,
} from '../modules/project/application/recents/recent-projects-use-cases.js';
import type { ComponentChange, OpenedComponent } from '../modules/sync/application/components/component-views.js';
import type { ComponentSummary } from '../modules/sync/domain/library/component-summary.js';
import type { AnalyzerStatus } from '../modules/sync/ports/component-analyzer.js';
import type { RecentProject } from '../modules/project/domain/recents/recent-projects.js';

export type ChooseLocationOutcome =
  | { readonly canceled: true }
  | { readonly canceled: false; readonly path: string };

export type OpenFolderOutcome = OpenProjectOutcome | { readonly kind: 'canceled' };

export interface UiGenApi {
  readonly platform: string;
  readonly recents: {
    list(): Promise<RecentProjectsListing>;
    remove(path: string): Promise<RecentProjectView[]>;
    clear(): Promise<RecentProjectView[]>;
    restore(projects: readonly RecentProject[]): Promise<RecentProjectView[]>;
    open(path: string): Promise<OpenProjectOutcome>;
  };
  readonly project: {
    defaultLocation(): Promise<string>;
    chooseLocation(): Promise<ChooseLocationOutcome>;
    create(request: CreateProjectRequest): Promise<CreateProjectOutcome>;
    openFolder(): Promise<OpenFolderOutcome>;
    adopt(request: AdoptLibraryRequest): Promise<OpenProjectOutcome>;
    close(): Promise<void>;
  };
  readonly components: {
    list(): Promise<ComponentSummary[]>;
    open(path: string): Promise<OpenedComponent>;
    lastOpened(): Promise<string | null>;
    onChanged(listener: (change: ComponentChange) => void): () => void;
    onStatus(listener: (status: AnalyzerStatus) => void): () => void;
  };
}

declare global {
  interface Window {
    readonly uiGen: UiGenApi;
  }
}
