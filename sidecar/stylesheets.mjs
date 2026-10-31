import { readFile, readdir, realpath, stat } from 'node:fs/promises';
import path from 'node:path';

export const MAX_STYLESHEET_BYTES = 8 * 1024 * 1024;

const PACKAGE_NAME = /^(?:@[a-z0-9~-][a-z0-9._~-]*\/)?[a-z0-9~-][a-z0-9._~-]*$/;
const UNRESOLVED_IMPORT = /@import\b/;
const COMPILED_SUFFIX = '.min.css';

const isObject = (value) => typeof value === 'object' && value !== null && !Array.isArray(value);

const isInside = (root, target) => {
  const relative = path.relative(root, target);
  return relative !== '' && !relative.startsWith('..') && !path.isAbsolute(relative);
};

const real = (target) => realpath(target).catch(() => null);

async function readManifest(root) {
  try {
    const manifest = JSON.parse(await readFile(path.join(root, 'package.json'), 'utf8'));
    return isObject(manifest) ? manifest : {};
  } catch {
    return {};
  }
}

async function readConfined(root, relative) {
  const file = await real(path.resolve(root, relative));
  if (file === null || !isInside(root, file)) {
    return null;
  }
  const info = await stat(file).catch(() => null);
  if (info === null || !info.isFile()) {
    return null;
  }
  if (info.size > MAX_STYLESHEET_BYTES) {
    return { tooBig: true };
  }
  return { css: await readFile(file, 'utf8') };
}

async function compiledSheets(root) {
  const entries = await readdir(path.join(root, 'dist')).catch(() => []);
  return entries.filter((entry) => entry.endsWith(COMPILED_SUFFIX)).sort().map((entry) => path.join('dist', entry));
}

async function stylesheetOf(modules, name) {
  const root = await real(path.join(modules, ...name.split('/')));
  if (root === null || !isInside(modules, root)) {
    return null;
  }
  const { style } = await readManifest(root);
  if (typeof style !== 'string') {
    return null;
  }
  const own = await readConfined(root, style);
  if (own === null || own.tooBig || !UNRESOLVED_IMPORT.test(own.css)) {
    return own;
  }
  for (const candidate of await compiledSheets(root)) {
    const compiled = await readConfined(root, candidate);
    if (compiled?.tooBig || (compiled !== null && !UNRESOLVED_IMPORT.test(compiled.css))) {
      return compiled;
    }
  }
  return null;
}

export async function findStylesheets(installed) {
  const modules = await real(path.join(installed.sources, 'node_modules'));
  if (modules === null) {
    return { styles: [], notes: [] };
  }
  const { dependencies } = await readManifest(installed.root);
  const dependencyNames = isObject(dependencies) ? Object.keys(dependencies).filter((name) => PACKAGE_NAME.test(name)) : [];
  const styles = [];
  const notes = [];
  for (const name of [...dependencyNames, installed.name]) {
    const found = await stylesheetOf(modules, name);
    if (found?.tooBig) {
      notes.push(`stylesheet of ${name} ignored: larger than ${MAX_STYLESHEET_BYTES / 1024 / 1024} MiB`);
    } else if (found !== null) {
      styles.push(found.css);
    }
  }
  return { styles, notes };
}
