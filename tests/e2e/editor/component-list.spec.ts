import { readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import { makeTempFolder, removeFolder } from '../project/studio-app';
import {
  FEATURE_LIST_FILE,
  PLAN_BADGE_FILE,
  PRICING_CARD_FILE,
  endsWithFile,
  lastOpened,
  listComponents,
  listedPath,
  openComponent,
  openProject,
  sha256,
  snapshotFolder,
  withStudio,
  writeEmptyLibrary,
  writeKitHorizon,
} from './editor-project';

let root: string;
let userData: string;

test.beforeEach(async () => {
  root = await makeTempFolder('editor-projects');
  userData = await makeTempFolder('user-data');
});

test.afterEach(async () => {
  await removeFolder(root);
  await removeFolder(userData);
});

// covers: FCT-0026
// covers: FCT-0028
test('lists the components of the components folder sorted by tag, subfolders included', async () => {
  const kit = join(root, 'kit-horizon');
  await writeKitHorizon(kit);

  await withStudio(userData, async (studio) => {
    await openProject(studio, kit);

    const listed = await listComponents(studio.window);

    expect(listed.map(({ tag }) => tag)).toEqual(['hz-feature-list', 'hz-plan-badge', 'hz-pricing-card']);
    expect(endsWithFile(listed[0]?.path ?? '', FEATURE_LIST_FILE)).toBe(true);
    expect(endsWithFile(listed[1]?.path ?? '', PLAN_BADGE_FILE)).toBe(true);
    expect(endsWithFile(listed[2]?.path ?? '', PRICING_CARD_FILE)).toBe(true);
    for (const { name } of listed) expect(name).not.toBe('');
  });
});

// covers: FCT-0027
test('lists no component and no last opened component for a library without components', async () => {
  const empty = join(root, 'empty-kit');
  await writeEmptyLibrary(empty);

  await withStudio(userData, async (studio) => {
    await openProject(studio, empty);

    expect(await listComponents(studio.window)).toEqual([]);
    expect(await lastOpened(studio.window)).toBeNull();
  });
});

// covers: FCT-0028
test('opens a listed component with its exact text, its model versioned by sha-256 and its highlighting', async () => {
  const kit = join(root, 'kit-horizon');
  await writeKitHorizon(kit);
  const text = await readFile(join(kit, PRICING_CARD_FILE), 'utf8');

  await withStudio(userData, async (studio) => {
    await openProject(studio, kit);
    const path = await listedPath(studio.window, 'hz-pricing-card');

    const opened = await openComponent(studio.window, path);

    expect(opened).toMatchObject({
      path,
      text,
      model: { tag: 'hz-pricing-card', className: 'HzPricingCard', version: sha256(text) },
    });
    expect(opened).toHaveProperty('highlight');
    const { highlight } = opened as { highlight: unknown };
    expect(Array.isArray(highlight) && highlight.length > 0).toBe(true);
    expect(highlight).toContainEqual(expect.objectContaining({ className: 'tk-kw' }));
  });
});

// covers: FCT-0026
test('reports no last opened component before any component is opened in the project', async () => {
  const kit = join(root, 'kit-horizon');
  await writeKitHorizon(kit);

  await withStudio(userData, async (studio) => {
    await openProject(studio, kit);

    expect(await lastOpened(studio.window)).toBeNull();
  });
});

// covers: FCT-0026
test('remembers the last opened component per project across restarts without writing into the project', async () => {
  const kit = join(root, 'kit-horizon');
  const other = join(root, 'kit-aurora');
  await writeKitHorizon(kit);
  await writeKitHorizon(other);
  const kitBefore = await snapshotFolder(kit);
  const otherBefore = await snapshotFolder(other);
  let pricingCard = '';
  let featureList = '';

  await withStudio(userData, async (studio) => {
    await openProject(studio, kit);
    await openComponent(studio.window, await listedPath(studio.window, 'hz-plan-badge'));
    pricingCard = await listedPath(studio.window, 'hz-pricing-card');
    await openComponent(studio.window, pricingCard);
    expect(await lastOpened(studio.window)).toBe(pricingCard);

    await openProject(studio, other);
    expect(await lastOpened(studio.window)).toBeNull();
    featureList = await listedPath(studio.window, 'hz-feature-list');
    await openComponent(studio.window, featureList);
    expect(await lastOpened(studio.window)).toBe(featureList);

    await openProject(studio, kit);
    expect(await lastOpened(studio.window)).toBe(pricingCard);
  });

  await withStudio(userData, async (studio) => {
    await openProject(studio, kit);
    expect(await lastOpened(studio.window)).toBe(pricingCard);
    await openProject(studio, other);
    expect(await lastOpened(studio.window)).toBe(featureList);
  });

  expect(await snapshotFolder(kit)).toEqual(kitBefore);
  expect(await snapshotFolder(other)).toEqual(otherBefore);
});

// covers: FCT-0043
test('opens a component whose syntax does not parse with its exact text and the syntax error', async () => {
  const kit = join(root, 'kit-horizon');
  await writeKitHorizon(kit);
  const readable = await readFile(join(kit, PLAN_BADGE_FILE), 'utf8');
  const broken = readable.replace("@property() label = ''", '@property() label = (');
  expect(broken).not.toBe(readable);
  await writeFile(join(kit, PLAN_BADGE_FILE), broken, 'utf8');

  await withStudio(userData, async (studio) => {
    await openProject(studio, kit);
    const path = await listedPath(studio.window, 'hz-plan-badge');

    const opened = await openComponent(studio.window, path);

    expect(opened).toMatchObject({ path, text: broken, model: { version: sha256(broken) } });
    expect(opened).toMatchObject({
      model: {
        diagnostics: expect.arrayContaining([
          expect.objectContaining({ severity: 'error', code: expect.stringMatching(/^ts\(\d+\)$/) }),
        ]),
      },
    });
  });
});
