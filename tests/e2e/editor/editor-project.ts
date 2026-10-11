import { createHash } from 'node:crypto';
import { readdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { ElectronApplication, Page } from '@playwright/test';
import {
  answerNextFolderDialog,
  closeStudio,
  launchStudio,
  writeFiles,
  type BridgeHost,
  type Studio,
} from '../project/studio-app';

export const COMPONENTS_DIR = 'src/components';

export const FEATURE_LIST_FILE = 'src/components/hz-feature-list.ts';
export const PLAN_BADGE_FILE = 'src/components/badge/hz-plan-badge.ts';
export const PRICING_CARD_FILE = 'src/components/hz-pricing-card.ts';
export const BARREL_FILE = 'src/components/index.ts';
export const OUTSIDE_COMPONENT_FILE = 'src/stories/hz-ghost.ts';

const PACKAGE_JSON = `{
  "name": "@kit-horizon/ui",
  "version": "1.3.0",
  "type": "module",
  "dependencies": {
    "lit": "^3.2.0"
  }
}
`;

const SETTINGS = `{
  "formatVersion": 1,
  "tagPrefix": "hz-",
  "componentsDir": "${COMPONENTS_DIR}"
}
`;

const BARREL = `export * from './hz-feature-list.js';
export * from './hz-pricing-card.js';
export * from './badge/hz-plan-badge.js';
`;

const OUTSIDE_COMPONENT = `import { LitElement, html } from 'lit';

export class HzGhost extends LitElement {
  render() {
    return html\`<p>Ghost</p>\`;
  }
}

customElements.define('hz-ghost', HzGhost);
`;

export interface ComponentEventProbe {
  componentChanges: unknown[];
  componentStatuses: unknown[];
}

async function fixture(name: string): Promise<string> {
  return readFile(join('tests', 'fixtures', 'components', name), 'utf8');
}

export async function fixtureText(file: string): Promise<string> {
  return fixture(file.slice(file.lastIndexOf('/') + 1));
}

export async function writeKitHorizon(root: string): Promise<void> {
  await writeFiles(root, {
    'package.json': PACKAGE_JSON,
    'uigen.json': SETTINGS,
    [FEATURE_LIST_FILE]: await fixtureText(FEATURE_LIST_FILE),
    [PLAN_BADGE_FILE]: await fixtureText(PLAN_BADGE_FILE),
    [PRICING_CARD_FILE]: await fixtureText(PRICING_CARD_FILE),
    [BARREL_FILE]: BARREL,
    [OUTSIDE_COMPONENT_FILE]: OUTSIDE_COMPONENT,
  });
}

export async function writeEmptyLibrary(root: string): Promise<void> {
  await writeFiles(root, {
    'package.json': PACKAGE_JSON,
    'uigen.json': SETTINGS,
    'src/components/README.md': '# Kit Horizon\n',
  });
}

export function sha256(text: string): string {
  return createHash('sha256').update(text, 'utf8').digest('hex');
}

export function endsWithFile(path: string, file: string): boolean {
  return path.replaceAll('\\', '/').endsWith(file);
}

export async function withStudio(userData: string, run: (studio: Studio) => Promise<void>): Promise<void> {
  const studio = await launchStudio(userData);
  try {
    await run(studio);
  } finally {
    await closeStudio(studio.app);
  }
}

export async function openProject(studio: Studio, path: string): Promise<void> {
  await answerNextFolderDialog(studio.app, path);
  const outcome = await studio.window.evaluate(() => (globalThis as unknown as BridgeHost).uiGen.project.openFolder());
  if (outcome.kind !== 'opened') throw new Error(`Expected ${path} to open, got ${outcome.kind}`);
}

export interface ListedComponent {
  readonly path: string;
  readonly tag: string;
  readonly name: string;
}

export async function listComponents(window: Page): Promise<ListedComponent[]> {
  return window.evaluate(async () => {
    const summaries = await (globalThis as unknown as BridgeHost).uiGen.components.list();
    return summaries.map(({ path, tag, name }) => ({ path, tag, name }));
  });
}

export async function listedPath(window: Page, tag: string): Promise<string> {
  const listed = (await listComponents(window)).find((component) => component.tag === tag);
  if (listed === undefined) throw new Error(`Component ${tag} is not listed`);
  return listed.path;
}

export async function openComponent(window: Page, path: string): Promise<unknown> {
  return window.evaluate((target) => (globalThis as unknown as BridgeHost).uiGen.components.open(target), path);
}

export async function lastOpened(window: Page): Promise<unknown> {
  return window.evaluate(() => (globalThis as unknown as BridgeHost).uiGen.components.lastOpened());
}

export async function refusalOf(window: Page, path: unknown): Promise<string> {
  return window.evaluate(
    (target) =>
      (globalThis as unknown as BridgeHost).uiGen.components.open(target as string).then(
        () => 'accepted',
        (error: unknown) => String(error),
      ),
    path,
  );
}

export async function startProbe(window: Page): Promise<void> {
  await window.evaluate(() => {
    const probe = globalThis as unknown as ComponentEventProbe;
    probe.componentChanges = [];
    probe.componentStatuses = [];
    const components = (globalThis as unknown as BridgeHost).uiGen.components;
    components.onChanged((change) => {
      probe.componentChanges.push(change);
    });
    components.onStatus((status) => {
      probe.componentStatuses.push(status);
    });
  });
}

export async function changesSeen(window: Page): Promise<unknown[]> {
  return window.evaluate(() => (globalThis as unknown as ComponentEventProbe).componentChanges);
}

export async function statusesSeen(window: Page): Promise<unknown[]> {
  return window.evaluate(() => (globalThis as unknown as ComponentEventProbe).componentStatuses);
}

export async function snapshotFolder(root: string): Promise<Record<string, string>> {
  const entries = await readdir(root, { recursive: true, withFileTypes: true });
  const files = entries.filter((entry) => entry.isFile()).map((entry) => join(entry.parentPath, entry.name));
  const snapshot: Record<string, string> = {};
  for (const file of files.toSorted()) snapshot[file] = await readFile(file, 'utf8');
  return snapshot;
}

export function mainProcessLog(app: ElectronApplication): () => string {
  const chunks: string[] = [];
  const record = (chunk: Buffer): void => {
    chunks.push(chunk.toString('utf8'));
  };
  app.process().stdout?.on('data', record);
  app.process().stderr?.on('data', record);
  return () => chunks.join('');
}

export async function analysisProcessIds(app: ElectronApplication): Promise<number[]> {
  return app.evaluate(({ app: electronApp }) =>
    electronApp
      .getAppMetrics()
      .filter((metric) => metric.type === 'Utility' && metric.serviceName === 'node.mojom.NodeService')
      .map((metric) => metric.pid),
  );
}
