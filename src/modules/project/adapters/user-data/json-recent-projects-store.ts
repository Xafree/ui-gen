import { randomUUID } from 'node:crypto';
import { copyFile, mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises';
import { dirname } from 'node:path';
import {
  parseRecentProjects,
  serializeRecentProjects,
  type RecentProject,
} from '../../domain/recents/recent-projects.js';
import type { RecentProjectsLoad, RecentProjectsStore } from '../../ports/recent-projects-store.js';

const BACKUP_SUFFIX = '.bak';

function isMissingFile(error: unknown): boolean {
  return error instanceof Error && 'code' in error && error.code === 'ENOENT';
}

async function removeTemporary(path: string): Promise<void> {
  try {
    await rm(path, { force: true });
  } catch (error) {
    console.warn(`Temporary recent projects file ${path} not removed`, error);
  }
}

export class JsonRecentProjectsStore implements RecentProjectsStore {
  readonly #file: string;

  constructor(file: string) {
    this.#file = file;
  }

  async load(): Promise<RecentProjectsLoad> {
    let text: string;
    try {
      text = await readFile(this.#file, 'utf8');
    } catch (error) {
      if (isMissingFile(error)) return { projects: [], unreadable: false };
      return this.#flagUnreadable('cannot be read', error);
    }
    let value: unknown;
    try {
      value = JSON.parse(text);
    } catch (error) {
      return this.#flagUnreadable('is not valid JSON', error);
    }
    const projects = parseRecentProjects(value);
    return projects === null ? this.#flagUnreadable('has an unexpected format') : { projects, unreadable: false };
  }

  async save(projects: readonly RecentProject[]): Promise<void> {
    const temporary = `${this.#file}.${randomUUID()}.tmp`;
    try {
      await mkdir(dirname(this.#file), { recursive: true });
      await writeFile(temporary, serializeRecentProjects(projects), { encoding: 'utf8', flag: 'wx' });
      await rename(temporary, this.#file);
    } catch (error) {
      console.warn(`Recent projects not saved in ${this.#file}`, error);
      await removeTemporary(temporary);
    }
  }

  async #flagUnreadable(problem: string, ...details: unknown[]): Promise<RecentProjectsLoad> {
    console.warn(`Recent projects file ${this.#file} ${problem}, using an empty list`, ...details);
    await this.#keepBackup();
    return { projects: [], unreadable: true };
  }

  async #keepBackup(): Promise<void> {
    const backup = `${this.#file}${BACKUP_SUFFIX}`;
    try {
      await copyFile(this.#file, backup);
    } catch (error) {
      console.warn(`Recent projects backup ${backup} not written`, error);
    }
  }
}
