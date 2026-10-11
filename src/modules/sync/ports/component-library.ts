export type StopWatching = () => void;

export interface ComponentLibrary {
  listFiles(componentsDir: string): Promise<readonly string[]>;
  readText(path: string): Promise<string>;
  watch(componentsDir: string, onChange: (path: string) => void): StopWatching;
}
