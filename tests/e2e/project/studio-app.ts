import { spawnSync } from 'node:child_process';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { _electron as electron, type ElectronApplication, type Page } from '@playwright/test';
import type { UiGenApi } from '../../../src/preload/ui-gen-api';

export interface BridgeHost {
  readonly uiGen: UiGenApi;
}

export interface Studio {
  readonly app: ElectronApplication;
  readonly window: Page;
}

export interface RecentEntry {
  readonly path: string;
  readonly name: string;
  readonly openedAt: string;
}

export async function makeTempFolder(label: string): Promise<string> {
  return mkdtemp(join(tmpdir(), `ui-gen-e2e-${label}-`));
}

export async function removeFolder(path: string): Promise<void> {
  await rm(path, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
}

export async function writeRecents(userData: string, projects: readonly RecentEntry[]): Promise<void> {
  await writeFile(
    join(userData, 'recent-projects.json'),
    `${JSON.stringify({ formatVersion: 1, projects }, null, 2)}\n`,
    'utf8',
  );
}

export async function writeFiles(root: string, files: Readonly<Record<string, string>>): Promise<void> {
  for (const [relativePath, content] of Object.entries(files)) {
    const path = join(root, relativePath);
    await mkdir(join(path, '..'), { recursive: true });
    await writeFile(path, content, 'utf8');
  }
}

export async function launchStudio(userData: string): Promise<Studio> {
  const app = await electron.launch({
    args: ['.', `--user-data-dir=${userData}`],
    cwd: process.cwd(),
    colorScheme: null,
  });
  try {
    const window = await app.firstWindow();
    await window.waitForLoadState('domcontentloaded');
    return { app, window };
  } catch (error) {
    await closeStudio(app);
    throw error;
  }
}

export async function answerNextFolderDialog(app: ElectronApplication, path: string): Promise<void> {
  await app.evaluate(({ dialog }, chosen) => {
    dialog.showOpenDialog = () => Promise.resolve({ canceled: false, filePaths: [chosen] });
  }, path);
}

export async function closeStudio(app: ElectronApplication): Promise<void> {
  const child = app.process();
  try {
    await app.close();
  } finally {
    if (child.pid !== undefined && child.exitCode === null && child.signalCode === null) {
      spawnSync('taskkill', ['/T', '/F', '/PID', String(child.pid)], { stdio: 'ignore' });
    }
  }
}
