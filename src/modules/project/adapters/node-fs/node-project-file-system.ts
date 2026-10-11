import { randomUUID } from 'node:crypto';
import { lstat, mkdir, readdir, readFile, rename, rm, rmdir, stat, writeFile } from 'node:fs/promises';
import { basename, dirname, isAbsolute, join, relative, sep } from 'node:path';
import { setTimeout as pause } from 'node:timers/promises';
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
const MAX_SOURCE_FILES = 2000;
const SKIPPABLE_CODES: ReadonlySet<string> = new Set(['ENOENT', 'EACCES', 'EPERM']);
const BYTE_ORDER_MARK = '\uFEFF';
const LEFTOVER_REMOVAL = { recursive: true, force: true, maxRetries: 3 } as const;
const RETRYABLE_RENAME_CODES: ReadonlySet<string> = new Set(['EPERM', 'EBUSY', 'EACCES']);
const RENAME_RETRIES = 3;
const RENAME_RETRY_DELAY_MS = 100;

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
  if (!isInside(folder, path)) {
    console.error(`Refused ${JSON.stringify(relativePath)}: it leaves ${folder}`);
    throw new ProjectWriteError(path, 'EINVAL');
  }
  return path;
}

function comparePaths(left: string, right: string): number {
  return left.localeCompare(right, 'en') || (left < right ? -1 : 1);
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

function relativeSlashPath(root: string, path: string): string {
  return relative(root, path).replaceAll(sep, '/');
}

function isSkippableCode(error: unknown): boolean {
  return SKIPPABLE_CODES.has(errorCode(error));
}

function logSkipped(root: string, path: string, error: unknown): void {
  console.warn(`Skipped unreadable entry ${relativeSlashPath(root, path)} (${errorCode(error)})`);
}

async function isReadableSize(root: string, path: string): Promise<boolean> {
  try {
    return (await stat(path)).size <= MAX_SOURCE_BYTES;
  } catch (error) {
    if (!isSkippableCode(error)) throw error;
    logSkipped(root, path, error);
    return false;
  }
}

async function listEntries(root: string, folder: string) {
  if (folder === root) return await readdir(folder, { withFileTypes: true });
  try {
    return await readdir(folder, { withFileTypes: true });
  } catch (error) {
    if (!isSkippableCode(error)) throw error;
    logSkipped(root, folder, error);
    return [];
  }
}

async function collectSourcePaths(root: string, folder: string, found: string[]): Promise<void> {
  for (const entry of await listEntries(root, folder)) {
    const path = join(folder, entry.name);
    if (entry.isDirectory()) {
      if (isTraversable(entry.name)) await collectSourcePaths(root, path, found);
    } else if (entry.isFile() && isSourceName(entry.name) && (await isReadableSize(root, path))) {
      found.push(relativeSlashPath(root, path));
    }
  }
}

async function readSourceIfReadable(folder: string, relativePath: string): Promise<SourceFile | null> {
  const path = toNativePath(folder, relativePath);
  try {
    return { relativePath, content: await readFile(path, 'utf8') };
  } catch (error) {
    if (!isSkippableCode(error)) throw error;
    logSkipped(folder, path, error);
    return null;
  }
}

async function readFirstSources(folder: string, relativePaths: readonly string[]): Promise<SourceFile[]> {
  const files: SourceFile[] = [];
  for (const relativePath of relativePaths) {
    if (files.length === MAX_SOURCE_FILES) break;
    const file = await readSourceIfReadable(folder, relativePath);
    if (file !== null) files.push(file);
  }
  return files;
}

async function lstatIfPresent(path: string) {
  try {
    return await lstat(path);
  } catch (error) {
    if (errorCode(error) === 'ENOENT') return null;
    throw error;
  }
}

async function freeTarget(target: string): Promise<boolean> {
  const info = await lstatIfPresent(target);
  if (info === null) return false;
  if (!info.isDirectory()) throw new ProjectWriteError(target, 'EEXIST');
  await rmdir(target);
  return true;
}

async function renameWithRetry(from: string, to: string, retriesLeft = RENAME_RETRIES): Promise<void> {
  try {
    await rename(from, to);
  } catch (error) {
    if (retriesLeft === 0 || !RETRYABLE_RENAME_CODES.has(errorCode(error))) throw error;
    await pause(RENAME_RETRY_DELAY_MS);
    await renameWithRetry(from, to, retriesLeft - 1);
  }
}

async function restoreEmptyFolder(path: string): Promise<void> {
  try {
    await mkdir(path);
  } catch (error) {
    console.error(`Cannot put back the empty folder ${path}`, error);
  }
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
    try {
      const relativePaths: string[] = [];
      await collectSourcePaths(folder, folder, relativePaths);
      return await readFirstSources(folder, relativePaths.toSorted(comparePaths));
    } catch (error) {
      throw writeFailure(folder, error);
    }
  }

  async createFolderAtomically(target: string, entries: readonly ProjectEntry[]): Promise<void> {
    const staging = join(dirname(target), `.${basename(target)}.uigen-${randomUUID()}`);
    const staged = entries.map((entry) => ({ entry, path: toNativePath(staging, entry.relativePath) }));
    let replacedEmptyFolder = false;
    try {
      await mkdir(staging);
      for (const { entry, path } of staged) await writeEntry(entry, path);
      replacedEmptyFolder = await freeTarget(target);
      await renameWithRetry(staging, target);
    } catch (error) {
      await removeLeftover(staging);
      if (replacedEmptyFolder) await restoreEmptyFolder(target);
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
