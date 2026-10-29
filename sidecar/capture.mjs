import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { build } from 'esbuild';
import { chromium } from 'playwright';
import { captureSnapshot } from './snapshot.mjs';

const here = import.meta.dirname;
const ROOT_SELECTOR = '[data-uigen-root]';
const VIEWPORT = { width: 400, height: 200 };
const TRANSITION_MARGIN_MS = 50;
const READY_TIMEOUT_MS = 15000;

const TAILWIND_SHADCN_THEME = `
@import "tailwindcss";
@theme inline {
  --color-background: var(--background);
  --color-foreground: var(--foreground);
  --color-primary: var(--primary);
  --color-primary-foreground: var(--primary-foreground);
  --color-secondary: var(--secondary);
  --color-secondary-foreground: var(--secondary-foreground);
  --color-accent: var(--accent);
  --color-accent-foreground: var(--accent-foreground);
  --color-destructive: var(--destructive);
  --color-ring: var(--ring);
  --radius-sm: calc(var(--radius) - 4px);
  --radius-md: calc(var(--radius) - 2px);
  --radius-lg: var(--radius);
}
:root {
  --radius: 0.625rem;
  --background: oklch(1 0 0);
  --foreground: oklch(0.145 0 0);
  --primary: oklch(0.205 0 0);
  --primary-foreground: oklch(0.985 0 0);
  --secondary: oklch(0.97 0 0);
  --secondary-foreground: oklch(0.205 0 0);
  --accent: oklch(0.97 0 0);
  --accent-foreground: oklch(0.205 0 0);
  --destructive: oklch(0.577 0.245 27.325);
  --ring: oklch(0.708 0 0);
}
`;

const LIBRARIES = {
  mui: { package: '@mui/material', props: { variant: 'contained' } },
  chakra: { package: '@chakra-ui/react', props: {} },
  shadcn: {
    package: 'shadcn/ui',
    version: 'n/a',
    props: { variant: 'default', size: 'default' },
    needs: ['@radix-ui/react-slot', 'class-variance-authority', '@tailwindcss/browser'],
  },
};
const STATES = ['default', 'hover'];

const usage = 'usage : node capture.mjs <mui|chakra|shadcn> <default|hover> <outDir>';

function parseArguments(argv) {
  const [lib, state, outDir, ...extra] = argv;
  if (!Object.hasOwn(LIBRARIES, lib) || !STATES.includes(state) || !outDir || extra.length > 0) {
    console.error(usage);
    process.exit(2);
  }
  return { lib, state, outDir: resolve(outDir) };
}

async function readPackageVersion(name) {
  const file = resolve(here, 'node_modules', name, 'package.json');
  if (!existsSync(file)) {
    throw new Error(`paquet absent : ${name} (attendu dans ${file}) ; lancer npm install dans sidecar/`);
  }
  return JSON.parse(await readFile(file, 'utf8')).version;
}

async function bundle(lib, buildDir) {
  await build({
    entryPoints: [resolve(here, 'fixtures-src', `${lib}.jsx`)],
    outfile: resolve(buildDir, `${lib}.js`),
    bundle: true,
    format: 'iife',
    platform: 'browser',
    jsx: 'automatic',
    minify: true,
    define: { 'process.env.NODE_ENV': '"production"' },
    logLevel: 'silent',
  });
}

function pageHtml(lib) {
  const tailwind =
    lib === 'shadcn'
      ? `<script src="${pathToFileURL(resolve(here, 'node_modules/@tailwindcss/browser/dist/index.global.js')).href}"></script>
<style type="text/tailwindcss">${TAILWIND_SHADCN_THEME}</style>`
      : '';
  return `<!doctype html>
<html lang="fr">
<head>
<meta charset="utf-8">
<title>${lib}</title>
${tailwind}
<style>
  html, body { background: #fff; }
  body { margin: 20px; font-family: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; }
</style>
</head>
<body>
<div id="mount"></div>
<script src="${lib}.js"></script>
</body>
</html>
`;
}

async function waitForRender(page, pageErrors) {
  try {
    await page.waitForFunction(
      (selector) => {
        const element = document.querySelector(selector);
        return element !== null && getComputedStyle(element).backgroundColor !== 'rgba(0, 0, 0, 0)';
      },
      ROOT_SELECTOR,
      { timeout: READY_TIMEOUT_MS },
    );
  } catch (error) {
    const details = pageErrors.length > 0 ? ` ; erreurs de page : ${pageErrors.join(' | ')}` : '';
    throw new Error(`rendu vide : aucun bouton ${ROOT_SELECTOR} stylé après ${READY_TIMEOUT_MS} ms${details}`, {
      cause: error,
    });
  }
  await page.evaluate(() => document.fonts.ready.then(() => undefined));
}

function settleTransitions(page) {
  return page.evaluate(
    async ([selector, margin, cap]) => {
      const seconds = (value) => value.split(',').map((part) => Number.parseFloat(part) || 0);
      const root = document.querySelector(selector);
      let longest = 0;
      for (const element of [root, ...root.querySelectorAll('*')]) {
        const { transitionDuration, transitionDelay } = getComputedStyle(element);
        const delays = seconds(transitionDelay);
        seconds(transitionDuration).forEach((duration, index) => {
          longest = Math.max(longest, duration + delays[index % delays.length]);
        });
      }
      await new Promise((done) => setTimeout(done, Math.min(longest * 1000 + margin, cap)));
    },
    [ROOT_SELECTOR, TRANSITION_MARGIN_MS, READY_TIMEOUT_MS],
  );
}

async function capture({ lib, state, outDir }) {
  const config = LIBRARIES[lib];
  const version = config.version ?? (await readPackageVersion(config.package));
  for (const name of config.needs ?? []) {
    await readPackageVersion(name);
  }

  const buildDir = resolve(here, 'out', 'build');
  await mkdir(buildDir, { recursive: true });
  await mkdir(outDir, { recursive: true });
  await bundle(lib, buildDir);
  const htmlPath = resolve(buildDir, `${lib}.html`);
  await writeFile(htmlPath, pageHtml(lib));

  const browser = await chromium.launch({ chromiumSandbox: true, args: ['--disable-lcd-text'] });
  try {
    const context = await browser.newContext({ viewport: VIEWPORT, deviceScaleFactor: 1, offline: true });
    const page = await context.newPage();
    const pageErrors = [];
    page.on('pageerror', (error) => pageErrors.push(error.message));
    await page.goto(pathToFileURL(htmlPath).href);
    await waitForRender(page, pageErrors);
    if (state === 'hover') {
      await page.hover(ROOT_SELECTOR);
    }
    await settleTransitions(page);

    const source = { package: config.package, version, component: 'Button', props: config.props, state };
    const snapshot = await captureSnapshot(page, ROOT_SELECTOR, source);
    await writeFile(resolve(outDir, `${lib}-${state}.json`), JSON.stringify(snapshot));
    await page.locator(ROOT_SELECTOR).screenshot({ path: resolve(outDir, `${lib}-${state}.png`), omitBackground: false });
    return snapshot;
  } finally {
    await browser.close();
  }
}

try {
  const snapshot = await capture(parseArguments(process.argv.slice(2)));
  const { x, y, width, height } = snapshot.root.box;
  console.log(`ok : ${snapshot.source.package} ${snapshot.source.state} box ${width}x${height} @ ${x},${y}`);
} catch (error) {
  console.error(`erreur : ${error.message}`);
  process.exit(1);
}
