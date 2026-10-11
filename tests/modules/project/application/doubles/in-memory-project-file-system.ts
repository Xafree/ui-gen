import {
  ProjectWriteError,
  type FolderState,
  type JsonFileRead,
  type ProjectEntry,
  type ProjectFileSystem,
  type SourceFile,
} from '../../../../../src/modules/project/ports/project-file-system';

const SEPARATOR = '\\';

export interface CreateFolderCall {
  readonly target: string;
  readonly entries: readonly ProjectEntry[];
}

export interface DiskEntry {
  readonly path: string;
  readonly content: string | null;
}

function toNativePath(relativePath: string): string {
  return relativePath.split('/').join(SEPARATOR);
}

function parentsOf(path: string, root: string): string[] {
  const parents: string[] = [];
  let current = path;
  while (current.length > root.length) {
    const lastSeparator = current.lastIndexOf(SEPARATOR);
    if (lastSeparator <= 0) break;
    current = current.slice(0, lastSeparator);
    parents.push(current);
  }
  return parents;
}

export class InMemoryProjectFileSystem implements ProjectFileSystem {
  readonly createFolderCalls: CreateFolderCall[] = [];
  readonly writtenFiles: string[] = [];
  private readonly folders = new Set<string>();
  private readonly files = new Map<string, string>();
  private creationFailure: Error | null = null;

  addFolder(path: string): this {
    this.folders.add(path);
    return this;
  }

  addFile(path: string, content: string): this {
    this.files.set(path, content);
    return this;
  }

  failNextCreationWith(error: Error): this {
    this.creationFailure = error;
    return this;
  }

  hasFolder(path: string): boolean {
    return this.folders.has(path);
  }

  fileContent(path: string): string | undefined {
    return this.files.get(path);
  }

  snapshot(): DiskEntry[] {
    const folders = [...this.folders].map((path): DiskEntry => ({ path, content: null }));
    const files = [...this.files].map(([path, content]): DiskEntry => ({ path, content }));
    return [...folders, ...files].sort((left, right) => left.path.localeCompare(right.path));
  }

  childPath(parent: string, name: string): string {
    return `${parent}${SEPARATOR}${name}`;
  }

  readFolderState(path: string): Promise<FolderState> {
    if (this.files.has(path)) return Promise.resolve({ kind: 'not-a-folder' });
    if (!this.folders.has(path)) return Promise.resolve({ kind: 'missing' });
    const entryCount = this.childrenOf(path).length;
    return Promise.resolve(entryCount === 0 ? { kind: 'empty' } : { kind: 'not-empty', entryCount });
  }

  readJsonFile(folder: string, relativePath: string): Promise<JsonFileRead> {
    const content = this.files.get(this.childPath(folder, toNativePath(relativePath)));
    if (content === undefined) return Promise.resolve({ kind: 'missing' });
    try {
      const value: unknown = JSON.parse(content);
      return Promise.resolve({ kind: 'ok', value });
    } catch {
      return Promise.resolve({ kind: 'invalid-json' });
    }
  }

  listSourceFiles(folder: string): Promise<SourceFile[]> {
    const prefix = `${folder}${SEPARATOR}`;
    const sources = [...this.files]
      .filter(([path]) => path.startsWith(prefix))
      .map(([path, content]): SourceFile => ({ relativePath: path.slice(prefix.length).split(SEPARATOR).join('/'), content }));
    return Promise.resolve(sources);
  }

  createFolderAtomically(target: string, entries: readonly ProjectEntry[]): Promise<void> {
    this.createFolderCalls.push({ target, entries });
    const failure = this.creationFailure;
    this.creationFailure = null;
    if (failure !== null) return Promise.reject(failure);
    if (this.files.has(target)) return Promise.reject(new ProjectWriteError(target, 'EEXIST'));
    if (this.folders.has(target) && this.childrenOf(target).length > 0) {
      return Promise.reject(new ProjectWriteError(target, 'ENOTEMPTY'));
    }
    this.folders.add(target);
    for (const entry of entries) this.applyEntry(target, entry);
    return Promise.resolve();
  }

  writeFileAtomically(folder: string, relativePath: string, content: string): Promise<void> {
    const path = this.childPath(folder, toNativePath(relativePath));
    this.writtenFiles.push(path);
    this.files.set(path, content);
    return Promise.resolve();
  }

  private applyEntry(target: string, entry: ProjectEntry): void {
    const path = this.childPath(target, toNativePath(entry.relativePath));
    for (const parent of parentsOf(path, target)) this.folders.add(parent);
    if (entry.kind === 'folder') this.folders.add(path);
    else this.files.set(path, entry.content);
  }

  private childrenOf(path: string): string[] {
    const prefix = `${path}${SEPARATOR}`;
    return [...this.folders, ...this.files.keys()].filter(
      (candidate) => candidate.startsWith(prefix) && !candidate.slice(prefix.length).includes(SEPARATOR),
    );
  }
}
