import { expect, test } from '@playwright/test';
import { closeStudio, launchStudio, makeTempFolder, removeFolder } from './project/studio-app';

test('opens the studio window isolated from Node, then closes it', async () => {
  const userData = await makeTempFolder('startup-user-data');
  const studio = await launchStudio(userData);
  try {
    const { window } = studio;
    await expect(window).toHaveTitle('Accueil, UI-GEN');
    await expect(window.locator('ui-studio')).toBeAttached();
    expect(await window.evaluate("customElements.get('ui-studio') !== undefined")).toBe(true);
    expect(await window.evaluate('typeof globalThis.require')).toBe('undefined');
    expect(await window.evaluate('typeof globalThis.process')).toBe('undefined');
    expect(await window.evaluate('typeof globalThis.uiGen.platform')).toBe('string');
  } finally {
    await closeStudio(studio.app);
    await removeFolder(userData);
  }
});
