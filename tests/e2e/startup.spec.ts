import { _electron as electron, expect, test } from '@playwright/test';

test('opens the studio window isolated from Node, then closes it', async () => {
  const electronApp = await electron.launch({ args: ['.'] });
  try {
    const studioWindow = await electronApp.firstWindow();
    await expect(studioWindow).toHaveTitle('UI-GEN');
    await expect(studioWindow.locator('ui-studio')).toBeAttached();
    expect(await studioWindow.evaluate("customElements.get('ui-studio') !== undefined")).toBe(true);
    expect(await studioWindow.evaluate('typeof globalThis.require')).toBe('undefined');
    expect(await studioWindow.evaluate('typeof globalThis.process')).toBe('undefined');
    expect(await studioWindow.evaluate('typeof globalThis.uiGen.platform')).toBe('string');
  } finally {
    await electronApp.close();
  }
});
