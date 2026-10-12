import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import { closeStudio, launchStudio, makeTempFolder, removeFolder, writeFiles } from '../../project/studio-app';
import { canvasContent, openRecentProject, rememberOpenedComponent } from './canvas-project';

const PACKAGE_JSON = '{ "name": "@kit-horizon/viewport-height", "version": "0.1.0", "type": "module", "dependencies": { "lit": "^3.3.0" } }\n';
const SETTINGS = '{ "formatVersion": 1, "tagPrefix": "hz-", "componentsDir": "src/components" }\n';
const TSCONFIG =
  '{ "compilerOptions": { "target": "ES2022", "module": "ESNext", "moduleResolution": "bundler", "experimentalDecorators": true, "useDefineForClassFields": false, "strict": true }, "include": ["src"] }\n';
const MAX_FRAME_HEIGHT = 4000;

const HERO = `import { LitElement, css, html } from 'lit';
import { customElement } from 'lit/decorators.js';

@customElement('hz-hero')
export class HzHero extends LitElement {
  static styles = css\`.hero { min-height: 100vh; background: rgb(240, 240, 250); }\`;

  render() {
    return html\`<section class="hero"><h1>Hero</h1></section>\`;
  }
}
`;

interface Prepared {
  readonly root: string;
  readonly userData: string;
}

async function prepare(label: string, tag: string, files: Readonly<Record<string, string>>): Promise<Prepared> {
  const root = await makeTempFolder(label);
  const userData = await makeTempFolder(`${label}-user-data`);
  const project = join(root, 'kit-horizon');
  await writeFiles(project, { 'package.json': PACKAGE_JSON, 'uigen.json': SETTINGS, 'tsconfig.json': TSCONFIG, ...files });
  await rememberOpenedComponent(userData, project, tag);
  return { root, userData };
}

async function dispose({ root, userData }: Prepared): Promise<void> {
  await removeFolder(root);
  await removeFolder(userData);
}

// covers: FCT-0035
test('keeps the frame bounded for a component that fills the viewport height', async () => {
  const prepared = await prepare('viewport-height', 'hz-hero', { 'src/components/hz-hero.ts': HERO });
  const studio = await launchStudio(prepared.userData);
  try {
    await openRecentProject(studio.window);
    await expect(canvasContent(studio.window).locator('hz-hero')).toHaveCount(1);
    const frameHeight = (): Promise<number> => studio.window.locator('#frame-body').evaluate((element: { offsetHeight: number }) => element.offsetHeight);
    await expect.poll(frameHeight).toBeGreaterThan(320);
    await studio.window.waitForTimeout(1500);

    const settled = await frameHeight();
    await studio.window.waitForTimeout(1000);

    expect(settled).toBeLessThanOrEqual(MAX_FRAME_HEIGHT);
    expect(await frameHeight()).toBe(settled);
  } finally {
    await closeStudio(studio.app);
    await dispose(prepared);
  }
});
