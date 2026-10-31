import { chromium } from 'playwright';

const LAUNCH_OPTIONS = {
  args: ['--disable-lcd-text', '--host-resolver-rules=MAP * ~NOTFOUND', '--force-webrtc-ip-handling-policy=disable_non_proxied_udp'],
  chromiumSandbox: true,
};
const LOCAL_SCHEMES = new Set(['data:', 'about:', 'blob:']);
const CLOSE_TIMEOUT_MS = 5_000;

let launching = null;

function launch() {
  const attempt = chromium.launch(LAUNCH_OPTIONS);
  attempt.catch(() => {
    if (launching === attempt) {
      launching = null;
    }
  });
  launching = attempt;
  return attempt;
}

async function ensureBrowser() {
  if (launching !== null) {
    const current = await launching.catch(() => null);
    if (current?.isConnected()) {
      return current;
    }
  }
  return launch();
}

const blockRemote = (route) => {
  const { protocol } = new URL(route.request().url());
  return LOCAL_SCHEMES.has(protocol) ? route.continue() : route.abort('blockedbyclient');
};

const closeWithin = (context) =>
  Promise.race([context.close(), new Promise((resolve) => setTimeout(resolve, CLOSE_TIMEOUT_MS).unref())]).catch(() => undefined);

export async function openPage(options) {
  const browser = await ensureBrowser();
  const context = await browser.newContext({ ...options, offline: true });
  try {
    await context.route('**/*', blockRemote);
    await context.routeWebSocket(/.*/, (socket) => socket.close());
    return { page: await context.newPage(), close: () => closeWithin(context) };
  } catch (error) {
    await closeWithin(context);
    throw error;
  }
}

export async function withPage(options, work) {
  const { page, close } = await openPage(options);
  try {
    return await work(page);
  } finally {
    await close();
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

export async function loadBundle(page, { code, html, styles = [] }) {
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.setContent(html);
  for (const content of styles) {
    await page.addStyleTag({ content });
  }
  await page.addScriptTag({ content: code });
  await page.evaluate(() => undefined);
  return errors;
}
