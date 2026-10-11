export type FolderPurpose = 'open-project' | 'choose-location';

export interface FolderPicker {
  pickFolder(purpose: FolderPurpose): Promise<string | null>;
}
