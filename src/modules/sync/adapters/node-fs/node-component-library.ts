import { readdir, readFile, stat } from 'node:fs/promises';
import { watch } from 'node:fs';
import { join } from 'node:path';
import type { ComponentLibrary, StopWatching } from '../../ports/component-library.js';

const PATH_SEPARATOR = /[\\/]/;
const MAX_COMPONENT_FILES = 2000;
const MAX_COMPONENT_BYTES = 1_000_000;
const COMPONENT_SUFFIX = '.ts';
const DECLARATION_SUFFIX = '.d.ts';
const SKIPPED_FOLDERS: ReadonlySet<string> = new Set(['node_modules', 'dist', 'out', 'build', 'coverage']);

export class ComponentFileTooLargeError extends Error {
  constructor(path: string) {
    super(`The component file ${path} is larger than ${String(MAX_COMPONENT_BYTES)} bytes`);
    this.name = 'ComponentFileTooLargeError';
  }
}

function isMissingFolder(error: unknown): boolean {
  return error instanceof Error && 'code' in error && (error.code === 'ENOENT' || error.code === 'ENOTDIR');
}

function isComponentName(name: string): boolean {
  return name.endsWith(COMPONENT_SUFFIX) && !name.endsWith(DECLARATION_SUFFIX);
}

function isInsideSkippedFolder(folder: string, parent: string): boolean {
  return parent
    .slice(folder.length)
    .split(PATH_SEPARATOR)
    .some((segment) => segment.startsWith('.') || SKIPPED_FOLDERS.has(segment));
}

function comparePaths(left: string, right: string): number {
  return left.localeCompare(right, 'en') || (left < right ? -1 : 1);
}

export class NodeComponentLibrary implements ComponentLibrary {
  async listFiles(componentsDir: string): Promise<readonly string[]> {
    let entries;
    try {
      entries = await readdir(componentsDir, { recursive: true, withFileTypes: true });
    } catch (error) {
      if (isMissingFolder(error)) return [];
      throw error;
    }
    return entries
      .filter(({ name, parentPath }) => isComponentName(name) && !isInsideSkippedFolder(componentsDir, parentPath))
      .filter((entry) => entry.isFile())
      .map(({ name, parentPath }) => join(parentPath, name))
      .toSorted(comparePaths)
      .slice(0, MAX_COMPONENT_FILES);
  }

  async readText(path: string): Promise<string> {
    if ((await stat(path)).size > MAX_COMPONENT_BYTES) throw new ComponentFileTooLargeError(path);
    return readFile(path, 'utf8');
  }

  watch(componentsDir: string, onChange: (path: string) => void): StopWatching {
    try {
      const watcher = watch(componentsDir, { recursive: true, persistent: false }, (_event, filename) => {
        if (filename !== null) onChange(join(componentsDir, filename));
      });
      watcher.on('error', (error) => {
        console.warn(`Watching ${componentsDir} stopped`, error);
        watcher.close();
      });
      return () => {
        watcher.close();
      };
    } catch (error) {
      console.warn(`Cannot watch ${componentsDir}`, error);
      return () => undefined;
    }
  }
}
