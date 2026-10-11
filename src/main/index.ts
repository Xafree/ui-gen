import { join } from 'node:path';
import { app, BrowserWindow } from 'electron';

async function createWindow(): Promise<void> {
  const mainWindow = new BrowserWindow({
    width: 1280,
    height: 800,
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

  const devServerUrl = process.env['ELECTRON_RENDERER_URL'];
  if (!app.isPackaged && devServerUrl !== undefined) {
    await mainWindow.loadURL(devServerUrl);
  } else {
    await mainWindow.loadFile(join(import.meta.dirname, '../renderer/index.html'));
  }
}

app.on('window-all-closed', () => {
  app.quit();
});

async function startStudio(): Promise<void> {
  try {
    await createWindow();
  } catch (error) {
    console.error('Failed to start the studio', error);
    app.exit(1);
  }
}

app.once('ready', () => {
  void startStudio();
});
