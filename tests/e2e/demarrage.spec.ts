import { _electron as electron, expect, test } from '@playwright/test';

test('ouvre la fenêtre du studio isolée de Node puis la ferme', async () => {
  const application = await electron.launch({ args: ['.'] });
  try {
    const fenetre = await application.firstWindow();
    await expect(fenetre).toHaveTitle('UI-GEN');
    await expect(fenetre.locator('ui-studio')).toBeAttached();
    expect(await fenetre.evaluate("customElements.get('ui-studio') !== undefined")).toBe(true);
    expect(await fenetre.evaluate('typeof globalThis.require')).toBe('undefined');
    expect(await fenetre.evaluate('typeof globalThis.process')).toBe('undefined');
    expect(await fenetre.evaluate('typeof globalThis.uiGen.plateforme')).toBe('string');
  } finally {
    await application.close();
  }
});
