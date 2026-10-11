import { randomUUID } from 'node:crypto';
import { lstat, mkdir, readdir, readFile, rename, rm, rmdir, stat, writeFile } from 'node:fs/promises';
import { basename, dirname, isAbsolute, join, relative, sep } from 'node:path';
import {
  ProjectWriteError,
  type FolderState,
  type JsonFileRead,
  type ProjectEntry,
  type ProjectFileSystem,
  type SourceFile,
} from '../../ports/project-file-system.js';

const SKIPPED_FOLDERS: ReadonlySet<string> = new Set(['node_modules', 'dist', 'out', 'build', 'coverage']);
const SOURCE_FILE = /\.(?:ts|mts|js|mjs)$/;
const DECLARATION_SUFFIX = '.d.ts';
const MAX_SOURCE_BYTES = 1_000_000;
const BYTE_ORDER_MARK = '\uFEFF';
const LEFTOVER_REMOVAL = { recursive: true, force: true, maxRetries: 3 } as const;

function errorCode(error: unknown): string {
  return error instanceof Error && 'code' in error && typeof error.code === 'string' ? error.code : 'UNKNOWN';
}

function writeFailure(path: string, error: unknown): ProjectWriteError {
  console.error(`Project file system failure on ${path}`, error);
  return new ProjectWriteError(path, errorCode(error), { cause: error });
}

function isInside(folder: string, path: string): boolean {
  const fromFolder = relative(folder, path);
  return fromFolder !== '' && !isAbsolute(fromFolder) && fromFolder.split(sep)[0] !== '..';
}

function toNativePath(folder: string, relativePath: string): string {
  const path = join(folder, ...relativePath.split('/'));
  if (!isInside(folder, path)) throw new ProjectWriteError(path, 'EINVAL');
  return path;
}

function withoutByteOrderMark(text: string): string {
  return text.startsWith(BYTE_ORDER_MARK) ? text.slice(BYTE_ORDER_MARK.length) : text;
}

function parseJson(text: string): JsonFileRead {
  try {
    const value: unknown = JSON.parse(withoutByteOrderMark(text));
    return { kind: 'ok', value };
  } catch {
    return { kind: 'invalid-json' };
  }
}

function isTraversable(folderName: string): boolean {
  return !folderName.startsWith('.') && !SKIPPED_FOLDERS.has(folderName);
}

function isSourceName(fileName: string): boolean {
  return SOURCE_FILE.test(fileName) && !fileName.endsWith(DECLARATION_SUFFIX);
}

async function collectSourcePaths(root: string, folder: string, found: string[]): Promise<void> {
  for (const entry of await readdir(folder, { withFileTypes: true })) {
    const path = join(folder, entry.name);
    if (entry.isDirectory()) {
      if (isTraversable(entry.name)) await collectSourcePaths(root, path, found);
    } else if (entry.isFile() && isSourceName(entry.name) && (await stat(path)).size <= MAX_SOURCE_BYTES) {
      found.push(relative(root, path).replaceAll(sep, '/'));
    }
  }
}

async function lstatIfPresent(path: string) {
  try {
    return await lstat(path);
  } catch (error) {
    if (errorCode(error) === 'ENOENT') return null;
    throw error;
  }
}

async function freeTarget(target: string): Promise<void> {
  const info = await lstatIfPresent(target);
  if (info === null) return;
  if (!info.isDirectory()) throw new ProjectWriteError(target, 'EEXIST');
  await rmdir(target);
}

async function removeLeftover(path: string): Promise<void> {
  try {
    await rm(path, LEFTOVER_REMOVAL);
  } catch (error) {
    console.error(`Cannot remove the leftover ${path}`, error);
  }
}

async function writeEntry(entry: ProjectEntry, path: string): Promise<void> {
  if (entry.kind === 'folder') {
    await mkdir(path, { recursive: true });
    return;
  }
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, entry.content, { encoding: 'utf8', flag: 'wx' });
}

export class NodeProjectFileSystem implements ProjectFileSystem {
  childPath(parent: string, name: string): string {
    return join(parent, name);
  }

  async readFolderState(path: string): Promise<FolderState> {
    try {
      if (!(await stat(path)).isDirectory()) return { kind: 'not-a-folder' };
      const entries = await readdir(path);
      return entries.length === 0 ? { kind: 'empty' } : { kind: 'not-empty', entryCount: entries.length };
    } catch (error) {
      const code = errorCode(error);
      if (code === 'ENOENT') return { kind: 'missing' };
      if (code === 'ENOTDIR') return { kind: 'not-a-folder' };
      throw writeFailure(path, error);
    }
  }

  async readJsonFile(folder: string, relativePath: string): Promise<JsonFileRead> {
    const path = toNativePath(folder, relativePath);
    try {
      return parseJson(await readFile(path, 'utf8'));
    } catch (error) {
      if (errorCode(error) === 'ENOENT') return { kind: 'missing' };
      console.error(`Cannot read ${path}`, error);
      return { kind: 'invalid-json' };
    }
  }

  async listSourceFiles(folder: string): Promise<SourceFile[]> {
    const relativePaths: string[] = [];
    await collectSourcePaths(folder, folder, relativePaths);
    const files: SourceFile[] = [];
    for (const relativePath of relativePaths.toSorted((left, right) => left.localeCompare(right, 'en'))) {
      files.push({ relativePath, content: await readFile(toNativePath(folder, relativePath), 'utf8') });
    }
    return files;
  }

  async createFolderAtomically(target: string, entries: readonly ProjectEntry[]): Promise<void> {
    const staging = join(dirname(target), `.${basename(target)}.uigen-${randomUUID()}`);
    const staged = entries.map((entry) => ({ entry, path: toNativePath(staging, entry.relativePath) }));
    try {
      await mkdir(staging);
      for (const { entry, path } of staged) await writeEntry(entry, path);
      await freeTarget(target);
      await rename(staging, target);
    } catch (error) {
      await removeLeftover(staging);
      throw writeFailure(target, error);
    }
  }

  async writeFileAtomically(folder: string, relativePath: string, content: string): Promise<void> {
    const target = toNativePath(folder, relativePath);
    const temporary = `${target}.${randomUUID()}.tmp`;
    try {
      await writeFile(temporary, content, { encoding: 'utf8', flag: 'wx' });
      await rename(temporary, target);
    } catch (error) {
      await removeLeftover(temporary);
      throw writeFailure(target, error);
    }
  }
}
