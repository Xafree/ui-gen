import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test, type Page } from '@playwright/test';
import { makeTempFolder, removeFolder } from '../project/studio-app';
import { canvasFrame, openRecentProject } from '../editor/canvas/canvas-project';
import {
  PRICING_CARD_FILE,
  fixtureText,
  listedPath,
  openComponent,
  openProject,
  withStudio,
  writeKitHorizon,
} from '../editor/editor-project';
import {
  expectEditorOn,
  rememberRecentProject,
  writeKitHorizonSupport,
  writeTsconfig,
} from '../editor/screen/editor-screen';

const SAMPLES = 20;
const PRICING_TAG = 'hz-pricing-card';
const WAIT_LIMIT_MS = 10_000;
const LAYERS_CODE_TARGET_MS = 150;
const CANVAS_TARGET_MS = 300;
const VARIANT_BRANCHES = 3;
const HEADER_OPENING = '<header>\n';

let root: string;
let userData: string;
let project: string;

test.beforeEach(async () => {
  root = await makeTempFolder('editor-latency');
  userData = await makeTempFolder('editor-latency-user-data');
  project = join(root, 'kit-horizon');
});

test.afterEach(async () => {
  await removeFolder(root);
  await removeFolder(userData);
});

function withProbes(source: string, count: number): string {
  const plain = '<span class="probe"></span>'.repeat(count - 1);
  const last = `<span class="probe" data-last="${String(count)}"></span>`;
  return source.replace(HEADER_OPENING, () => `${HEADER_OPENING}        ${plain}${last}\n`);
}

function percentile95(values: readonly number[]): number {
  const sorted = values.toSorted((left, right) => left - right);
  return sorted[Math.ceil(sorted.length * 0.95) - 1] ?? Number.NaN;
}

async function elapsedUntil(startedAt: number, label: string, reached: () => Promise<boolean>): Promise<number> {
  for (;;) {
    if (await reached()) return Date.now() - startedAt;
    if (Date.now() - startedAt > WAIT_LIMIT_MS) throw new Error(`${label} did not show the written file within ${String(WAIT_LIMIT_MS)} ms`);
  }
}

async function layerCount(window: Page): Promise<number> {
  const text = await window.locator('#layer-count').textContent();
  return Number(/\d+/.exec(text ?? '')?.[0]);
}

async function canvasProbes(window: Page): Promise<number> {
  try {
    const frame = await canvasFrame(window);
    const count: unknown = await frame.evaluate(
      `document.querySelector("${PRICING_TAG}")?.shadowRoot?.querySelectorAll(".probe").length ?? 0`,
    );
    return typeof count === 'number' ? count : 0;
  } catch {
    return 0;
  }
}

// covers: FCT-0030
// covers: FCT-0035
test('reports how fast a written component file reaches layers, code and canvas', async () => {
  test.setTimeout(240_000);
  await writeKitHorizon(project);
  await writeTsconfig(project);
  await writeKitHorizonSupport(project);
  await withStudio(userData, async (studio) => {
    await openProject(studio, project);
    await openComponent(studio.window, await listedPath(studio.window, PRICING_TAG));
  });
  await rememberRecentProject(userData, project);
  const original = await fixtureText(PRICING_CARD_FILE);

  await withStudio(userData, async ({ window }) => {
    await openRecentProject(window);
    await expectEditorOn(window, PRICING_TAG);
    await expect(window.locator('#layer-count')).toHaveText(/\d/);
    await expect.poll(() => canvasFrame(window).then(() => true, () => false)).toBe(true);
    const baseCount = await layerCount(window);
    const layersAndCode: number[] = [];
    const canvas: number[] = [];

    for (let sample = 1; sample <= SAMPLES; sample += 1) {
      const startedAt = Date.now();
      const written = writeFile(join(project, PRICING_CARD_FILE), withProbes(original, sample), 'utf8');
      const [layers, code, painted] = await Promise.all([
        elapsedUntil(startedAt, 'layers', async () => (await layerCount(window)) === baseCount + sample * VARIANT_BRANCHES),
        elapsedUntil(startedAt, 'code', async () =>
          ((await window.locator('#code').textContent()) ?? '').includes(`data-last="${String(sample)}"`),
        ),
        elapsedUntil(startedAt, 'canvas', async () => (await canvasProbes(window)) === sample),
        written,
      ]);
      layersAndCode.push(Math.max(layers, code));
      canvas.push(painted);
    }

    const layersCodeP95 = percentile95(layersAndCode);
    const canvasP95 = percentile95(canvas);
    test.info().annotations.push(
      { type: 'latency-p95-layers-and-code-ms', description: `${String(layersCodeP95)} (target ${String(LAYERS_CODE_TARGET_MS)}, report only)` },
      { type: 'latency-p95-canvas-ms', description: `${String(canvasP95)} (target ${String(CANVAS_TARGET_MS)}, report only)` },
      { type: 'latency-layers-and-code-samples-ms', description: layersAndCode.join(',') },
      { type: 'latency-canvas-samples-ms', description: canvas.join(',') },
    );
    expect(layersAndCode).toHaveLength(SAMPLES);
    expect(canvas).toHaveLength(SAMPLES);
  });
});
