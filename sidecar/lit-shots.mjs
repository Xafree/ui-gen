import { readFile, mkdir } from 'node:fs/promises';
import path from 'node:path';
import { closeBrowser, loadBundle, withPage } from './browser.mjs';
import { bundleLitPackage } from './lit-bundle.mjs';
import { BACKGROUNDS, VIEWPORT, pageHtml, screenshotPainted } from './render.mjs';

const MOUNT = '[data-uigen-root]';
const SHOWN = '[data-uigen-shown]';
const SETTLE_MS = 50;

export const shotName = (id) => `${id.replaceAll(/[^A-Za-z0-9]/g, '-')}.png`;

async function forcePseudo(cdp, pseudo) {
  const { root } = await cdp.send('DOM.getDocument', { depth: 0 });
  const { nodeId } = await cdp.send('DOM.querySelector', { nodeId: root.nodeId, selector: SHOWN });
  await cdp.send('CSS.forcePseudoState', { nodeId, forcedPseudoClasses: pseudo === null ? [] : [pseudo] });
}

function show({ mount, tag, attrs }) {
  const element = document.createElement(tag);
  for (const [name, value] of Object.entries(attrs)) {
    element.setAttribute(name, value);
  }
  element.setAttribute('data-uigen-shown', '');
  document.querySelector(mount).replaceChildren(element);
  return element.updateComplete.then(() => document.fonts.ready).then(() => undefined);
}

async function shootTheme({ code, tokens, tag, variants, theme, outDir }) {
  await withPage({ viewport: VIEWPORT, deviceScaleFactor: 1, colorScheme: theme }, async (page) => {
    const errors = await loadBundle(page, { code, html: pageHtml(theme), styles: [tokens] });
    if (errors.length > 0) {
      throw new Error(`the exported package failed to load: ${errors[0]}`);
    }
    const cdp = await page.context().newCDPSession(page);
    await cdp.send('DOM.enable');
    await cdp.send('CSS.enable');
    for (const { id, attrs, pseudo } of variants) {
      await page.evaluate(show, { mount: MOUNT, tag, attrs });
      await forcePseudo(cdp, pseudo);
      await page.waitForTimeout(SETTLE_MS);
      await screenshotPainted(page, SHOWN, path.join(outDir, theme, shotName(id)));
    }
  });
}

export async function shoot({ libDir, tag, variants, themes, outDir }) {
  const { code } = await bundleLitPackage({ libDir });
  const tokens = await readFile(path.join(libDir, 'tokens', 'tokens.css'), 'utf8');
  for (const theme of themes) {
    if (!Object.hasOwn(BACKGROUNDS, theme)) {
      throw new Error(`unknown theme ${theme}`);
    }
    await mkdir(path.join(outDir, theme), { recursive: true });
    await shootTheme({ code, tokens, tag, variants, theme, outDir });
  }
}

if (process.argv[1] === import.meta.filename) {
  try {
    await shoot(JSON.parse(await readFile(process.argv[2], 'utf8')));
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  } finally {
    await closeBrowser();
  }
}
