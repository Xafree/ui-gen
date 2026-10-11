import { expect } from 'vitest';
import type { IpcChannel } from '../../../../src/main/ipc/channels';
import {
  registerProjectHandlers,
  type IpcInvokeEventLike,
  type IpcRegistry,
} from '../../../../src/main/ipc/project-handlers';
import { ProjectSession } from '../../../../src/main/session/project-session';
import type { RecentProject } from '../../../../src/modules/project/domain/recents/recent-projects';
import type { FolderPicker, FolderPurpose } from '../../../../src/modules/project/ports/folder-picker';
import type {
  FolderState,
  JsonFileRead,
  ProjectEntry,
  SourceFile,
} from '../../../../src/modules/project/ports/project-file-system';
import type { RecentProjectsLoad } from '../../../../src/modules/project/ports/recent-projects-store';
import { InMemoryProjectFileSystem } from '../../../modules/project/application/doubles/in-memory-project-file-system';
import { InMemoryRecentProjectsStore } from '../../../modules/project/application/doubles/in-memory-recent-projects-store';
import { NOW } from '../../../modules/project/application/open/lit-library-fixtures';

export const TRUSTED_URL = 'file:///C:/Program%20Files/UI-GEN/resources/app.asar/out/renderer/index.html';
export const FOREIGN_URL = 'https://studio-update.example.net/phishing/index.html';
export const DOCUMENTS = 'C:\\Users\\Camille\\Documents';

type Listener = (event: IpcInvokeEventLike, payload: unknown) => Promise<unknown>;

export class FakeIpcRegistry implements IpcRegistry {
  readonly #listeners = new Map<IpcChannel, Listener>();

  get channels(): IpcChannel[] {
    return [...this.#listeners.keys()];
  }

  handle(channel: IpcChannel, listener: Listener): void {
    if (this.#listeners.has(channel)) throw new Error(`Channel ${channel} registered twice`);
    this.#listeners.set(channel, listener);
  }

  invoke(channel: IpcChannel, payload?: unknown): Promise<unknown> {
    return this.invokeFrom({ url: TRUSTED_URL }, channel, payload);
  }

  invokeFrom(senderFrame: IpcInvokeEventLike['senderFrame'], channel: IpcChannel, payload?: unknown): Promise<unknown> {
    const listener = this.#listeners.get(channel);
    if (listener === undefined) throw new Error(`No handler registered for ${channel}`);
    return listener({ senderFrame }, payload);
  }
}

export class ScriptedFolderPicker implements FolderPicker {
  readonly purposes: FolderPurpose[] = [];
  readonly #answers: (string | null)[] = [];

  answer(path: string | null): this {
    this.#answers.push(path);
    return this;
  }

  pickFolder(purpose: FolderPurpose): Promise<string | null> {
    this.purposes.push(purpose);
    const next = this.#answers.shift();
    if (next === undefined) return Promise.reject(new Error(`Unexpected folder dialog (${purpose})`));
    return Promise.resolve(next);
  }
}

export class RecordingFileSystem extends InMemoryProjectFileSystem {
  readonly touchedPaths: string[] = [];

  override readFolderState(path: string): Promise<FolderState> {
    this.touchedPaths.push(path);
    return super.readFolderState(path);
  }

  override readJsonFile(folder: string, relativePath: string): Promise<JsonFileRead> {
    this.touchedPaths.push(folder);
    return super.readJsonFile(folder, relativePath);
  }

  override listSourceFiles(folder: string): Promise<SourceFile[]> {
    this.touchedPaths.push(folder);
    return super.listSourceFiles(folder);
  }

  override createFolderAtomically(target: string, entries: readonly ProjectEntry[]): Promise<void> {
    this.touchedPaths.push(target);
    return super.createFolderAtomically(target, entries);
  }

  override writeFileAtomically(folder: string, relativePath: string, content: string): Promise<void> {
    this.touchedPaths.push(folder);
    return super.writeFileAtomically(folder, relativePath, content);
  }
}

export class RecordingRecentsStore extends InMemoryRecentProjectsStore {
  loadCount = 0;

  override load(): Promise<RecentProjectsLoad> {
    this.loadCount += 1;
    return super.load();
  }
}

export interface HandlerHarness {
  readonly registry: FakeIpcRegistry;
  readonly fileSystem: RecordingFileSystem;
  readonly recents: RecordingRecentsStore;
  readonly picker: ScriptedFolderPicker;
  readonly session: ProjectSession;
  readonly checkedSenders: string[];
  readonly clockReads: { count: number };
}

export interface HarnessOptions {
  readonly fileSystem?: RecordingFileSystem;
  readonly recents?: RecordingRecentsStore | readonly RecentProject[];
  readonly isTrustedSender?: (senderUrl: string) => boolean;
}

function storeFrom(recents: HarnessOptions['recents']): RecordingRecentsStore {
  if (recents instanceof RecordingRecentsStore) return recents;
  return new RecordingRecentsStore(recents ?? []);
}

export function setUpHandlers(options: HarnessOptions = {}): HandlerHarness {
  const registry = new FakeIpcRegistry();
  const fileSystem = options.fileSystem ?? new RecordingFileSystem();
  const recents = storeFrom(options.recents);
  const picker = new ScriptedFolderPicker();
  const session = new ProjectSession(DOCUMENTS);
  const checkedSenders: string[] = [];
  const clockReads = { count: 0 };
  const trust = options.isTrustedSender ?? ((senderUrl: string) => senderUrl === TRUSTED_URL);
  registerProjectHandlers(registry, {
    project: {
      fileSystem,
      recentProjects: recents,
      clock: () => {
        clockReads.count += 1;
        return new Date(NOW);
      },
    },
    folderPicker: picker,
    session,
    isTrustedSender: (senderUrl: string) => {
      checkedSenders.push(senderUrl);
      return trust(senderUrl);
    },
  });
  return { registry, fileSystem, recents, picker, session, checkedSenders, clockReads };
}

export function expectNothingTouched(harness: HandlerHarness): void {
  expect({
    fileSystem: harness.fileSystem.touchedPaths,
    recentLoads: harness.recents.loadCount,
    recentSaves: harness.recents.saves.length,
    dialogs: harness.picker.purposes,
    clockReads: harness.clockReads.count,
  }).toEqual({ fileSystem: [], recentLoads: 0, recentSaves: 0, dialogs: [], clockReads: 0 });
}

export async function rejectionOf(call: () => Promise<unknown>): Promise<unknown> {
  try {
    await call();
  } catch (error) {
    return error;
  }
  throw new Error('Expected the IPC call to reject');
}
