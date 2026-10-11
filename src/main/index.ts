import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { app, BrowserWindow, ipcMain } from 'electron';
import { createElectronFolderPicker } from '../modules/project/adapters/electron-dialog/electron-folder-picker.js';
import { NodeProjectFileSystem } from '../modules/project/adapters/node-fs/node-project-file-system.js';
import { NodeComponentLibrary } from '../modules/sync/adapters/node-fs/node-component-library.js';
import { JsonLastOpenedStore } from '../modules/sync/adapters/user-data/json-last-opened-store.js';
import { ComputeClient } from '../modules/sync/adapters/utility-process/compute-client.js';
import { LAST_OPENED_FILE } from '../modules/sync/domain/library/last-opened.js';
import { JsonRecentProjectsStore } from '../modules/project/adapters/user-data/json-recent-projects-store.js';
import { RECENT_PROJECTS_FILE } from '../modules/project/domain/recents/recent-projects.js';
import { denyCanvasPermissions, guardCanvasFrames } from './canvas/canvas-guards.js';
import { createCanvasHandler } from './canvas/canvas-handler.js';
import { handleCanvasProtocol, registerCanvasScheme } from './canvas/canvas-protocol.js';
import { registerComponentHandlers } from './components/component-handlers.js';
import { ComponentSession } from './components/component-session.js';
import { createStudioPublisher } from './components/studio-publisher.js';
import { forkComputeProcess } from './compute/electron-compute-process.js';
import { PUSH_CHANNELS } from './ipc/channels.js';
import { registerProjectHandlers } from './ipc/project-handlers.js';
import { registerRecentListHandlers } from './ipc/recent-list-handlers.js';
import { createSenderGuard } from './ipc/sender-guard.js';
import { ForgottenRecents } from './session/forgotten-recents.js';
import { ProjectSession } from './session/project-session.js';

const USER_DATA_ARGUMENT = '--user-data-dir=';
const RENDERER_ENTRY = join(import.meta.dirname, '../renderer/index.html');
const COMPUTE_HOST_ENTRY = join(import.meta.dirname, 'compute-host.js');
const CANVAS_HARNESS_ENTRY = join(import.meta.dirname, 'canvas-harness.js');

let studioWindow: BrowserWindow | null = null;
let analyzer: ComputeClient | null = null;
let components: ComponentSession | null = null;

function userDataFolderFrom(argv: readonly string[]): string | null {
  const argument = argv.findLast((candidate) => candidate.startsWith(USER_DATA_ARGUMENT));
  const folder = argument?.slice(USER_DATA_ARGUMENT.length) ?? '';
  return folder === '' ? null : resolve(folder);
}

function rendererUrl(): string {
  const devServerUrl = process.env['ELECTRON_RENDERER_URL'];
  return !app.isPackaged && devServerUrl !== undefined ? devServerUrl : pathToFileURL(RENDERER_ENTRY).href;
}

function startComponents(project: ProjectSession): ComponentSession {
  const publish = createStudioPublisher(() => studioWindow?.webContents ?? null);
  analyzer = new ComputeClient({
    spawn: () => forkComputeProcess(COMPUTE_HOST_ENTRY),
    onStatus: (status) => {
      publish(PUSH_CHANNELS.componentStatus, status);
    },
  });
  analyzer.start();
  components = new ComponentSession({
    project,
    library: new NodeComponentLibrary(),
    analyzer,
    lastOpened: new JsonLastOpenedStore(join(app.getPath('userData'), LAST_OPENED_FILE)),
    publish: (change) => {
      publish(PUSH_CHANNELS.componentChanged, change);
    },
  });
  return components;
}

function startCanvas(project: ProjectSession, compute: ComputeClient): void {
  handleCanvasProtocol(
    createCanvasHandler({
      projectRoot: () => project.currentProject?.path ?? null,
      compute,
      studio: {
        harnessScript: CANVAS_HARNESS_ENTRY,
        packagesRoot: join(app.getAppPath(), 'node_modules'),
      },
    }),
  );
}

function registerIpcHandlers(trustedRendererUrl: string): void {
  const project = {
    fileSystem: new NodeProjectFileSystem(),
    recentProjects: new JsonRecentProjectsStore(join(app.getPath('userData'), RECENT_PROJECTS_FILE)),
    clock: () => new Date(),
  };
  const isTrustedSender = createSenderGuard(trustedRendererUrl);
  const session = new ProjectSession(app.getPath('documents'));
  registerProjectHandlers(ipcMain, {
    project,
    folderPicker: createElectronFolderPicker(() => BrowserWindow.getAllWindows()[0] ?? null),
    session,
    isTrustedSender,
  });
  registerComponentHandlers(ipcMain, { components: startComponents(session), isTrustedSender });
  if (analyzer !== null) startCanvas(session, analyzer);
  registerRecentListHandlers(ipcMain, { project, forgotten: new ForgottenRecents(), isTrustedSender });
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
      nodeIntegrationInSubFrames: false,
    },
  });

  studioWindow = mainWindow;
  mainWindow.once('ready-to-show', () => {
    if (process.env['UIGEN_SHOW_INACTIVE'] === '1') mainWindow.showInactive();
    else mainWindow.show();
  });
  guardCanvasFrames(mainWindow.webContents);
  denyCanvasPermissions(mainWindow.webContents.session);
  mainWindow.webContents.on('will-navigate', (event) => {
    event.preventDefault();
  });

  await mainWindow.loadURL(url);
}

registerCanvasScheme();

const userDataFolder = userDataFolderFrom(process.argv);
if (userDataFolder !== null) app.setPath('userData', userDataFolder);

app.on('window-all-closed', () => {
  app.quit();
});

app.on('will-quit', () => {
  components?.dispose();
  analyzer?.stop();
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
