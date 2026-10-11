import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { app, BrowserWindow, ipcMain } from 'electron';
import { createElectronFolderPicker } from '../modules/project/adapters/electron-dialog/electron-folder-picker.js';
import { NodeProjectFileSystem } from '../modules/project/adapters/node-fs/node-project-file-system.js';
import { JsonRecentProjectsStore } from '../modules/project/adapters/user-data/json-recent-projects-store.js';
import { RECENT_PROJECTS_FILE } from '../modules/project/domain/recents/recent-projects.js';
import { registerProjectHandlers } from './ipc/project-handlers.js';
import { createSenderGuard } from './ipc/sender-guard.js';
import { ProjectSession } from './session/project-session.js';

const USER_DATA_ARGUMENT = '--user-data-dir=';
const RENDERER_ENTRY = join(import.meta.dirname, '../renderer/index.html');

function userDataFolderFrom(argv: readonly string[]): string | null {
  const argument = argv.findLast((candidate) => candidate.startsWith(USER_DATA_ARGUMENT));
  const folder = argument?.slice(USER_DATA_ARGUMENT.length) ?? '';
  return folder === '' ? null : resolve(folder);
}

function rendererUrl(): string {
  const devServerUrl = process.env['ELECTRON_RENDERER_URL'];
  return !app.isPackaged && devServerUrl !== undefined ? devServerUrl : pathToFileURL(RENDERER_ENTRY).href;
}

function registerIpcHandlers(trustedRendererUrl: string): void {
  registerProjectHandlers(ipcMain, {
    project: {
      fileSystem: new NodeProjectFileSystem(),
      recentProjects: new JsonRecentProjectsStore(join(app.getPath('userData'), RECENT_PROJECTS_FILE)),
      clock: () => new Date(),
    },
    folderPicker: createElectronFolderPicker(() => BrowserWindow.getAllWindows()[0] ?? null),
    session: new ProjectSession(app.getPath('documents')),
    isTrustedSender: createSenderGuard(trustedRendererUrl),
  });
}

async function createWindow(url: string): Promise<void> {
  const mainWindow = new BrowserWindow({
    width: 1440,
    height: 900,
    useContentSize: true,
    show: false,
    webPreferences: {
      preload: join(import.meta.dirname, '../preload/index.cjs'),
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
    },
  });

  mainWindow.once('ready-to-show', () => {
    mainWindow.show();
  });
  mainWindow.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  mainWindow.webContents.on('will-navigate', (event) => {
    event.preventDefault();
  });

  await mainWindow.loadURL(url);
}

const userDataFolder = userDataFolderFrom(process.argv);
if (userDataFolder !== null) app.setPath('userData', userDataFolder);

app.on('window-all-closed', () => {
  app.quit();
});

async function startStudio(): Promise<void> {
  try {
    const url = rendererUrl();
    registerIpcHandlers(url);
    await createWindow(url);
  } catch (error) {
    console.error('Failed to start the studio', error);
    app.exit(1);
  }
}

app.once('ready', () => {
  void startStudio();
});
