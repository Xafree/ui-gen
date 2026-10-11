import { isRecord } from '../json-value.js';

export const MAX_RECENT_PROJECTS = 10;
export const RECENT_PROJECTS_FILE = 'recent-projects.json';

const RECENT_PROJECTS_FORMAT_VERSION = 1;
const DRIVE_ROOT_SEPARATOR_INDEX = 2;

export interface RecentProject {
  readonly path: string;
  readonly name: string;
  readonly openedAt: string;
}

export function samePath(left: string, right: string): boolean {
  return left.toLowerCase() === right.toLowerCase();
}

function readEntry(value: unknown): RecentProject | null {
  if (!isRecord(value)) return null;
  const { path, name, openedAt } = value;
  if (typeof path !== 'string' || path === '' || typeof name !== 'string' || typeof openedAt !== 'string') return null;
  return Number.isNaN(Date.parse(openedAt)) ? null : { path, name, openedAt };
}

function readEntries(items: readonly unknown[]): RecentProject[] | null {
  const entries: RecentProject[] = [];
  for (const item of items) {
    const entry = readEntry(item);
    if (entry === null) return null;
    entries.push(entry);
  }
  return entries;
}

function byMostRecent(left: RecentProject, right: RecentProject): number {
  return Date.parse(right.openedAt) - Date.parse(left.openedAt);
}

function keepMostRecent(projects: readonly RecentProject[]): RecentProject[] {
  return projects.toSorted(byMostRecent).slice(0, MAX_RECENT_PROJECTS);
}

export function parseRecentProjects(value: unknown): RecentProject[] | null {
  if (!isRecord(value)) return null;
  const { formatVersion, projects } = value;
  if (formatVersion !== RECENT_PROJECTS_FORMAT_VERSION || !Array.isArray(projects)) return null;
  const entries = readEntries(projects);
  return entries === null ? null : keepMostRecent(entries);
}

export function serializeRecentProjects(projects: readonly RecentProject[]): string {
  const entries = projects.map(({ path, name, openedAt }) => ({ path, name, openedAt }));
  return `${JSON.stringify({ formatVersion: RECENT_PROJECTS_FORMAT_VERSION, projects: entries }, null, 2)}\n`;
}

export function removeRecentProject(projects: readonly RecentProject[], path: string): RecentProject[] {
  return projects.filter((project) => !samePath(project.path, path));
}

export function recordRecentProject(projects: readonly RecentProject[], entry: RecentProject): RecentProject[] {
  return keepMostRecent([entry, ...removeRecentProject(projects, entry.path)]);
}

export function findRecentProject(projects: readonly RecentProject[], path: string): RecentProject | undefined {
  return projects.find((project) => samePath(project.path, path));
}

export function parentFolderOf(path: string): string | null {
  const lastSeparator = Math.max(path.lastIndexOf('\\'), path.lastIndexOf('/'));
  return lastSeparator > DRIVE_ROOT_SEPARATOR_INDEX ? path.slice(0, lastSeparator) : null;
}
