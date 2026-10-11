import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { UiShell } from '../../../src/renderer/shell/ui-shell.js';
import { FEATURE_LIST, fakeEditorApi } from '../editor/editor-fixtures.js';
import { kitHorizonProject } from '../home/home-fixtures.js';
import { deepQueryAll } from '../home/shadow-dom.js';
import { navLink } from './shell-harness.js';

const shells: UiShell[] = [];

async function mountShell(): Promise<UiShell> {
  const shell = new UiShell();
  shell.api = fakeEditorApi({}).api;
  shell.project = kitHorizonProject();
  shell.notice = null;
  document.body.append(shell);
  shells.push(shell);
  await shell.updateComplete;
  return shell;
}

function editor(): Element | undefined {
  return deepQueryAll('ui-editor')[0];
}

function isEditorShown(): boolean {
  return editor()?.checkVisibility() === true;
}

beforeEach(() => {
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
  for (const shell of shells.splice(0)) shell.remove();
});

describe('the editor screen in the shell', () => {
  it('gives the editor the api and the path of the opened project, in place of the pending stage', async () => {
    await mountShell();

    await expect.poll(() => isEditorShown()).toBe(true);
    expect(Reflect.get(editor() ?? {}, 'projectPath')).toBe('E:\\projets\\kit-horizon');
    expect(deepQueryAll('main#main')[0]?.checkVisibility()).toBe(false);
  });

  it('opens the starting component in the editor canvas', async () => {
    await mountShell();

    await expect.poll(() => deepQueryAll('ui-canvas-frame')).toHaveLength(1);
    const component: unknown = Reflect.get(deepQueryAll('ui-canvas-frame')[0] ?? {}, 'component');
    expect(component).toMatchObject({ path: FEATURE_LIST.path });
  });

  it('hides the editor on the other screens and brings it back on the editor', async () => {
    await mountShell();
    await expect.poll(() => isEditorShown()).toBe(true);

    await navLink('Playground').click();

    await expect.poll(() => isEditorShown()).toBe(false);
    expect(editor()?.hasAttribute('hidden')).toBe(true);
    expect(deepQueryAll('#av-empty')[0]?.checkVisibility()).toBe(true);
    await navLink('Éditeur').click();
    await expect.poll(() => isEditorShown()).toBe(true);
    expect(deepQueryAll('#av-empty')[0]?.checkVisibility()).toBe(false);
  });
});
