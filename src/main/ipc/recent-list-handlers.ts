import type { ProjectDependencies } from '../../modules/project/application/open/project-outcomes.js';
import {
  clearRecentProjects,
  restoreRecentProjects,
} from '../../modules/project/application/recents/forget-recent-projects.js';
import {
  removeRecentProjectEntry,
  type RecentProjectView,
} from '../../modules/project/application/recents/recent-projects-use-cases.js';
import {
  MAX_RECENT_PROJECTS,
  samePath,
  type RecentProject,
} from '../../modules/project/domain/recents/recent-projects.js';
import type { ForgottenRecents } from '../session/forgotten-recents.js';
import { IPC_CHANNELS } from './channels.js';
import {
  MAX_NAME_LENGTH,
  readAbsolutePath,
  readExactObject,
  readPathPayload,
  readText,
  type PathPayload,
} from './payloads.js';
import { guardedHandle, withoutPayload, type IpcRegistry, type PayloadReader } from './project-handlers.js';

export interface RecentListHandlerDependencies {
  readonly project: ProjectDependencies;
  readonly forgotten: ForgottenRecents;
  readonly isTrustedSender: (senderUrl: string) => boolean;
}

export interface RestorePayload {
  readonly projects: RecentProject[];
}

const ISO_DATE_TIME = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(?::\d{2}(?:\.\d{1,9})?)?(?:Z|[+-]\d{2}:\d{2})?$/;

function readIsoDate(value: unknown): string | null {
  const isIsoDate = typeof value === 'string' && ISO_DATE_TIME.test(value) && !Number.isNaN(Date.parse(value));
  return isIsoDate ? value : null;
}

function readRecentProject(value: unknown): RecentProject | null {
  const record = readExactObject(value, ['path', 'name', 'openedAt']);
  if (record === null) return null;
  const path = readAbsolutePath(record['path']);
  const name = readText(record['name'], MAX_NAME_LENGTH);
  const openedAt = readIsoDate(record['openedAt']);
  return path === null || name === null || openedAt === null ? null : { path, name, openedAt };
}

function readRecentProjects(items: readonly unknown[]): RecentProject[] | null {
  const projects: RecentProject[] = [];
  for (const item of items) {
    const project = readRecentProject(item);
    if (project === null) return null;
    projects.push(project);
  }
  return projects;
}

export function readRestorePayload(payload: unknown): RestorePayload | null {
  const items = readExactObject(payload, ['projects'])?.['projects'];
  if (!Array.isArray(items) || items.length > MAX_RECENT_PROJECTS) return null;
  const projects = readRecentProjects(items);
  return projects === null ? null : { projects };
}

function readForgottenProjects(forgotten: ForgottenRecents): PayloadReader<RestorePayload> {
  return (payload) => {
    const request = readRestorePayload(payload);
    if (request === null) return null;
    return request.projects.every((project) => forgotten.isForgotten(project)) ? request : null;
  };
}

async function remove(
  { project, forgotten }: RecentListHandlerDependencies,
  { path }: PathPayload,
): Promise<RecentProjectView[]> {
  const { projects } = await project.recentProjects.load();
  const remaining = await removeRecentProjectEntry(project, path);
  forgotten.remember(projects.filter((entry) => samePath(entry.path, path)));
  return remaining;
}

async function clear({ project, forgotten }: RecentListHandlerDependencies): Promise<RecentProjectView[]> {
  const { projects } = await project.recentProjects.load();
  const remaining = await clearRecentProjects(project);
  forgotten.remember(projects);
  return remaining;
}

export function registerRecentListHandlers(registry: IpcRegistry, deps: RecentListHandlerDependencies): void {
  const handle = guardedHandle(registry, deps.isTrustedSender);
  handle(IPC_CHANNELS.recentsRemove, readPathPayload, (payload) => remove(deps, payload));
  handle(IPC_CHANNELS.recentsClear, withoutPayload, () => clear(deps));
  handle(IPC_CHANNELS.recentsRestore, readForgottenProjects(deps.forgotten), ({ projects }) =>
    restoreRecentProjects(deps.project, projects),
  );
}
