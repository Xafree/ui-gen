export type FolderState =
  | { readonly kind: 'missing' }
  | { readonly kind: 'not-a-folder' }
  | { readonly kind: 'empty' }
  | { readonly kind: 'not-empty'; readonly entryCount: number };

export type JsonFileRead =
  | { readonly kind: 'missing' }
  | { readonly kind: 'invalid-json' }
  | { readonly kind: 'ok'; readonly value: unknown };

export interface SourceFile {
  readonly relativePath: string;
  readonly content: string;
}

export type ProjectEntry =
  | { readonly kind: 'folder'; readonly relativePath: string }
  | { readonly kind: 'file'; readonly relativePath: string; readonly content: string };

export class ProjectWriteError extends Error {
  readonly path: string;
  readonly code: string;

  constructor(path: string, code: string, options?: ErrorOptions) {
    super(`Cannot write ${path} (${code})`, options);
    this.name = 'ProjectWriteError';
    this.path = path;
    this.code = code;
  }
}

export interface ProjectFileSystem {
  childPath(parent: string, name: string): string;
  readFolderState(path: string): Promise<FolderState>;
  readJsonFile(folder: string, relativePath: string): Promise<JsonFileRead>;
  listSourceFiles(folder: string): Promise<SourceFile[]>;
  createFolderAtomically(target: string, entries: readonly ProjectEntry[]): Promise<void>;
  writeFileAtomically(folder: string, relativePath: string, content: string): Promise<void>;
}
