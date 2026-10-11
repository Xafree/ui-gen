import { dialog, type BrowserWindow, type OpenDialogOptions } from 'electron';
import type { FolderPicker, FolderPurpose } from '../../ports/folder-picker.js';

const OPTIONS: Readonly<Record<FolderPurpose, OpenDialogOptions>> = {
  'open-project': {
    title: 'Ouvrir un projet',
    buttonLabel: 'Ouvrir le dossier',
    properties: ['openDirectory'],
  },
  'choose-location': {
    title: "Choisir l'emplacement",
    buttonLabel: 'Choisir ce dossier',
    properties: ['openDirectory', 'createDirectory'],
  },
};

export function createElectronFolderPicker(getOwnerWindow: () => BrowserWindow | null): FolderPicker {
  return {
    async pickFolder(purpose) {
      const owner = getOwnerWindow();
      const options = OPTIONS[purpose];
      const result =
        owner === null ? await dialog.showOpenDialog(options) : await dialog.showOpenDialog(owner, options);
      const [path] = result.filePaths;
      return result.canceled || path === undefined ? null : path;
    },
  };
}
