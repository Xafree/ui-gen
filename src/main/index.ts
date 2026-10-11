import { join } from 'node:path';
import { app, BrowserWindow } from 'electron';

function creerFenetre(): void {
  const fenetre = new BrowserWindow({
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

  fenetre.once('ready-to-show', () => {
    fenetre.show();
  });
  fenetre.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  fenetre.webContents.on('will-navigate', (evenement) => {
    evenement.preventDefault();
  });

  const urlDeDeveloppement = process.env['ELECTRON_RENDERER_URL'];
  const chargement =
    !app.isPackaged && urlDeDeveloppement !== undefined
      ? fenetre.loadURL(urlDeDeveloppement)
      : fenetre.loadFile(join(import.meta.dirname, '../renderer/index.html'));
  chargement.catch((erreur: unknown) => {
    console.error('Chargement de la fenêtre impossible', erreur);
    app.exit(1);
  });
}

app.on('window-all-closed', () => {
  app.quit();
});

app
  .whenReady()
  .then(creerFenetre)
  .catch((erreur: unknown) => {
    console.error('Démarrage du studio impossible', erreur);
    app.exit(1);
  });
