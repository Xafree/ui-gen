import { contextBridge, ipcRenderer } from 'electron';
import { IPC_CHANNELS, PUSH_CHANNELS, type IpcChannel, type PushChannel } from '../main/ipc/channels.js';
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
import type { RecentProject } from '../modules/project/domain/recents/recent-projects.js';
import type { ComponentChange, OpenedComponent } from '../modules/sync/application/components/component-views.js';
import type { ComponentSummary } from '../modules/sync/domain/library/component-summary.js';
import type { AnalyzerStatus } from '../modules/sync/ports/component-analyzer.js';
import type { ChooseLocationOutcome, OpenFolderOutcome, UiGenApi } from './ui-gen-api.js';

function invoke(channel: IpcChannel, ...payload: readonly unknown[]): Promise<unknown> {
  return ipcRenderer.invoke(channel, ...payload);
}

function subscribe(channel: PushChannel, listener: (message: unknown) => void): () => void {
  const forward = (_event: unknown, message: unknown): void => {
    listener(message);
  };
  ipcRenderer.on(channel, forward);
  return () => {
    ipcRenderer.removeListener(channel, forward);
  };
}

const api: UiGenApi = Object.freeze({
  platform: process.platform,
  recents: Object.freeze({
    list: () => invoke(IPC_CHANNELS.recentsList) as Promise<RecentProjectsListing>,
    remove: (path: string) => invoke(IPC_CHANNELS.recentsRemove, { path }) as Promise<RecentProjectView[]>,
    clear: () => invoke(IPC_CHANNELS.recentsClear) as Promise<RecentProjectView[]>,
    restore: (projects: readonly RecentProject[]) =>
      invoke(IPC_CHANNELS.recentsRestore, { projects }) as Promise<RecentProjectView[]>,
    open: (path: string) => invoke(IPC_CHANNELS.recentsOpen, { path }) as Promise<OpenProjectOutcome>,
  }),
  project: Object.freeze({
    defaultLocation: () => invoke(IPC_CHANNELS.projectDefaultLocation) as Promise<string>,
    chooseLocation: () => invoke(IPC_CHANNELS.projectChooseLocation) as Promise<ChooseLocationOutcome>,
    create: (request: CreateProjectRequest) =>
      invoke(IPC_CHANNELS.projectCreate, request) as Promise<CreateProjectOutcome>,
    openFolder: () => invoke(IPC_CHANNELS.projectOpenFolder) as Promise<OpenFolderOutcome>,
    adopt: (request: AdoptLibraryRequest) => invoke(IPC_CHANNELS.projectAdopt, request) as Promise<OpenProjectOutcome>,
    close: () => invoke(IPC_CHANNELS.projectClose) as Promise<void>,
  }),
  components: Object.freeze({
    list: () => invoke(IPC_CHANNELS.componentList) as Promise<ComponentSummary[]>,
    open: (path: string) => invoke(IPC_CHANNELS.componentOpen, { path }) as Promise<OpenedComponent>,
    lastOpened: () => invoke(IPC_CHANNELS.componentLastOpened) as Promise<string | null>,
    onChanged: (listener: (change: ComponentChange) => void) =>
      subscribe(PUSH_CHANNELS.componentChanged, (message) => {
        listener(message as ComponentChange);
      }),
    onStatus: (listener: (status: AnalyzerStatus) => void) =>
      subscribe(PUSH_CHANNELS.componentStatus, (message) => {
        listener(message as AnalyzerStatus);
      }),
  }),
});

contextBridge.exposeInMainWorld('uiGen', api);
