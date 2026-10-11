import { dialog, type BrowserWindow } from 'electron';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createElectronFolderPicker } from '../../../../../src/modules/project/adapters/electron-dialog/electron-folder-picker';

vi.mock('electron', () => ({
  dialog: { showOpenDialog: () => Promise.reject(new Error('showOpenDialog must be stubbed by the test')) },
}));

const studioWindow = { id: 1 } as unknown as BrowserWindow;
const chosenFolder = 'E:\\projets\\kit-horizon';

afterEach(() => {
  vi.restoreAllMocks();
});

describe('createElectronFolderPicker', () => {
  it('asks for a project folder on the studio window and returns the chosen path', async () => {
    const showOpenDialog = vi
      .spyOn(dialog, 'showOpenDialog')
      .mockResolvedValue({ canceled: false, filePaths: [chosenFolder] });

    const path = await createElectronFolderPicker(() => studioWindow).pickFolder('open-project');

    expect(path).toBe(chosenFolder);
    expect(showOpenDialog).toHaveBeenCalledExactlyOnceWith(studioWindow, {
      title: 'Ouvrir un projet',
      buttonLabel: 'Ouvrir le dossier',
      properties: ['openDirectory'],
    });
  });

  it('lets the user create a folder when choosing a location', async () => {
    const showOpenDialog = vi
      .spyOn(dialog, 'showOpenDialog')
      .mockResolvedValue({ canceled: false, filePaths: [chosenFolder] });

    await createElectronFolderPicker(() => studioWindow).pickFolder('choose-location');

    expect(showOpenDialog).toHaveBeenCalledExactlyOnceWith(studioWindow, {
      title: "Choisir l'emplacement",
      buttonLabel: 'Choisir ce dossier',
      properties: ['openDirectory', 'createDirectory'],
    });
  });

  it('opens the dialog without an owner when there is no studio window', async () => {
    const showOpenDialog = vi
      .spyOn(dialog, 'showOpenDialog')
      .mockResolvedValue({ canceled: false, filePaths: [chosenFolder] });

    await createElectronFolderPicker(() => null).pickFolder('open-project');

    expect(showOpenDialog).toHaveBeenCalledExactlyOnceWith({
      title: 'Ouvrir un projet',
      buttonLabel: 'Ouvrir le dossier',
      properties: ['openDirectory'],
    });
  });

  it('returns null when the user cancels the dialog', async () => {
    vi.spyOn(dialog, 'showOpenDialog').mockResolvedValue({ canceled: true, filePaths: [] });

    await expect(createElectronFolderPicker(() => studioWindow).pickFolder('open-project')).resolves.toBeNull();
  });

  it('returns null when the dialog is confirmed without a path', async () => {
    vi.spyOn(dialog, 'showOpenDialog').mockResolvedValue({ canceled: false, filePaths: [] });

    await expect(createElectronFolderPicker(() => studioWindow).pickFolder('open-project')).resolves.toBeNull();
  });

  it('reads the dialog function when it is called, so that a test can replace it later', async () => {
    const picker = createElectronFolderPicker(() => studioWindow);
    const replacement = vi
      .spyOn(dialog, 'showOpenDialog')
      .mockResolvedValue({ canceled: false, filePaths: [chosenFolder] });

    await picker.pickFolder('open-project');

    expect(replacement).toHaveBeenCalledOnce();
  });
});
