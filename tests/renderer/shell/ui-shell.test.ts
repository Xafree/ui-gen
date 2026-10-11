import { page, userEvent, type Locator } from 'vitest/browser';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ICON_PATHS, type IconName } from '../../../src/renderer/icons/icon-paths.js';
import { UiShell } from '../../../src/renderer/shell/ui-shell.js';
import { PROJECT_CLOSED } from '../../../src/renderer/studio-events.js';
import type { UiStudio } from '../../../src/renderer/ui-studio.js';
import { kitHorizonProject, makeUiGenApi } from '../home/home-fixtures.js';
import { deepActiveElement, deepQueryAll, removeShownToasts, shownToasts, visibleText } from '../home/shadow-dom.js';
import {
  PENDING_CHAT,
  emptyStateText,
  focusOn,
  isFocused,
  mountStudio,
  navLink,
  notificationRegions,
  openKitHorizon,
  projectButton,
  screensButton,
  shownHome,
  shownShell,
  studioApi,
  studioNav,
  type ProjectApi,
} from './shell-harness.js';

interface ExpectedScreen {
  readonly label: string;
  readonly short: string;
  readonly icon: IconName;
  readonly lot: number;
}

const EDITOR: ExpectedScreen = { label: 'Éditeur', short: 'Éditeur', icon: 'cadre', lot: 1 };
const PLAYGROUND: ExpectedScreen = { label: 'Playground', short: 'Playground', icon: 'play', lot: 3 };
const COMPONENT_IMPORT: ExpectedScreen = { label: 'Import de composants', short: 'Composants', icon: 'composants', lot: 7 };
const TOKEN_IMPORT: ExpectedScreen = { label: 'Import de tokens', short: 'Tokens', icon: 'tokens', lot: 2 };
const EXPORT: ExpectedScreen = { label: 'Export', short: 'Export', icon: 'package', lot: 5 };
const ALL_SCREENS = [EDITOR, PLAYGROUND, COMPONENT_IMPORT, TOKEN_IMPORT, EXPORT];

const shells: UiShell[] = [];
const studios: UiStudio[] = [];

async function mountShell(): Promise<UiShell> {
  const shell = new UiShell();
  shell.api = makeUiGenApi({});
  shell.project = kitHorizonProject();
  shell.notice = null;
  document.body.append(shell);
  shells.push(shell);
  await shell.updateComplete;
  await expect.element(projectButton()).toBeVisible();
  return shell;
}

function recordClosed(shell: UiShell): unknown[] {
  const details: unknown[] = [];
  shell.addEventListener(PROJECT_CLOSED, (event: Event) => {
    if (event instanceof CustomEvent) {
      const detail: unknown = event.detail;
      details.push(detail);
    }
  });
  return details;
}

function changeProjectItem(): Locator {
  return page.getByRole('button', { name: 'Changer de projet', exact: true });
}

function closeProjectItem(): Locator {
  return page.getByRole('button', { name: 'Fermer le projet', exact: true });
}

function screensMenuLink(label: string): Locator {
  return page.getByRole('link', { name: label, exact: true });
}

function menuItemIn(zone: string, selector: string, label: string): Element | undefined {
  return deepQueryAll(`${zone} ${selector}`).find((item) => visibleText(item) === label);
}

function screensMenuItem(label: string): Element | undefined {
  return menuItemIn('.ui-topbar__screens', 'a', label);
}

function projectMenuItem(label: string): Element | undefined {
  return menuItemIn('.ui-topbar__project-menu', 'button', label);
}

function isShownNow(element: Element | undefined): boolean {
  return element?.checkVisibility() ?? false;
}

function chatButton(): Locator {
  return page.getByRole('button', { name: /^Chat IA/ });
}

function exportLink(): Locator {
  return page.getByRole('link', { name: 'Exporter', exact: true });
}

function themeButton(): Locator {
  return page.getByRole('button', { name: /^Passer au thème (sombre|clair)$/ });
}

function mainStage(): Element | undefined {
  return deepQueryAll('main#main')[0];
}

function navLabels(): string[] {
  return studioNav()
    .getByRole('link')
    .elements()
    .map((link) => visibleText(link.querySelector('.ui-nav__long')));
}

function currentNavLabels(): string[] {
  return deepQueryAll('nav a[aria-current="page"] .ui-nav__long').map((label) => visibleText(label));
}

function firstPathOf(name: IconName): string | null {
  return /\bd="([^"]+)"/.exec(ICON_PATHS[name])?.[1] ?? null;
}

function emptyIconPath(): string | null {
  return deepQueryAll('#av-empty .ui-empty__icon svg path')[0]?.getAttribute('d') ?? null;
}

function isEditorShown(): boolean {
  return deepQueryAll('ui-editor').some((editor) => editor.checkVisibility());
}

async function expectEditorShown(): Promise<void> {
  await expect.poll(() => currentNavLabels()).toEqual([EDITOR.label]);
  await expect.poll(() => isEditorShown()).toBe(true);
  expect(isShownNow(deepQueryAll('#av-empty')[0])).toBe(false);
  expect(document.title).toBe(`${EDITOR.label}, UI-GEN`);
  expect(visibleText(deepQueryAll('.ui-topbar__screens > button')[0] ?? null)).toBe(`Écran actuel : ${EDITOR.short}`);
}

async function expectScreenShown(screen: ExpectedScreen): Promise<void> {
  if (screen === EDITOR) {
    await expectEditorShown();
    return;
  }
  await expect.poll(() => currentNavLabels()).toEqual([screen.label]);
  await expect.poll(() => emptyStateText()).toBe(`L'écran ${screen.label} arrive avec le lot ${String(screen.lot)}.`);
  await expect.element(page.getByRole('heading', { level: 1, name: screen.label, exact: true })).toBeInTheDocument();
  expect(visibleText(deepQueryAll('#av-heading')[0] ?? null)).toBe(screen.label);
  expect(deepQueryAll('#av-heading')[0]?.classList.contains('sr-only')).toBe(true);
  expect(document.title).toBe(`${screen.label}, UI-GEN`);
  expect(visibleText(deepQueryAll('.ui-topbar__screens > button')[0] ?? null)).toBe(`Écran actuel : ${screen.short}`);
  expect(emptyIconPath()).toBe(firstPathOf(screen.icon));
  expect(isEditorShown()).toBe(false);
}

afterEach(async () => {
  for (const shell of shells.splice(0)) {
    shell.remove();
  }
  for (const studio of studios.splice(0)) {
    studio.remove();
  }
  removeShownToasts();
  await page.viewport(1440, 900);
});

describe('<ui-shell> top bar', () => {
  // covers: FCT-0014
  it('shows the project, the five screens in order, then Chat IA, Exporter, the theme button and the avatar', async () => {
    await mountShell();

    expect(deepQueryAll('header.ui-topbar')).toHaveLength(1);
    const project = projectButton();
    expect(visibleText(project.element())).toContain('Kit Horizon');
    expect(visibleText(project.element())).toContain('@horizon/ui 0.4.0');
    await expect.element(project).toHaveAttribute('aria-haspopup', 'true');
    await expect.element(project).toHaveAttribute('aria-expanded', 'false');
    await expect.element(studioNav()).toBeVisible();
    expect(navLabels()).toEqual([
      'Éditeur',
      'Playground',
      'Import de composants',
      'Import de tokens',
      'Export',
    ]);
    await expect.element(chatButton()).toBeVisible();
    expect(chatButton().element().id).toBe('chat-toggle');
    expect(visibleText(chatButton().element())).toBe('Chat IA Ctrl I');
    await expect.element(exportLink()).toBeVisible();
    expect(exportLink().element().id).toBe('export-link');
    await expect.element(themeButton()).toBeVisible();
    await expect.element(page.getByRole('img', { name: 'Compte local' })).toBeVisible();
  });

  // covers: FCT-0014
  it('opens on the Éditeur, marked as the current page, showing the editor in place of a pending empty state', async () => {
    await mountShell();

    await expect.element(navLink('Éditeur')).toHaveAttribute('aria-current', 'page');
    for (const label of ['Playground', 'Import de composants', 'Import de tokens', 'Export']) {
      await expect.element(navLink(label)).not.toHaveAttribute('aria-current');
    }
    await expectScreenShown(EDITOR);
  });

  // covers: FCT-0015
  it('shows the stage of a screen not yet built with no panel, no status bar and no action', async () => {
    await mountShell();
    await navLink('Playground').click();
    await expectScreenShown(PLAYGROUND);

    const stage = mainStage();
    expect(stage?.classList.contains('ui-stage')).toBe(true);
    expect(stage?.classList.contains('av-stage')).toBe(true);
    expect(stage?.querySelectorAll('button, a, input, select, textarea')).toHaveLength(0);
    expect(stage?.querySelectorAll('.ui-panel, .ui-statusbar, .ui-empty__actions')).toHaveLength(0);
    expect(deepQueryAll('.ui-panel, .ui-statusbar').filter((element) => element.checkVisibility())).toHaveLength(0);
  });
});

describe('<ui-shell> screen navigation', () => {
  // covers: FCT-0015
  it.each(ALL_SCREENS.map((screen) => [screen.label, screen] as const))(
    'activates %s in place when its tab is clicked',
    async (label, screen) => {
      await mountShell();
      if (label === 'Éditeur') {
        await navLink('Playground').click();
        await expectScreenShown(PLAYGROUND);
      }

      await navLink(label).click();

      await expectScreenShown(screen);
      await expect.element(navLink(label)).toHaveAttribute('aria-current', 'page');
    },
  );

  // covers: FCT-0015
  it('leaves only one tab marked as the current page after several changes', async () => {
    await mountShell();

    await navLink('Import de tokens').click();
    await navLink('Export').click();
    await navLink('Playground').click();

    await expectScreenShown(PLAYGROUND);
    expect(deepQueryAll('nav a[aria-current="page"]')).toHaveLength(1);
  });

  // covers: FCT-0019
  it('activates a tab with Enter and another with Space', async () => {
    await mountShell();

    focusOn(navLink('Import de composants'));
    await userEvent.keyboard('{Enter}');
    await expectScreenShown(COMPONENT_IMPORT);

    focusOn(navLink('Import de tokens'));
    await userEvent.keyboard('[Space]');
    await expectScreenShown(TOKEN_IMPORT);
  });

  // covers: FCT-0015
  it('activates the Export screen from the Exporter link, by click, Enter or Space', async () => {
    await mountShell();

    await exportLink().click();
    await expectScreenShown(EXPORT);

    await navLink('Éditeur').click();
    await expectScreenShown(EDITOR);
    focusOn(exportLink());
    await userEvent.keyboard('{Enter}');
    await expectScreenShown(EXPORT);

    await navLink('Éditeur').click();
    await expectScreenShown(EDITOR);
    focusOn(exportLink());
    await userEvent.keyboard('[Space]');
    await expectScreenShown(EXPORT);
  });

  // covers: FCT-0015
  it('activates a screen from the Écrans menu of a narrow window, closes the menu and gives the focus back to its button', async () => {
    await page.viewport(600, 900);
    await mountShell();
    await expect.element(screensButton()).toBeVisible();

    await screensButton().click();
    await expect.element(page.getByText('Écrans du studio', { exact: true })).toBeVisible();
    await expect.element(screensMenuLink('Éditeur')).toHaveAttribute('aria-current', 'page');
    expect(isShownNow(screensMenuItem('Import de tokens'))).toBe(true);
    await screensMenuLink('Import de tokens').click();

    await expectScreenShown(TOKEN_IMPORT);
    await expect.poll(() => isShownNow(screensMenuItem('Import de tokens'))).toBe(false);
    await expect.poll(() => isFocused(screensButton())).toBe(true);
    await expect.element(screensButton()).toHaveAttribute('aria-expanded', 'false');
  });

  // covers: FCT-0019
  it('activates a screen from the Écrans menu with the keyboard and gives the focus back to its button', async () => {
    await page.viewport(600, 900);
    await mountShell();

    focusOn(screensButton());
    await userEvent.keyboard('{Enter}');
    await expect.element(screensMenuLink('Playground')).toBeVisible();
    expect(isShownNow(screensMenuItem('Playground'))).toBe(true);
    focusOn(screensMenuLink('Playground'));
    await userEvent.keyboard('{Enter}');

    await expectScreenShown(PLAYGROUND);
    await expect.poll(() => isShownNow(screensMenuItem('Playground'))).toBe(false);
    await expect.poll(() => isFocused(screensButton())).toBe(true);

    await screensButton().click();
    await expect.element(screensMenuLink('Playground')).toHaveAttribute('aria-current', 'page');
  });
});

describe('<ui-shell> project menu', () => {
  // covers: FCT-0012
  it('shows the project path and both leave actions when the project button is clicked', async () => {
    await mountShell();

    await projectButton().click();

    await expect.element(projectButton()).toHaveAttribute('aria-expanded', 'true');
    await expect.element(page.getByText('E:\\projets\\kit-horizon', { exact: true })).toBeVisible();
    await expect.element(changeProjectItem()).toBeVisible();
    await expect.element(closeProjectItem()).toBeVisible();
  });

  // covers: FCT-0012
  it('asks to switch project when « Changer de projet » is chosen', async () => {
    const shell = await mountShell();
    const closed = recordClosed(shell);

    await projectButton().click();
    await changeProjectItem().click();

    await expect.poll(() => closed).toEqual([{ project: kitHorizonProject(), reason: 'switch' }]);
  });

  // covers: FCT-0012
  it('asks to close the project when « Fermer le projet » is chosen', async () => {
    const shell = await mountShell();
    const closed = recordClosed(shell);

    await projectButton().click();
    await closeProjectItem().click();

    await expect.poll(() => closed).toEqual([{ project: kitHorizonProject(), reason: 'close' }]);
  });

  // covers: FCT-0019
  it('opens with Enter on the first item, loops with the arrows and closes with Escape back on its button', async () => {
    await mountShell();

    focusOn(projectButton());
    await userEvent.keyboard('{Enter}');
    await expect.element(projectButton()).toHaveAttribute('aria-expanded', 'true');
    await expect.poll(() => isFocused(changeProjectItem())).toBe(true);

    await userEvent.keyboard('{ArrowDown}');
    await expect.poll(() => isFocused(closeProjectItem())).toBe(true);
    await userEvent.keyboard('{ArrowDown}');
    await expect.poll(() => isFocused(changeProjectItem())).toBe(true);
    await userEvent.keyboard('{ArrowUp}');
    await expect.poll(() => isFocused(closeProjectItem())).toBe(true);
    await userEvent.keyboard('{ArrowUp}');
    await expect.poll(() => isFocused(changeProjectItem())).toBe(true);
    expect(isShownNow(projectMenuItem('Changer de projet'))).toBe(true);

    await userEvent.keyboard('{Escape}');
    await expect.element(projectButton()).toHaveAttribute('aria-expanded', 'false');
    await expect.poll(() => isShownNow(projectMenuItem('Changer de projet'))).toBe(false);
    await expect.poll(() => isFocused(projectButton())).toBe(true);
  });

  // covers: FCT-0019
  it('opens with Space on the first item', async () => {
    await mountShell();

    focusOn(projectButton());
    await userEvent.keyboard('[Space]');

    await expect.element(projectButton()).toHaveAttribute('aria-expanded', 'true');
    await expect.poll(() => isFocused(changeProjectItem())).toBe(true);
  });

  // covers: FCT-0019
  it('closes when Tab leaves the menu', async () => {
    await mountShell();
    focusOn(projectButton());
    await userEvent.keyboard('{Enter}');
    await expect.poll(() => isFocused(changeProjectItem())).toBe(true);
    await userEvent.keyboard('{ArrowDown}');
    await expect.poll(() => isFocused(closeProjectItem())).toBe(true);
    expect(isShownNow(projectMenuItem('Fermer le projet'))).toBe(true);

    await userEvent.tab();

    await expect.element(projectButton()).toHaveAttribute('aria-expanded', 'false');
    await expect.poll(() => isShownNow(projectMenuItem('Fermer le projet'))).toBe(false);
  });

  // covers: FCT-0019
  it('closes on a click outside the menu without leaving the project', async () => {
    const shell = await mountShell();
    const closed = recordClosed(shell);
    await navLink('Playground').click();
    await expectScreenShown(PLAYGROUND);
    await projectButton().click();
    await expect.element(changeProjectItem()).toBeVisible();
    expect(isShownNow(projectMenuItem('Changer de projet'))).toBe(true);

    await page.getByText('Disponible dans un prochain lot', { exact: true }).click();

    await expect.element(projectButton()).toHaveAttribute('aria-expanded', 'false');
    await expect.poll(() => isShownNow(projectMenuItem('Changer de projet'))).toBe(false);
    expect(closed).toEqual([]);
  });

  // covers: FCT-0019
  it('asks to switch project from the keyboard with Enter on « Changer de projet »', async () => {
    const shell = await mountShell();
    const closed = recordClosed(shell);
    focusOn(projectButton());
    await userEvent.keyboard('{Enter}');
    await expect.poll(() => isFocused(changeProjectItem())).toBe(true);

    await userEvent.keyboard('{Enter}');

    await expect.poll(() => closed).toEqual([{ project: kitHorizonProject(), reason: 'switch' }]);
  });

  // covers: FCT-0019
  it('asks to close the project from the keyboard with Space on « Fermer le projet »', async () => {
    const shell = await mountShell();
    const closed = recordClosed(shell);
    focusOn(projectButton());
    await userEvent.keyboard('{Enter}');
    await userEvent.keyboard('{ArrowDown}');
    await expect.poll(() => isFocused(closeProjectItem())).toBe(true);

    await userEvent.keyboard('[Space]');

    await expect.poll(() => closed).toEqual([{ project: kitHorizonProject(), reason: 'close' }]);
  });
});

describe('<ui-shell> keyboard walk', () => {
  // covers: FCT-0019
  it('walks the top bar with Tab, each element showing a visible focus', async () => {
    await mountShell();
    focusOn(projectButton());
    const expected = [
      navLink('Éditeur'),
      navLink('Playground'),
      navLink('Import de composants'),
      navLink('Import de tokens'),
      navLink('Export'),
      chatButton(),
      exportLink(),
      themeButton(),
    ];

    for (const target of expected) {
      await userEvent.tab();
      await expect.poll(() => isFocused(target)).toBe(true);
      expect(deepActiveElement()?.matches(':focus-visible')).toBe(true);
    }
  });
});

describe('<ui-shell> inside the studio', () => {
  // covers: FCT-0014
  it('arrives without any notice for a project opened without a question, using the single notification region of the studio', async () => {
    await mountStudio(studioApi(() => Promise.resolve()), studios);

    await openKitHorizon(null);

    expect(shownToasts()).toEqual([]);
    expect(notificationRegions()).toHaveLength(1);
    expect(document.title).toBe('Éditeur, UI-GEN');
  });

  // covers: FCT-0014
  it('shows the arrival notice it is given as a toast', async () => {
    await mountStudio(studioApi(() => Promise.resolve()), studios);
    const notice = {
      tone: 'success',
      title: 'Kit Horizon créé',
      message: 'Le projet est prêt dans E:\\projets\\kit-horizon.',
    } as const;

    await openKitHorizon(notice);

    await expect.poll(() => shownToasts()).toEqual([notice]);
    expect(notificationRegions()).toHaveLength(1);
  });

  // covers: FCT-0014
  it('says the Chat IA arrives with lot 6 when its button is clicked', async () => {
    await mountStudio(studioApi(() => Promise.resolve()), studios);
    await openKitHorizon(null);

    await chatButton().click();

    await expect.poll(() => shownToasts()).toEqual([PENDING_CHAT]);
    expect(notificationRegions()).toHaveLength(1);
  });

  // covers: FCT-0019
  it('says the Chat IA arrives with lot 6 on Ctrl+I', async () => {
    await mountStudio(studioApi(() => Promise.resolve()), studios);
    await openKitHorizon(null);
    focusOn(navLink('Playground'));

    await userEvent.keyboard('{Control>}i{/Control}');

    await expect.poll(() => shownToasts()).toEqual([PENDING_CHAT]);
  });

  // covers: FCT-0019
  it('ignores Ctrl+Alt+I and Ctrl+Shift+I', async () => {
    await mountStudio(studioApi(() => Promise.resolve()), studios);
    await openKitHorizon(null);
    focusOn(navLink('Playground'));

    await userEvent.keyboard('{Control>}{Alt>}i{/Alt}{/Control}');
    await userEvent.keyboard('{Control>}{Shift>}I{/Shift}{/Control}');
    await userEvent.keyboard('{Control>}i{/Control}');

    await expect.poll(() => shownToasts()).toEqual([PENDING_CHAT]);
  });

  // covers: FCT-0012
  it('stops answering Ctrl+I once the project is closed', async () => {
    const studio = await mountStudio(studioApi(() => Promise.resolve()), studios);
    await openKitHorizon(null);
    await projectButton().click();
    await changeProjectItem().click();
    await expect.poll(() => shownHome()).toBeDefined();
    expect(shownShell()).toBeUndefined();

    await userEvent.keyboard('{Control>}i{/Control}');
    await studio.updateComplete;

    expect(shownToasts()).toEqual([]);
  });

  // covers: FCT-0012
  it('goes back home and says the project stays in the recent list after « Fermer le projet »', async () => {
    const close = vi.fn<ProjectApi['close']>(() => Promise.resolve());
    await mountStudio(studioApi(close), studios);
    await openKitHorizon(null);

    await projectButton().click();
    await closeProjectItem().click();

    await expect.poll(() => shownHome()).toBeDefined();
    expect(shownShell()).toBeUndefined();
    expect(close).toHaveBeenCalledTimes(1);
    await expect.poll(() => shownToasts()).toEqual([
      { tone: 'info', title: 'Kit Horizon fermé', message: 'Il reste dans les projets récents.' },
    ]);
  });

  // covers: FCT-0012
  it('goes back home without any notice after « Changer de projet »', async () => {
    const close = vi.fn<ProjectApi['close']>(() => Promise.resolve());
    await mountStudio(studioApi(close), studios);
    await openKitHorizon(null);

    await projectButton().click();
    await changeProjectItem().click();

    await expect.poll(() => shownHome()).toBeDefined();
    expect(shownShell()).toBeUndefined();
    expect(close).toHaveBeenCalledTimes(1);
    await expect.element(page.getByRole('heading', { level: 2, name: 'Projets récents' })).toBeVisible();
    expect(shownToasts()).toEqual([]);
  });

  // covers: FCT-0012
  it('keeps the project open and gives the focus back to the project button when the close is refused', async () => {
    const close = vi.fn<ProjectApi['close']>(() => Promise.reject(new Error('IPC channel closed')));
    await mountStudio(studioApi(close), studios);
    await openKitHorizon(null);

    await projectButton().click();
    await closeProjectItem().click();

    await expect.poll(() => shownToasts()).toEqual([
      { tone: 'danger', title: 'Action impossible', message: "La demande a été refusée. Rien n'a été modifié." },
    ]);
    expect(shownShell()).toBeDefined();
    expect(shownHome()).toBeUndefined();
    await expect.poll(() => isFocused(projectButton())).toBe(true);
    await expect.element(projectButton()).toHaveAttribute('aria-expanded', 'false');
    expect(notificationRegions()).toHaveLength(1);
  });
});
