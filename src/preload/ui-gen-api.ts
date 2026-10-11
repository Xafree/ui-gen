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

export type ChooseLocationOutcome =
  | { readonly canceled: true }
  | { readonly canceled: false; readonly path: string };

export type OpenFolderOutcome = OpenProjectOutcome | { readonly kind: 'canceled' };

export interface UiGenApi {
  readonly platform: string;
  readonly recents: {
    list(): Promise<RecentProjectsListing>;
    remove(path: string): Promise<RecentProjectView[]>;
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
}

declare global {
  interface Window {
    readonly uiGen: UiGenApi;
  }
}
