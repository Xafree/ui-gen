import { chromium } from 'playwright';

const LAUNCH_OPTIONS = { args: ['--disable-lcd-text'] };

let launching = null;

async function ensureBrowser() {
  if (launching !== null) {
    const current = await launching;
    if (current.isConnected()) {
      return current;
    }
  }
  launching = chromium.launch(LAUNCH_OPTIONS);
  return launching;
}

export async function withPage(options, work) {
  const browser = await ensureBrowser();
  const context = await browser.newContext(options);
  try {
    return await work(await context.newPage());
  } finally {
    await context.close().catch(() => undefined);
  }
}

export async function closeBrowser() {
  if (launching === null) {
    return;
  }
  const pending = launching;
  launching = null;
  const browser = await pending.catch(() => null);
  await browser?.close();
}

export async function loadBundle(page, { code, html }) {
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.setContent(html);
  await page.addScriptTag({ content: code });
  await page.evaluate(() => undefined);
  return errors;
}
