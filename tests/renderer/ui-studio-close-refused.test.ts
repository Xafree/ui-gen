import { afterEach, describe, expect, it, vi } from 'vitest';
import type { UiGenApi } from '../../src/preload/ui-gen-api.js';
import {
  PROJECT_CLOSED,
  PROJECT_OPENED,
  type ProjectClosedDetail,
  type ProjectOpenedDetail,
} from '../../src/renderer/studio-events.js';
import { UiStudio } from '../../src/renderer/ui-studio.js';
import { kitHorizon, kitHorizonProject, listing, lumenUi, makeUiGenApi, type RecentsApi } from './home/home-fixtures.js';
import { deepQueryAll, removeShownToasts, shownToasts } from './home/shadow-dom.js';

type ProjectApi = UiGenApi['project'];

const REFUSED = {
  tone: 'danger',
  title: 'Action impossible',
  message: "La demande a été refusée. Rien n'a été modifié.",
};

const mounted: UiStudio[] = [];

async function mountStudio(close: ProjectApi['close']): Promise<UiStudio> {
  const list = vi.fn<RecentsApi['list']>(() => Promise.resolve(listing([kitHorizon(), lumenUi()])));
  const api = makeUiGenApi({ list });
  const studio = new UiStudio();
  studio.api = { ...api, project: { ...api.project, close } };
  document.body.append(studio);
  mounted.push(studio);
  await studio.updateComplete;
  return studio;
}

function shownHome(): Element | undefined {
  return deepQueryAll('ui-home')[0];
}

function shownShell(): Element | undefined {
  return deepQueryAll('ui-shell')[0];
}

async function openKitHorizon(): Promise<void> {
  await expect.poll(() => shownHome()).toBeDefined();
  const detail: ProjectOpenedDetail = { project: kitHorizonProject(), notice: null };
  shownHome()?.dispatchEvent(
    new CustomEvent<ProjectOpenedDetail>(PROJECT_OPENED, { detail, bubbles: true, composed: true }),
  );
  await expect.poll(() => shownShell()).toBeDefined();
}

function closeFromShell(): void {
  const detail: ProjectClosedDetail = { project: kitHorizonProject(), reason: 'close' };
  shownShell()?.dispatchEvent(
    new CustomEvent<ProjectClosedDetail>(PROJECT_CLOSED, { detail, bubbles: true, composed: true }),
  );
}

afterEach(() => {
  for (const studio of mounted.splice(0)) {
    studio.remove();
  }
  removeShownToasts();
});

describe('<ui-studio> when the api refuses to close the project', () => {
  // covers: FCT-0012
  it('keeps the project open and says the request was refused', async () => {
    const close = vi.fn<ProjectApi['close']>(() => Promise.reject(new Error('IPC channel closed')));
    await mountStudio(close);
    await openKitHorizon();

    closeFromShell();

    await expect.poll(() => shownToasts()).toContainEqual(REFUSED);
    expect(close).toHaveBeenCalledTimes(1);
    expect(shownShell()).toBeDefined();
    expect(shownHome()).toBeUndefined();
    expect(shownToasts()).toHaveLength(1);
  });

  // covers: FCT-0012
  it('goes back home on a new attempt that the api accepts and drops the earlier refusal', async () => {
    const close = vi
      .fn<ProjectApi['close']>()
      .mockRejectedValueOnce(new Error('IPC channel closed'))
      .mockResolvedValueOnce(undefined);
    await mountStudio(close);
    await openKitHorizon();
    closeFromShell();
    await expect.poll(() => shownToasts()).toContainEqual(REFUSED);

    closeFromShell();

    await expect.poll(() => shownHome()).toBeDefined();
    expect(close).toHaveBeenCalledTimes(2);
    expect(shownShell()).toBeUndefined();
    await expect.poll(() => shownToasts()).toEqual([
      {
        tone: 'info',
        title: 'Kit Horizon fermé',
        message: 'Il reste dans les projets récents.',
      },
    ]);
  });
});
