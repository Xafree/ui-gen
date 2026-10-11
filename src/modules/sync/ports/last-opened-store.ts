export interface LastOpenedStore {
  load(projectPath: string): Promise<string | null>;
  save(projectPath: string, componentPath: string): Promise<void>;
}
