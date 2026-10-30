import { readFile, stat } from 'node:fs/promises';
import path from 'node:path';
import { openPage } from './browser.mjs';
import { startBudget } from './deadline.mjs';
import { bundleLitPackage, maskPaths } from './lit-bundle.mjs';
import { ServiceError } from './protocol.mjs';

const THEMES = ['light', 'dark'];
const SHAPES = ['round', 'squircle', 'bevel'];
const FORMATS = ['jpeg', 'png'];
const BUTTONS = ['left', 'middle', 'right'];
const MODIFIERS = new Map([
  ['shift', 'Shift'],
  ['ctrl', 'Control'],
  ['alt', 'Alt'],
]);
const KEYS = new Set(['Enter', 'Tab', 'Escape', 'Backspace', 'Delete', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Home', 'End', 'PageUp', 'PageDown', 'Space']);
const CONTROL_CHARACTER = /\p{Cc}/u;
const WIDTH_RANGE = [320, 3840];
const HEIGHT_RANGE = [240, 2160];
const SCALE_RANGE = [1, 2];
const MAX_TAGS = 256;
const MAX_TAG_LENGTH = 128;
const MAX_EVENTS = 64;
const MAX_TEXT = 256;
const MAX_WHEEL = 4000;
const MAX_TOKENS_BYTES = 512 * 1024;
const MAX_OVERRIDES_BYTES = 256 * 1024;
const MAX_TEMPLATE_BYTES = 2 * 1024 * 1024;
const MAX_IMAGE_BYTES = 8 * 1024 * 1024;
const MAX_PAGE_ERRORS = 16;
const MAX_PAGE_ERROR_LENGTH = 256;
const OPEN_BUDGET_MS = 40_000;
const FRAME_WAIT_MS = 40;
const JPEG_QUALITY = 85;
const SIGNAL_QUALITY = 30;
const ACTION_TIMEOUT_MS = 5_000;
const PAGE_URL = 'https://uigen.invalid/playground';
const CRASHED = 'the playground page crashed';

const GUARD = `(() => {
  const defined = [];
  const define = customElements.define.bind(customElements);
  customElements.define = (name, ...rest) => {
    define(name, ...rest);
    defined.push(name);
  };
  window.__uigenDefined = defined;
  window.open = () => null;
  document.addEventListener('click', (event) => {
    const link = event.composedPath().find((node) => node instanceof HTMLAnchorElement || node instanceof HTMLAreaElement || node instanceof SVGAElement);
    if (link !== undefined && !(link.getAttribute('href') ?? link.getAttribute('xlink:href') ?? '').startsWith('#')) {
      event.preventDefault();
    }
  }, true);
  window.navigation?.addEventListener('navigate', (event) => {
    if (event.cancelable && !event.destination.sameDocument) {
      event.preventDefault();
    }
  });
  document.addEventListener('submit', (event) => event.preventDefault(), true);
})();`;

const pageHtml = (theme) => `<!doctype html>
<html data-theme="${theme}"><head><meta charset="utf-8">
<style id="uigen-page">html { background: Canvas; color: CanvasText; } :root[data-theme="light"] { color-scheme: light; } :root[data-theme="dark"] { color-scheme: dark; }</style>
<style id="uigen-tokens"></style><style id="uigen-overrides"></style>
<script>${GUARD}</script></head><body></body></html>`;

const invalid = (message) => new ServiceError('invalidRequest', message);
const crashed = () => new ServiceError('render', CRASHED);

let session = null;
let tail = Promise.resolve();

const serial = (work) => {
  const run = tail.then(work, work);
  tail = run.catch(() => undefined);
  return run;
};

const isObject = (value) => typeof value === 'object' && value !== null && !Array.isArray(value);
const inRange = (value, [min, max]) => typeof value === 'number' && Number.isFinite(value) && value >= min && value <= max;
const firstLine = (error, fallback) => (error instanceof Error ? maskPaths(error.message.split('\n')[0]) : fallback);

function checkDir(dir) {
  if (typeof dir !== 'string' || dir === '' || !path.isAbsolute(dir)) {
    throw invalid('"dir" must be an absolute path');
  }
  return path.join(dir, '.uigen', 'playground');
}

function checkWindow({ width, height, scale }) {
  if (!inRange(width, WIDTH_RANGE) || !inRange(height, HEIGHT_RANGE) || !inRange(scale, SCALE_RANGE)) {
    throw invalid(`"width" must be ${WIDTH_RANGE.join(' to ')}, "height" ${HEIGHT_RANGE.join(' to ')} and "scale" ${SCALE_RANGE.join(' to ')}`);
  }
}

function checkTheme(theme) {
  if (!THEMES.includes(theme)) {
    throw invalid(`"theme" must be one of ${THEMES.join(', ')}`);
  }
}

function checkTags(tags) {
  if (!Array.isArray(tags) || tags.length > MAX_TAGS || tags.some((tag) => typeof tag !== 'string' || tag === '' || tag.length > MAX_TAG_LENGTH)) {
    throw invalid(`"tags" must be at most ${MAX_TAGS} non-empty strings`);
  }
}

function checkText(name, value, maxBytes) {
  if (typeof value !== 'string' || Buffer.byteLength(value) > maxBytes) {
    throw invalid(`"${name}" must be a string of at most ${maxBytes / 1024} KiB`);
  }
}

async function readStaged(file, maxBytes) {
  try {
    if ((await stat(file)).size > maxBytes) {
      throw new ServiceError('tooLarge', `${path.basename(file)} is larger than ${maxBytes / 1024} KiB`);
    }
    return await readFile(file, 'utf8');
  } catch (error) {
    if (error instanceof ServiceError) {
      throw error;
    }
    if (error?.code === 'ENOENT') {
      return '';
    }
    throw new ServiceError('render', `cannot read ${path.basename(file)}: ${error?.code ?? 'unknown error'}`);
  }
}

const readTemplate = (staging) => readStaged(path.join(staging, 'template.html'), MAX_TEMPLATE_BYTES);

const installContent = ({ tokens, template }) => {
  document.getElementById('uigen-tokens').textContent = tokens;
  document.body.innerHTML = template;
};

const replaceBody = (template) => {
  document.body.innerHTML = template;
};

const applyShape = (shape) => {
  window.__uigenShape ??= (() => {
    const sheet = new CSSStyleSheet();
    const options = { childList: true, subtree: true };
    const observer = new MutationObserver((records) => {
      for (const { addedNodes } of records) {
        addedNodes.forEach((node) => node instanceof Element && adoptTree(node));
      }
    });
    const adopt = (element) => {
      const shadow = element.shadowRoot;
      if (shadow !== null) {
        if (!shadow.adoptedStyleSheets.includes(sheet)) {
          shadow.adoptedStyleSheets = [...shadow.adoptedStyleSheets, sheet];
        }
        observer.observe(shadow, options);
        adoptTree(shadow);
      }
    };
    const adoptTree = (root) => {
      if (root instanceof Element) {
        adopt(root);
      }
      root.querySelectorAll('*').forEach(adopt);
    };
    return { sheet, options, observer, adoptTree };
  })();
  const { sheet, options, observer, adoptTree } = window.__uigenShape;
  sheet.replaceSync(shape === 'round' ? '' : `:host, * { corner-shape: ${shape}; }`);
  if (shape === 'round') {
    observer.disconnect();
  } else {
    observer.observe(document, options);
    adoptTree(document);
  }
};

const applyPage = ({ tokens, overrides, theme }) => {
  if (tokens !== undefined) {
    document.getElementById('uigen-tokens').textContent = tokens;
  }
  if (overrides !== undefined) {
    document.getElementById('uigen-overrides').textContent = overrides;
  }
  if (theme !== undefined) {
    document.documentElement.dataset.theme = theme;
  }
};

const probePage = (tags) => ({
  defined: [...window.__uigenDefined],
  missing: tags.filter((tag) => customElements.get(tag) === undefined),
  shape: CSS.supports('corner-shape', 'squircle'),
});

const startScreencast = (current) =>
  current.cdp.send('Page.startScreencast', { format: 'jpeg', quality: SIGNAL_QUALITY, maxWidth: current.width, maxHeight: current.height, everyNthFrame: 1 });

function createSession({ page, close, cdp }, { width, height, scale, theme }) {
  const current = { page, close, cdp, width, height, scale, theme, shape: 'round', seq: 0, fresh: false, signal: null, pendingAcks: [], refused: null, crashed: false, waiters: [] };
  current.wake = () => current.waiters.splice(0).forEach((resolve) => resolve());
  current.markChanged = () => {
    current.seq += 1;
    current.wake();
  };
  page.setDefaultTimeout(ACTION_TIMEOUT_MS);
  current.crash = new Promise((resolve) => {
    page.once('crash', () => {
      current.crashed = true;
      current.wake();
      void current.close();
      resolve();
    });
  });
  page.on('popup', (popup) => void popup.close());
  page.on('dialog', (dialog) => void dialog.dismiss());
  cdp.on('Page.screencastFrame', ({ data, sessionId }) => {
    current.pendingAcks.push(sessionId);
    if (data !== current.signal) {
      current.signal = data;
      if (!current.fresh) {
        current.fresh = true;
        current.seq += 1;
      }
      current.wake();
    }
  });
  return current;
}

const applyWindow = ({ cdp, width, height, scale }) =>
  cdp.send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: scale, mobile: false });

async function closeSession() {
  const previous = session;
  session = null;
  await previous?.close();
}

const abandoned = () => new ServiceError('render', 'opening the playground was abandoned');

async function start(params, attempt) {
  const staging = checkDir(params.dir);
  const { code } = await bundleLitPackage({ libDir: path.join(staging, 'lib') });
  const tokens = await readStaged(path.join(staging, 'lib', 'tokens', 'tokens.css'), MAX_TOKENS_BYTES);
  const template = await readTemplate(staging);
  if (attempt.cancelled) {
    throw abandoned();
  }
  const opened = await openPage({ viewport: { width: params.width, height: params.height }, deviceScaleFactor: params.scale, colorScheme: params.theme });
  attempt.opened = opened;
  try {
    if (attempt.cancelled) {
      throw abandoned();
    }
    const { page } = opened;
    const current = createSession({ ...opened, cdp: await page.context().newCDPSession(page) }, params);
    const errors = [];
    page.on('pageerror', (error) => {
      if (errors.length < MAX_PAGE_ERRORS) {
        errors.push(error.message.slice(0, MAX_PAGE_ERROR_LENGTH));
      }
    });
    await applyWindow(current);
    await page.route(PAGE_URL, (route) => route.fulfill({ contentType: 'text/html', body: pageHtml(params.theme) }));
    await page.goto(PAGE_URL);
    await page.evaluate(installContent, { tokens, template });
    await page.addScriptTag({ content: code });
    const probe = await page.evaluate(probePage, params.tags);
    await startScreencast(current);
    if (attempt.cancelled) {
      throw abandoned();
    }
    session = current;
    return { ...probe, errors };
  } catch (error) {
    await opened.close();
    throw error;
  }
}

export function open(params, { budgetMs = OPEN_BUDGET_MS } = {}) {
  return serial(async () => {
    checkDir(params.dir);
    checkWindow(params);
    checkTheme(params.theme);
    checkTags(params.tags);
    await closeSession();
    const budget = startBudget(budgetMs, `opening the playground did not finish within ${budgetMs / 1000} s`);
    const attempt = { cancelled: false, opened: null };
    try {
      return await budget.bound(start(params, attempt));
    } catch (error) {
      attempt.cancelled = true;
      await attempt.opened?.close();
      throw error instanceof ServiceError ? error : new ServiceError('render', firstLine(error, 'the playground page could not be opened'));
    }
  });
}

function live() {
  if (session === null) {
    throw invalid('no playground is open');
  }
  if (session.crashed) {
    throw crashed();
  }
  return session;
}

async function within(work) {
  const current = live();
  try {
    return await Promise.race([
      work(current),
      current.crash.then(() => {
        throw crashed();
      }),
    ]);
  } catch (error) {
    if (current.crashed) {
      throw crashed();
    }
    throw error instanceof ServiceError ? error : new ServiceError('render', firstLine(error, 'the playground page failed'));
  }
}

export function setTemplate(params) {
  return serial(() => {
    const staging = checkDir(params.dir);
    return within(async (current) => {
      await current.page.evaluate(replaceBody, await readTemplate(staging));
      if (current.shape !== 'round') {
        await current.page.evaluate(applyShape, current.shape);
      }
      current.markChanged();
      return {};
    });
  });
}

export function apply({ tokens, overrides, theme, shape }) {
  return serial(() => {
    if (tokens !== undefined) {
      checkText('tokens', tokens, MAX_TOKENS_BYTES);
    }
    if (overrides !== undefined) {
      checkText('overrides', overrides, MAX_OVERRIDES_BYTES);
    }
    if (theme !== undefined) {
      checkTheme(theme);
    }
    if (shape !== undefined && !SHAPES.includes(shape)) {
      throw invalid(`"shape" must be one of ${SHAPES.join(', ')}`);
    }
    return within(async (current) => {
      if (theme !== undefined && theme !== current.theme) {
        await current.page.emulateMedia({ colorScheme: theme });
        current.theme = theme;
      }
      await current.page.evaluate(applyPage, { tokens, overrides, theme });
      if (shape !== undefined && shape !== current.shape) {
        await current.page.evaluate(applyShape, shape);
        current.shape = shape;
      }
      current.markChanged();
      return {};
    });
  });
}

export function resize(params) {
  return serial(() => {
    checkWindow(params);
    return within(async (current) => {
      const { width, height, scale } = params;
      await current.page.setViewportSize({ width, height });
      Object.assign(current, { width, height, scale, signal: null, pendingAcks: [] });
      await applyWindow(current);
      await current.cdp.send('Page.stopScreencast');
      await startScreencast(current);
      return {};
    });
  });
}

const waitForFrame = (current) =>
  new Promise((resolve) => {
    const timer = setTimeout(resolve, FRAME_WAIT_MS);
    current.waiters.push(() => {
      clearTimeout(timer);
      resolve();
    });
  });

const imageRefusal = (current, key) => (current.refused?.key === key ? current.refused.error : null);

async function acknowledge(current) {
  for (const sessionId of current.pendingAcks.splice(0)) {
    await current.cdp.send('Page.screencastFrameAck', { sessionId });
  }
}

async function encode(current, format) {
  const { data } = await current.cdp.send('Page.captureScreenshot', format === 'png' ? { format } : { format, quality: JPEG_QUALITY });
  if (data.length * 0.75 > MAX_IMAGE_BYTES) {
    throw new ServiceError('tooLarge', 'the image is larger than 8 MiB');
  }
  return { format, width: Math.round(current.width * current.scale), height: Math.round(current.height * current.scale), data };
}

export function frame({ after, format = 'jpeg' }) {
  return serial(() => {
    if (!Number.isSafeInteger(after) || after < 0) {
      throw invalid('"after" must be an integer >= 0');
    }
    if (!FORMATS.includes(format)) {
      throw invalid(`"format" must be one of ${FORMATS.join(', ')}`);
    }
    return within(async (current) => {
      if (current.seq <= after) {
        await acknowledge(current);
        await waitForFrame(current);
        live();
      }
      if (current.seq <= after) {
        return { seq: current.seq, changed: false };
      }
      const { seq } = current;
      current.fresh = false;
      const key = `${seq} ${format}`;
      try {
        const refusal = imageRefusal(current, key);
        if (refusal !== null) {
          throw refusal;
        }
        return { seq, changed: true, ...(await encode(current, format)) };
      } catch (error) {
        if (error instanceof ServiceError && error.kind === 'tooLarge') {
          current.refused = { key, error };
        }
        throw error;
      } finally {
        await acknowledge(current);
      }
    });
  });
}

function checkEvent(event, { width, height }) {
  if (!isObject(event)) {
    throw invalid('every event must be a JSON object');
  }
  const onWindow = () => {
    if (!inRange(event.x, [0, width]) || !inRange(event.y, [0, height])) {
      throw invalid(`"x" and "y" must be inside the ${width} x ${height} window`);
    }
  };
  switch (event.type) {
    case 'move':
      onWindow();
      break;
    case 'down':
    case 'up':
      onWindow();
      if (!BUTTONS.includes(event.button)) {
        throw invalid(`"button" must be one of ${BUTTONS.join(', ')}`);
      }
      break;
    case 'wheel':
      onWindow();
      if (!inRange(event.dx, [-MAX_WHEEL, MAX_WHEEL]) || !inRange(event.dy, [-MAX_WHEEL, MAX_WHEEL])) {
        throw invalid(`"dx" and "dy" must be within ${MAX_WHEEL}`);
      }
      break;
    case 'key':
      if (!KEYS.has(event.key)) {
        throw invalid(`"key" must be one of ${[...KEYS].join(', ')}`);
      }
      if (event.mods !== undefined && (!Array.isArray(event.mods) || event.mods.some((mod) => !MODIFIERS.has(mod)))) {
        throw invalid(`"mods" must list only ${[...MODIFIERS.keys()].join(', ')}`);
      }
      break;
    case 'text':
      if (typeof event.text !== 'string' || event.text.length > MAX_TEXT || CONTROL_CHARACTER.test(event.text)) {
        throw invalid(`"text" must be at most ${MAX_TEXT} characters without control characters`);
      }
      break;
    default:
      throw invalid('unknown event type');
  }
}

async function relay({ mouse, keyboard }, event) {
  switch (event.type) {
    case 'move':
      return mouse.move(event.x, event.y);
    case 'down':
      await mouse.move(event.x, event.y);
      return mouse.down({ button: event.button });
    case 'up':
      await mouse.move(event.x, event.y);
      return mouse.up({ button: event.button });
    case 'wheel':
      await mouse.move(event.x, event.y);
      return mouse.wheel(event.dx, event.dy);
    case 'key':
      return keyboard.press([...(event.mods ?? []).map((mod) => MODIFIERS.get(mod)), event.key].join('+'));
    default:
      return keyboard.insertText(event.text);
  }
}

export function input({ events }) {
  return serial(() =>
    within(async (current) => {
      if (!Array.isArray(events) || events.length < 1 || events.length > MAX_EVENTS) {
        throw invalid(`"events" must hold 1 to ${MAX_EVENTS} events`);
      }
      events.forEach((event) => checkEvent(event, current));
      try {
        for (const event of events) {
          await relay(current.page, event);
        }
      } finally {
        current.markChanged();
      }
      return {};
    }),
  );
}

export function close() {
  return serial(async () => {
    await closeSession();
    return {};
  });
}
