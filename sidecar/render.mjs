import { createHash } from 'node:crypto';
import { mkdir } from 'node:fs/promises';
import path from 'node:path';
import { loadBundle, withPage } from './browser.mjs';
import { IS_COMPONENT, bundleEntry } from './bundle.mjs';
import { insideRoot, readInstalledPackage } from './installed-package.mjs';
import { ServiceError } from './protocol.mjs';
import { captureSnapshot } from './snapshot.mjs';

const ROOT_SELECTOR = '[data-uigen-root]';
const RENDERABLE = ['react', 'web-components'];
const STATES = ['default', 'hover', 'focus', 'pressed', 'disabled'];
const THEMES = ['light', 'dark'];
const VIEWPORT = { width: 800, height: 600 };
const RENDER_TIMEOUT_MS = 25_000;
const BACKGROUNDS = { light: '#ffffff', dark: '#1e1e1e' };

const invalid = (message) => new ServiceError('invalidRequest', message);

const pageHtml = (theme) => `<!doctype html>
<html><head><meta charset="utf-8"><style>
html { background: ${BACKGROUNDS[theme]}; color-scheme: ${theme}; }
body { margin: 20px; font-family: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; }
[data-uigen-root] { display: inline-block; }
</style></head><body><div data-uigen-root></div></body></html>`;

const reactEntry = (name, component, props) => `import * as lib from ${JSON.stringify(name)};
import { createElement } from 'react';
import { flushSync } from 'react-dom';
import { createRoot } from 'react-dom/client';
const isComponent = ${IS_COMPONENT};
const fail = (error) => { window.__uigenError = String(error?.message ?? error); };
if (!Object.hasOwn(lib, ${JSON.stringify(component)}) || !isComponent(lib[${JSON.stringify(component)}])) {
  window.__uigenMissing = true;
} else {
  const root = createRoot(document.querySelector(${JSON.stringify(ROOT_SELECTOR)}), { onUncaughtError: fail, onCaughtError: fail });
  flushSync(() => root.render(createElement(lib[${JSON.stringify(component)}], ${JSON.stringify(props)})));
}`;

const elementEntry = (modulePath, tag, props) => `import ${JSON.stringify(modulePath)};
const element = document.createElement(${JSON.stringify(tag)});
for (const [key, value] of Object.entries(${JSON.stringify(props)})) {
  if (key in element) { element[key] = value; } else { element.setAttribute(key, String(value)); }
}
document.querySelector(${JSON.stringify(ROOT_SELECTOR)}).append(element);`;

const safeSegment = (text) => text.replace(/[^A-Za-z0-9._-]+/g, '_').replace(/^\.+/, '_');

function checkParams({ component, props, state, theme }) {
  if (typeof component !== 'string' || component === '') {
    throw invalid('"component" must be a non-empty string');
  }
  if (typeof props !== 'object' || props === null || Array.isArray(props)) {
    throw invalid('"props" must be a JSON object');
  }
  if (!STATES.includes(state)) {
    throw invalid(`"state" must be one of ${STATES.join(', ')}`);
  }
  if (!THEMES.includes(theme)) {
    throw invalid(`"theme" must be one of ${THEMES.join(', ')}`);
  }
}

function buildEntry(installed, component, props) {
  const modulePath = installed.tags.get(component);
  if (modulePath !== undefined) {
    const file = insideRoot(installed.root, modulePath);
    if (file === null) {
      throw new ServiceError('componentNotFound', `component "${component}" is declared outside ${installed.name}`);
    }
    return elementEntry(file, component, props);
  }
  if (installed.framework !== 'react') {
    throw new ServiceError('componentNotFound', `component "${component}" is not declared by ${installed.name}`);
  }
  return reactEntry(installed.name, component, props);
}

const settle = (page) =>
  page.evaluate(async () => {
    await document.fonts.ready;
    const finite = document.getAnimations().filter((animation) => animation.effect?.getComputedTiming().endTime !== Infinity);
    await Promise.all(finite.map((animation) => animation.finished.catch(() => undefined)));
  });

async function forceState(page, state) {
  const target = page.locator(`${ROOT_SELECTOR} > :not(style, script)`).first();
  if (state === 'hover') {
    await target.hover();
  } else if (state === 'pressed') {
    await target.hover();
    await page.mouse.down();
  } else if (state === 'focus') {
    await target.focus();
  }
  await settle(page);
}

function pngSegments(installed, { component, props, state, theme }) {
  const hash = createHash('sha256').update(JSON.stringify(props)).digest('hex').slice(0, 12);
  const name = `${safeSegment(component)}-${hash}-${state}-${theme}.png`;
  return ['.uigen', 'captures', safeSegment(installed.name), safeSegment(installed.version), name];
}

async function renderInPage(page, installed, params, code) {
  const { component, props, state, theme } = params;
  page.setDefaultTimeout(RENDER_TIMEOUT_MS);
  const errors = await loadBundle(page, { code, html: pageHtml(theme) });
  const failure = errors[0] ?? (await page.evaluate(() => window.__uigenError));
  if (failure !== undefined) {
    throw new ServiceError('render', failure);
  }
  if (await page.evaluate(() => window.__uigenMissing === true)) {
    throw new ServiceError('componentNotFound', `component "${component}" is not exported by ${installed.name}`);
  }
  await settle(page);
  await forceState(page, state);
  const source = { package: installed.name, version: installed.version, component, props, state };
  const snapshot = await captureSnapshot(page, ROOT_SELECTOR, source);
  const segments = pngSegments(installed, { component, props, state, theme });
  const file = path.join(params.dir, ...segments);
  await mkdir(path.dirname(file), { recursive: true });
  await page.locator(ROOT_SELECTOR).screenshot({ path: file });
  return { snapshot, png: segments.join('/') };
}

export async function render(params) {
  const installed = await readInstalledPackage(params.dir, params.package);
  checkParams(params);
  if (!RENDERABLE.includes(installed.framework)) {
    throw new ServiceError('frameworkUnsupported', `${installed.framework} packages cannot be rendered yet (supported: ${RENDERABLE.join(', ')})`);
  }
  const props = params.state === 'disabled' ? { ...params.props, disabled: true } : params.props;
  const code = await bundleEntry({ contents: buildEntry(installed, params.component, props), resolveDir: installed.sources });
  try {
    return await withPage({ viewport: VIEWPORT, deviceScaleFactor: 1, colorScheme: params.theme }, (page) =>
      renderInPage(page, installed, params, code),
    );
  } catch (error) {
    if (error instanceof ServiceError) {
      throw error;
    }
    throw new ServiceError('render', error instanceof Error ? error.message : 'the page could not be rendered');
  }
}
