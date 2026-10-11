import { readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test, type Page } from '@playwright/test';
import { makeTempFolder, removeFolder } from '../project/studio-app';
import {
  OUTSIDE_COMPONENT_FILE,
  PLAN_BADGE_FILE,
  analysisProcessIds,
  changesSeen,
  listedPath,
  openComponent,
  openProject,
  sha256,
  startProbe,
  statusesSeen,
  withStudio,
  writeKitHorizon,
} from './editor-project';

const LABEL_LINE = "@property() label = ''";

let root: string;
let userData: string;
let kit: string;
let readable: string;

test.beforeEach(async () => {
  root = await makeTempFolder('editor-changes');
  userData = await makeTempFolder('user-data');
  kit = join(root, 'kit-horizon');
  await writeKitHorizon(kit);
  readable = await readFile(join(kit, PLAN_BADGE_FILE), 'utf8');
});

test.afterEach(async () => {
  await removeFolder(root);
  await removeFolder(userData);
});

function withLabel(label: string): string {
  return readable.replace(LABEL_LINE, `@property() label = '${label}'`);
}

async function writeBadge(text: string): Promise<void> {
  await writeFile(join(kit, PLAN_BADGE_FILE), text, 'utf8');
}

async function waitForChangeCount(window: Page, count: number): Promise<void> {
  await expect.poll(async () => (await changesSeen(window)).length, { timeout: 10_000 }).toBeGreaterThanOrEqual(count);
}

async function waitForChangedText(window: Page, text: string): Promise<void> {
  await expect
    .poll(async () => (await changesSeen(window)).some((change) => (change as { text?: unknown }).text === text), {
      timeout: 10_000,
    })
    .toBe(true);
}

// covers: FCT-0042
test('pushes the new version of an open component modified outside the studio, once per write', async () => {
  const first = withLabel('Pro');
  const second = withLabel('Équipe');

  await withStudio(userData, async (studio) => {
    await openProject(studio, kit);
    const path = await listedPath(studio.window, 'hz-plan-badge');
    await openComponent(studio.window, path);
    await startProbe(studio.window);

    await writeBadge(first);
    await waitForChangeCount(studio.window, 1);
    await writeBadge(second);
    await waitForChangedText(studio.window, second);

    const changes = await changesSeen(studio.window);
    expect(changes).toHaveLength(2);
    expect(changes[0]).toMatchObject({
      path,
      text: first,
      model: { tag: 'hz-plan-badge', className: 'HzPlanBadge', version: sha256(first) },
    });
    expect(changes[0]).toHaveProperty('highlight');
    expect(changes[1]).toMatchObject({ path, text: second, model: { version: sha256(second) } });
  });
});

// covers: FCT-0042
test('coalesces a burst of writes into the last version read', async () => {
  const drafts = [withLabel('Brouillon 1'), withLabel('Brouillon 2'), withLabel('Brouillon 3')];
  const settled = withLabel('Final');

  await withStudio(userData, async (studio) => {
    await openProject(studio, kit);
    const path = await listedPath(studio.window, 'hz-plan-badge');
    await openComponent(studio.window, path);
    await startProbe(studio.window);

    for (const draft of drafts) await writeBadge(draft);
    await waitForChangeCount(studio.window, 1);
    await writeBadge(settled);
    await waitForChangedText(studio.window, settled);

    const changes = await changesSeen(studio.window);
    expect(changes.map((change) => (change as { text?: unknown }).text)).toEqual([drafts[2], settled]);
    expect(changes[0]).toMatchObject({ path, model: { version: sha256(drafts[2] ?? '') } });
  });
});

// covers: FCT-0042
test('ignores files outside the components folder', async () => {
  const sentinel = withLabel('Sentinelle');

  await withStudio(userData, async (studio) => {
    await openProject(studio, kit);
    const path = await listedPath(studio.window, 'hz-plan-badge');
    await openComponent(studio.window, path);
    await startProbe(studio.window);

    const outside = join(kit, OUTSIDE_COMPONENT_FILE);
    await writeFile(outside, `${await readFile(outside, 'utf8')}\n// edited\n`, 'utf8');
    await writeFile(join(kit, 'README.md'), '# Kit Horizon\n', 'utf8');
    await writeBadge(sentinel);
    await waitForChangedText(studio.window, sentinel);

    const changes = await changesSeen(studio.window);
    expect(changes).toHaveLength(1);
    expect(changes[0]).toMatchObject({ path, text: sentinel });
  });
});

// covers: FCT-0043
test('keeps the model of the last readable version and reports the syntax error of the new one', async () => {
  const broken = readable.replace(LABEL_LINE, '@property() label = (');
  const repaired = withLabel('Réparé');
  expect(broken).not.toBe(readable);

  await withStudio(userData, async (studio) => {
    await openProject(studio, kit);
    const path = await listedPath(studio.window, 'hz-plan-badge');
    await openComponent(studio.window, path);
    await startProbe(studio.window);

    await writeBadge(broken);
    await waitForChangedText(studio.window, broken);
    await writeBadge(repaired);
    await waitForChangedText(studio.window, repaired);

    const [onBroken, onRepaired] = await changesSeen(studio.window);
    expect(onBroken).toMatchObject({
      path,
      text: broken,
      model: { tag: 'hz-plan-badge', version: sha256(readable) },
      diagnostics: expect.arrayContaining([
        expect.objectContaining({ severity: 'error', code: expect.stringMatching(/^ts\(\d+\)$/) }),
      ]),
    });
    expect(onBroken).toHaveProperty('highlight');
    expect(onRepaired).toMatchObject({ path, text: repaired, model: { version: sha256(repaired) } });
    const repairedModel = (onRepaired as { model: { diagnostics: { severity: string }[] } }).model;
    expect(repairedModel.diagnostics.filter(({ severity }) => severity === 'error')).toEqual([]);
  });
});

test('restarts the analysis process when it stops and announces it', async () => {
  const afterRestart = withLabel('Relancé');

  await withStudio(userData, async (studio) => {
    await openProject(studio, kit);
    const path = await listedPath(studio.window, 'hz-plan-badge');
    await openComponent(studio.window, path);
    await startProbe(studio.window);
    const before = await analysisProcessIds(studio.app);
    expect(before.length).toBeGreaterThan(0);

    for (const pid of before) process.kill(pid);

    await expect
      .poll(async () => {
        const statuses = await statusesSeen(studio.window);
        const restarting = statuses.indexOf('restarting');
        return restarting >= 0 && statuses.indexOf('ready', restarting) > restarting;
      }, { timeout: 15_000 })
      .toBe(true);
    const after = await analysisProcessIds(studio.app);
    expect(after.length).toBeGreaterThan(0);
    expect(after.filter((pid) => before.includes(pid))).toEqual([]);

    const featureList = await listedPath(studio.window, 'hz-feature-list');
    expect(await openComponent(studio.window, featureList)).toMatchObject({
      path: featureList,
      model: { tag: 'hz-feature-list' },
    });
    await openComponent(studio.window, path);
    await writeBadge(afterRestart);
    await waitForChangedText(studio.window, afterRestart);
    const changes = await changesSeen(studio.window);
    expect(changes.at(-1)).toMatchObject({ path, model: { version: sha256(afterRestart) } });
  });
});
