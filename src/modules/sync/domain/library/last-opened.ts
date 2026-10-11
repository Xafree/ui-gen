export const LAST_OPENED_FILE = 'last-opened-components.json';

const LAST_OPENED_FORMAT_VERSION = 1;

export type LastOpenedComponents = Readonly<Record<string, string>>;

function projectKey(projectPath: string): string {
  return projectPath.toLowerCase();
}

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isEntries(entries: readonly (readonly [string, unknown])[]): entries is readonly (readonly [string, string])[] {
  return entries.every(([, componentPath]) => typeof componentPath === 'string' && componentPath !== '');
}

export function parseLastOpened(value: unknown): LastOpenedComponents | null {
  if (!isRecord(value) || value['formatVersion'] !== LAST_OPENED_FORMAT_VERSION) return null;
  const projects = value['projects'];
  if (!isRecord(projects)) return null;
  const entries = Object.entries(projects);
  return isEntries(entries) ? Object.fromEntries(entries) : null;
}

export function serializeLastOpened(opened: LastOpenedComponents): string {
  return `${JSON.stringify({ formatVersion: LAST_OPENED_FORMAT_VERSION, projects: opened }, null, 2)}\n`;
}

export function lastOpenedOf(opened: LastOpenedComponents, projectPath: string): string | null {
  return Object.hasOwn(opened, projectKey(projectPath)) ? (opened[projectKey(projectPath)] ?? null) : null;
}

export function withLastOpened(
  opened: LastOpenedComponents,
  projectPath: string,
  componentPath: string,
): LastOpenedComponents {
  return { ...opened, [projectKey(projectPath)]: componentPath };
}
