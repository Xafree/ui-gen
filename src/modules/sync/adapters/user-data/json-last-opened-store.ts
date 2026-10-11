import { randomUUID } from 'node:crypto';
import { mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises';
import { dirname } from 'node:path';
import {
  lastOpenedOf,
  parseLastOpened,
  serializeLastOpened,
  withLastOpened,
  type LastOpenedComponents,
} from '../../domain/library/last-opened.js';
import type { LastOpenedStore } from '../../ports/last-opened-store.js';

function isMissingFile(error: unknown): boolean {
  return error instanceof Error && 'code' in error && error.code === 'ENOENT';
}

export class JsonLastOpenedStore implements LastOpenedStore {
  readonly #file: string;
  #writing: Promise<void> = Promise.resolve();

  constructor(file: string) {
    this.#file = file;
  }

  async load(projectPath: string): Promise<string | null> {
    return lastOpenedOf(await this.#read(), projectPath);
  }

  save(projectPath: string, componentPath: string): Promise<void> {
    this.#writing = this.#writing.then(() => this.#write(projectPath, componentPath));
    return this.#writing;
  }

  async #read(): Promise<LastOpenedComponents> {
    let text: string;
    try {
      text = await readFile(this.#file, 'utf8');
    } catch (error) {
      if (!isMissingFile(error)) console.warn(`Last opened components file ${this.#file} cannot be read`, error);
      return {};
    }
    try {
      const opened = parseLastOpened(JSON.parse(text));
      if (opened === null) console.warn(`Last opened components file ${this.#file} has an unexpected format`);
      return opened ?? {};
    } catch (error) {
      console.warn(`Last opened components file ${this.#file} is not valid JSON`, error);
      return {};
    }
  }

  async #write(projectPath: string, componentPath: string): Promise<void> {
    const temporary = `${this.#file}.${randomUUID()}.tmp`;
    try {
      const opened = withLastOpened(await this.#read(), projectPath, componentPath);
      await mkdir(dirname(this.#file), { recursive: true });
      await writeFile(temporary, serializeLastOpened(opened), { encoding: 'utf8', flag: 'wx' });
      await rename(temporary, this.#file);
    } catch (error) {
      console.warn(`Last opened component not saved in ${this.#file}`, error);
      await rm(temporary, { force: true }).catch(() => undefined);
    }
  }
}
