import { createHash } from 'node:crypto';
import { mkdir } from 'node:fs/promises';
import path from 'node:path';
import { loadBundle, withPage } from './browser.mjs';
import { IS_COMPONENT, cachedBundle } from './bundle.mjs';
import { startBudget } from './deadline.mjs';
import { insideRoot, readInstalledPackage } from './installed-package.mjs';
import { ServiceError } from './protocol.mjs';
import { captureSnapshot } from './snapshot.mjs';
import { findStylesheets } from './stylesheets.mjs';

const ROOT_SELECTOR = '[data-uigen-root]';
const MOUNT_GLOBAL = '__uigenMount';
const COMPONENT_MARK = 'data-uigen-component';
const COMPONENT_SELECTOR = `[${COMPONENT_MARK}]`;
const RENDERABLE = ['react', 'web-components'];
const STATES = ['default', 'hover', 'focus', 'pressed', 'disabled'];
const THEMES = ['light', 'dark'];
const VIEWPORT = { width: 800, height: 600 };
const RENDER_BUDGET_MS = 25_000;
const RENDER_TOTAL_MS = 28_000;
const MAX_SAMPLE_TEXT_CHARS = 256;
const SAMPLE_TEXT_IGNORED = 'sampleText ignored: the component takes no content';
const SETTLE_CAP_MS = 3_000;
const BACKGROUNDS = { light: '#ffffff', dark: '#1e1e1e' };

const invalid = (message) => new ServiceError('invalidRequest', message);

const htmlAttributes = (theme) => (theme === 'dark' ? 'class="dark" data-theme="dark"' : 'data-theme="light"');

const pageHtml = (theme) => `<!doctype html>
<html ${htmlAttributes(theme)}><head><meta charset="utf-8"><style>
html { background: ${BACKGROUNDS[theme]}; color-scheme: ${theme}; }
body { margin: 20px; font-family: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; }
[data-uigen-root] { display: inline-block; }
</style></head><body><div data-uigen-root></div></body></html>`;

const reactEntry = (name) => `import * as lib from ${JSON.stringify(name)};
import { createElement } from 'react';
import { flushSync } from 'react-dom';
import { createRoot } from 'react-dom/client';
const isComponent = ${IS_COMPONENT};
const fail = (error) => { window.__uigenError = String(error?.message ?? error); };
export function mount({ component, props, text }) {
  if (!Object.hasOwn(lib, component) || !isComponent(lib[component])) {
    window.__uigenMissing = true;
    return;
  }
  const root = createRoot(document.querySelector(${JSON.stringify(ROOT_SELECTOR)}), { onUncaughtError: fail, onCaughtError: fail });
  window.__uigenParts = Object.keys(lib[component]).filter((key) => key !== 'Root' && /^[A-Z]/.test(key) && isComponent(lib[component][key]));
  const content = text === undefined ? [] : [text];
  flushSync(() => root.render(createElement(lib[component], props, ...content)));
}`;

const elementEntry = (modulePath) => `import ${JSON.stringify(modulePath)};
export function mount({ component, props, text }) {
  const element = document.createElement(component);
  for (const [key, value] of Object.entries(props)) {
    if (key in element) { element[key] = value; } else { element.setAttribute(key, String(value)); }
  }
  if (text !== undefined) { element.append(document.createTextNode(text)); }
  document.querySelector(${JSON.stringify(ROOT_SELECTOR)}).append(element);
}`;

const mountScript = (code, input) => `(() => {
${code}
${MOUNT_GLOBAL}.mount(${JSON.stringify(input)});
})();`;

const safeSegment = (text) => text.replace(/[^A-Za-z0-9._-]+/g, '_').replace(/^\.+/, '_');

function checkParams({ component, props, state, theme, sampleText }) {
  if (sampleText !== undefined && (typeof sampleText !== 'string' || sampleText.length > MAX_SAMPLE_TEXT_CHARS)) {
    throw invalid(`"sampleText" must be a string of at most ${MAX_SAMPLE_TEXT_CHARS} characters`);
  }
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

function buildEntry(installed, component) {
  const modulePath = installed.tags.get(component);
  if (modulePath !== undefined) {
    const file = insideRoot(installed.root, modulePath);
    if (file === null) {
      throw new ServiceError('componentNotFound', `component "${component}" is declared outside ${installed.name}`);
    }
    return elementEntry(file);
  }
  if (installed.framework !== 'react') {
    throw new ServiceError('componentNotFound', `component "${component}" is not declared by ${installed.name}`);
  }
  return reactEntry(installed.name);
}

const installStamps = ({ root, sources }) => [path.join(root, 'package.json'), path.join(sources, 'package-lock.json'), path.join(sources, 'node_modules', '.package-lock.json')];

const settle = (page, budget) =>
  page.evaluate(async (capMs) => {
    const settled = (async () => {
      await document.fonts.ready;
      const finite = document.getAnimations().filter((animation) => animation.effect?.getComputedTiming().endTime !== Infinity);
      await Promise.all(finite.map((animation) => animation.finished.catch(() => undefined)));
    })();
    await Promise.race([settled, new Promise((resolve) => setTimeout(resolve, capMs))]);
  }, Math.min(SETTLE_CAP_MS, budget.remaining()));

async function forceState(page, state, budget) {
  const target = page.locator(`${ROOT_SELECTOR} > :not(style, script)`).first();
  if (state === 'hover') {
    await target.hover();
  } else if (state === 'pressed') {
    await target.hover();
    await page.mouse.down();
  } else if (state === 'focus') {
    await target.focus();
  }
  await settle(page, budget);
}

// The bootstrap div is not the component: a lone rendered element is measured and drawn on its own box.
function markRenderedElement({ rootSelector, mark }) {
  const IGNORED = new Set(['style', 'script', 'link', 'meta', 'template', 'noscript']);
  const loneChild = (parent) => {
    const hasText = [...parent.childNodes].some((node) => node.nodeType === Node.TEXT_NODE && node.textContent.trim() !== '');
    const rendered = [...parent.children].filter((child) => !IGNORED.has(child.localName) && getComputedStyle(child).display !== 'none');
    return !hasText && rendered.length === 1 ? rendered[0] : undefined;
  };
  let element = loneChild(document.querySelector(rootSelector));
  while (element !== undefined && getComputedStyle(element).display === 'contents') {
    element = loneChild(element);
  }
  const box = element?.getBoundingClientRect();
  if (box === undefined || box.width === 0 || box.height === 0) {
    return false;
  }
  element.setAttribute(mark, '');
  return true;
}

async function screenshotPainted(page, selector, file) {
  const target = page.locator(selector);
  const box = await target.boundingBox();
  const left = Math.round(box?.x ?? 0);
  const top = Math.round(box?.y ?? 0);
  const right = Math.round((box?.x ?? 0) + (box?.width ?? 0));
  const bottom = Math.round((box?.y ?? 0) + (box?.height ?? 0));
  if (right > left && bottom > top && right <= VIEWPORT.width && bottom <= VIEWPORT.height) {
    await page.screenshot({ path: file, clip: { x: left, y: top, width: right - left, height: bottom - top } });
  } else {
    await target.screenshot({ path: file });
  }
}

function pngSegments(installed, { component, props, state, theme }) {
  const hash = createHash('sha256').update(JSON.stringify(props)).digest('hex').slice(0, 12);
  const name = `${safeSegment(component)}-${hash}-${state}-${theme}.png`;
  return ['.uigen', 'captures', safeSegment(installed.name), safeSegment(installed.version), name];
}

async function renderInPage(page, installed, params, { code, styles }, budget) {
  const { component, props, state, theme } = params;
  page.setDefaultTimeout(Math.max(1, budget.remaining()));
  const errors = await loadBundle(page, { code, html: pageHtml(theme), styles });
  const failure = errors[0] ?? (await page.evaluate(() => window.__uigenError));
  if (failure !== undefined) {
    throw new ServiceError('render', failure);
  }
  if (await page.evaluate(() => window.__uigenMissing === true)) {
    throw new ServiceError('componentNotFound', `component "${component}" is not exported by ${installed.name}`);
  }
  await settle(page, budget);
  await forceState(page, state, budget);
  const source = { package: installed.name, version: installed.version, component, props, state };
  const captured = await page.evaluate(markRenderedElement, { rootSelector: ROOT_SELECTOR, mark: COMPONENT_MARK }) ? COMPONENT_SELECTOR : ROOT_SELECTOR;
  const snapshot = await captureSnapshot(page, captured, source);
  const segments = pngSegments(installed, { component, props, state, theme });
  const file = path.join(params.dir, ...segments);
  await mkdir(path.dirname(file), { recursive: true });
  await screenshotPainted(page, captured, file);
  const parts = await page.evaluate(() => window.__uigenParts ?? []);
  return { snapshot, png: segments.join('/'), ...(parts.length === 0 ? {} : { note: `rendered without its parts: ${parts.join(', ')}` }) };
}

export const retryBudgetMs = (spentMs) => Math.min(RENDER_BUDGET_MS, Math.max(0, RENDER_TOTAL_MS - spentMs));

export async function render(params) {
  const started = Date.now();
  const newBudget = (ms) => startBudget(ms, `rendering did not finish within ${RENDER_BUDGET_MS / 1000} s`);
  let budget = newBudget(RENDER_BUDGET_MS);
  const installed = await readInstalledPackage(params.dir, params.package);
  checkParams(params);
  if (!RENDERABLE.includes(installed.framework)) {
    throw new ServiceError('frameworkUnsupported', `${installed.framework} packages cannot be rendered yet (supported: ${RENDERABLE.join(', ')})`);
  }
  const props = params.state === 'disabled' ? { ...params.props, disabled: true, isDisabled: true } : params.props;
  const { styles, notes } = await budget.bound(findStylesheets(installed));
  const withNotes = (result, extra = []) => {
    const all = [...notes, ...(result.note === undefined ? [] : [result.note]), ...extra];
    return all.length === 0 ? result : { ...result, note: all.join('; ') };
  };
  const inPage = async (text) => {
    const code = mountScript(bundled, { component: params.component, props, text });
    try {
      return await budget.bound(
        withPage({ viewport: VIEWPORT, deviceScaleFactor: 1, colorScheme: params.theme }, (page) =>
          budget.bound(renderInPage(page, installed, params, { code, styles }, budget)),
        ),
      );
    } catch (error) {
      if (error instanceof ServiceError) {
        throw error;
      }
      throw new ServiceError('render', error instanceof Error ? error.message : 'the page could not be rendered');
    }
  };
  const bundled = await budget.bound(
    cachedBundle({ contents: buildEntry(installed, params.component), resolveDir: installed.sources, globalName: MOUNT_GLOBAL, stamps: installStamps(installed) }),
  );
  try {
    return withNotes(await inPage(params.sampleText));
  } catch (error) {
    const refusedText = params.sampleText !== undefined && error instanceof ServiceError && error.kind === 'render' && budget.remaining() > 0;
    if (!refusedText) {
      throw error;
    }
  }
  budget = newBudget(retryBudgetMs(Date.now() - started));
  return withNotes(await inPage(undefined), [SAMPLE_TEXT_IGNORED]);
}
