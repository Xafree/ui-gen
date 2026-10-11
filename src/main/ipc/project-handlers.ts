import {
  createProject,
  type CreateProjectOutcome,
  type CreateProjectRequest,
} from '../../modules/project/application/create/create-project.js';
import { adoptLibrary, type AdoptLibraryRequest } from '../../modules/project/application/open/adopt-library.js';
import { openProjectFolder } from '../../modules/project/application/open/open-project-folder.js';
import { openRecentProject } from '../../modules/project/application/open/open-recent-project.js';
import type {
  OpenProjectOutcome,
  ProjectDependencies,
} from '../../modules/project/application/open/project-outcomes.js';
import {
  listRecentProjects,
  proposeProjectLocation,
} from '../../modules/project/application/recents/recent-projects-use-cases.js';
import type { FolderPicker } from '../../modules/project/ports/folder-picker.js';
import type { ChooseLocationOutcome, OpenFolderOutcome } from '../../preload/ui-gen-api.js';
import type { ProjectSession } from '../session/project-session.js';
import { IPC_CHANNELS, type IpcChannel } from './channels.js';
import {
  IpcRejectedError,
  readAdoptLibraryPayload,
  readCreateProjectPayload,
  readPathPayload,
  type IpcRejectionReason,
  type PathPayload,
} from './payloads.js';

export interface IpcInvokeEventLike {
  readonly senderFrame: { readonly url: string } | null;
}

export interface IpcRegistry {
  handle(channel: IpcChannel, listener: (event: IpcInvokeEventLike, payload: unknown) => Promise<unknown>): void;
}

export interface ProjectHandlerDependencies {
  readonly project: ProjectDependencies;
  readonly folderPicker: FolderPicker;
  readonly session: ProjectSession;
  readonly isTrustedSender: (senderUrl: string) => boolean;
}

export type PayloadReader<Input> = (payload: unknown) => Input | null;

export type AsyncPayloadReader<Input> = (payload: unknown) => Promise<Input | null>;

export type GuardedHandle = <Input>(
  channel: IpcChannel,
  read: PayloadReader<Input> | AsyncPayloadReader<Input>,
  useCase: (input: Input) => Promise<unknown>,
) => void;

const UNEXPECTED_FAILURE_MESSAGE = 'The request failed unexpectedly';

const LOCATION_NOT_GRANTED: CreateProjectOutcome = {
  kind: 'invalid-request',
  errors: [{ field: 'location', code: 'not-granted' }],
};

const ADOPTION_NOT_PENDING: OpenProjectOutcome = {
  kind: 'invalid-request',
  errors: [{ field: 'path', code: 'not-pending' }],
};

export function withoutPayload(): undefined {
  return undefined;
}

function refuse(channel: IpcChannel, reason: IpcRejectionReason, senderUrl: string | null): IpcRejectedError {
  console.warn(`IPC ${channel} refused (${reason}), sender ${senderUrl ?? 'has no frame'}`);
  return new IpcRejectedError(reason);
}

async function runUseCase<Result>(channel: IpcChannel, useCase: () => Promise<Result>): Promise<Result> {
  try {
    return await useCase();
  } catch (cause) {
    console.error(`IPC ${channel} failed`, cause);
    throw new Error(UNEXPECTED_FAILURE_MESSAGE);
  }
}

export function guardedHandle(registry: IpcRegistry, isTrustedSender: (senderUrl: string) => boolean): GuardedHandle {
  return (channel, read, useCase) => {
    registry.handle(channel, async (event, payload) => {
      const senderUrl = event.senderFrame?.url ?? null;
      if (senderUrl === null || !isTrustedSender(senderUrl)) throw refuse(channel, 'untrusted-sender', senderUrl);
      const input = await runUseCase(channel, async () => read(payload));
      if (input === null) throw refuse(channel, 'invalid-payload', senderUrl);
      return runUseCase(channel, () => useCase(input));
    });
  };
}

function recordOpening(session: ProjectSession, outcome: OpenProjectOutcome): OpenProjectOutcome {
  switch (outcome.kind) {
    case 'opened':
      session.openProject(outcome.project);
      break;
    case 'needs-adoption':
      session.setPendingAdoption(outcome.proposal.path);
      break;
    default:
      session.setPendingAdoption(null);
  }
  return outcome;
}

async function openRecent(
  { project, session }: ProjectHandlerDependencies,
  { path }: PathPayload,
): Promise<OpenProjectOutcome> {
  return recordOpening(session, await openRecentProject(project, path));
}

async function proposeLocation({ project, session }: ProjectHandlerDependencies): Promise<string> {
  const location = await proposeProjectLocation(project, session.defaultLocation);
  session.grantLocation(location);
  return location;
}

async function chooseLocation({ folderPicker, session }: ProjectHandlerDependencies): Promise<ChooseLocationOutcome> {
  const path = await folderPicker.pickFolder('choose-location');
  if (path === null) return { canceled: true };
  session.grantLocation(path);
  return { canceled: false, path };
}

async function create(
  { project, session }: ProjectHandlerDependencies,
  request: CreateProjectRequest,
): Promise<CreateProjectOutcome> {
  if (!session.isLocationGranted(request.location)) return LOCATION_NOT_GRANTED;
  const outcome = await createProject(project, request);
  if (outcome.kind === 'created') session.openProject(outcome.project);
  return outcome;
}

async function openFolder({ project, folderPicker, session }: ProjectHandlerDependencies): Promise<OpenFolderOutcome> {
  const path = await folderPicker.pickFolder('open-project');
  if (path === null) return { kind: 'canceled' };
  return recordOpening(session, await openProjectFolder(project, path));
}

async function adopt(
  { project, session }: ProjectHandlerDependencies,
  request: AdoptLibraryRequest,
): Promise<OpenProjectOutcome> {
  if (!session.isPendingAdoption(request.path)) return ADOPTION_NOT_PENDING;
  const outcome = await adoptLibrary(project, request);
  if (outcome.kind === 'opened') session.openProject(outcome.project);
  return outcome;
}

function closeProject(session: ProjectSession): Promise<void> {
  session.closeProject();
  session.setPendingAdoption(null);
  return Promise.resolve();
}

export function registerProjectHandlers(registry: IpcRegistry, deps: ProjectHandlerDependencies): void {
  const handle = guardedHandle(registry, deps.isTrustedSender);
  handle(IPC_CHANNELS.recentsList, withoutPayload, () => listRecentProjects(deps.project));
  handle(IPC_CHANNELS.recentsOpen, readPathPayload, (payload) => openRecent(deps, payload));
  handle(IPC_CHANNELS.projectDefaultLocation, withoutPayload, () => proposeLocation(deps));
  handle(IPC_CHANNELS.projectChooseLocation, withoutPayload, () => chooseLocation(deps));
  handle(IPC_CHANNELS.projectCreate, readCreateProjectPayload, (request) => create(deps, request));
  handle(IPC_CHANNELS.projectOpenFolder, withoutPayload, () => openFolder(deps));
  handle(IPC_CHANNELS.projectAdopt, readAdoptLibraryPayload, (request) => adopt(deps, request));
  handle(IPC_CHANNELS.projectClose, withoutPayload, () => closeProject(deps.session));
}
