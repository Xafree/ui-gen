import { page } from 'vitest/browser';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ComponentChange } from '../../../src/modules/sync/application/components/component-views.js';
import type { UiGenApi } from '../../../src/preload/ui-gen-api.js';
import { UiShell } from '../../../src/renderer/shell/ui-shell.js';
import { kitHorizonProject, makeUiGenApi } from '../home/home-fixtures.js';
import { deepQueryAll } from '../home/shadow-dom.js';
import { openedComponent } from '../editor/canvas/fake-harness.js';
import { navLink, projectButton } from './shell-harness.js';

const COMPONENT_PATH = 'E:\\projets\\kit-horizon\\src\\components\\hz-card.ts';
const shells: UiShell[] = [];

interface ComponentsStub {
  readonly lastOpened?: () => Promise<string | null>;
  readonly list?: () => Promise<{ path: string; tag: string; name: string }[]>;
  readonly open?: (path: string) => Promise<ReturnType<typeof openedComponent>>;
}

function apiWith(stub: ComponentsStub, subscribe?: (listener: (change: ComponentChange) => void) => () => void): UiGenApi {
  const base = makeUiGenApi({});
  return {
    ...base,
    components: {
      ...base.components,
      ...stub,
      ...(subscribe === undefined ? {} : { onChanged: subscribe }),
    },
  };
}

async function mountShell(api: UiGenApi): Promise<UiShell> {
  const shell = new UiShell();
  shell.api = api;
  shell.project = kitHorizonProject();
  shell.notice = null;
  document.body.append(shell);
  shells.push(shell);
  await shell.updateComplete;
  await expect.element(projectButton()).toBeVisible();
  return shell;
}

function canvasFrame(): Element | undefined {
  return deepQueryAll('ui-canvas-frame')[0];
}

function emptyState(): Element | undefined {
  return deepQueryAll('#av-empty')[0];
}

beforeEach(() => {
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
  for (const shell of shells.splice(0)) shell.remove();
});

describe('the editor screen canvas', () => {
  it('shows the last opened component in the canvas instead of the pending empty state', async () => {
    const open = vi.fn((path: string) => Promise.resolve(openedComponent('v1', path)));

    await mountShell(apiWith({ lastOpened: () => Promise.resolve(COMPONENT_PATH), open }));

    await expect.poll(() => canvasFrame()).toBeDefined();
    expect(open).toHaveBeenCalledWith(COMPONENT_PATH);
    expect(emptyState()?.hasAttribute('hidden')).toBe(true);
    expect(canvasFrame()?.hasAttribute('hidden')).toBe(false);
    expect(Reflect.get(canvasFrame() ?? {}, 'projectPath')).toBe('E:\\projets\\kit-horizon');
  });

  it('falls back on the first listed component when none was opened before', async () => {
    const open = vi.fn((path: string) => Promise.resolve(openedComponent('v1', path)));
    const list = () => Promise.resolve([{ path: COMPONENT_PATH, tag: 'hz-card', name: 'Card' }]);

    await mountShell(apiWith({ lastOpened: () => Promise.resolve(null), list, open }));

    await expect.poll(() => canvasFrame()).toBeDefined();
    expect(open).toHaveBeenCalledWith(COMPONENT_PATH);
  });

  it('keeps the pending empty state when the library has no component', async () => {
    await mountShell(apiWith({ lastOpened: () => Promise.resolve(null), list: () => Promise.resolve([]) }));

    await expect.poll(() => emptyState()?.hasAttribute('hidden')).toBe(false);
    expect(canvasFrame()).toBeUndefined();
  });

  it('keeps the pending empty state, and says why, when the component cannot be opened', async () => {
    await mountShell(apiWith({ lastOpened: () => Promise.reject(new Error('refused')) }));

    await expect.poll(() => console.warn).toHaveBeenCalledWith(
      'The component to show in the editor could not be opened',
      expect.any(Error),
    );
    expect(canvasFrame()).toBeUndefined();
    expect(emptyState()?.hasAttribute('hidden')).toBe(false);
  });

  it('hides the canvas on the other screens and brings it back on the editor', async () => {
    await mountShell(
      apiWith({ lastOpened: () => Promise.resolve(COMPONENT_PATH), open: (path) => Promise.resolve(openedComponent('v1', path)) }),
    );
    await expect.poll(() => canvasFrame()).toBeDefined();

    await navLink('Playground').click();

    await expect.poll(() => canvasFrame()?.hasAttribute('hidden')).toBe(true);
    await expect.poll(() => emptyState()?.hasAttribute('hidden')).toBe(false);
    await navLink('Éditeur').click();
    await expect.poll(() => canvasFrame()?.hasAttribute('hidden')).toBe(false);
    await expect.poll(() => emptyState()?.hasAttribute('hidden')).toBe(true);
    await expect.element(page.getByRole('heading', { level: 1, name: 'Éditeur', exact: true })).toBeInTheDocument();
  });

  it('follows the changes of the shown component only, and stops following when removed', async () => {
    const feed: { listener: (change: ComponentChange) => void } = { listener: () => undefined };
    let stopped = false;
    await mountShell(
      apiWith(
        { lastOpened: () => Promise.resolve(COMPONENT_PATH), open: (path) => Promise.resolve(openedComponent('v1', path)) },
        (next) => {
          feed.listener = next;
          return () => {
            stopped = true;
          };
        },
      ),
    );
    await expect.poll(() => canvasFrame()).toBeDefined();
    const version = (): unknown => Reflect.get(Reflect.get(canvasFrame() ?? {}, 'component') as object, 'model');

    feed.listener({ ...openedComponent('v2', 'E:\\projets\\kit-horizon\\src\\components\\other.ts'), diagnostics: [] });
    feed.listener({ ...openedComponent('v3', COMPONENT_PATH), diagnostics: [] });

    await expect.poll(() => (version() as { version: string }).version).toBe('v3');
    shells[0]?.remove();
    expect(stopped).toBe(true);
  });
});
