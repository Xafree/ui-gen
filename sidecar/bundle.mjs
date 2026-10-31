import { realpathSync } from 'node:fs';
import { stat } from 'node:fs/promises';
import path from 'node:path';
import { build } from 'esbuild';
import { ServiceError } from './protocol.mjs';

export const NODE_MODULES = path.join(import.meta.dirname, 'node_modules');
const MAX_CACHED_BUNDLES = 16;

export const IS_COMPONENT = `(value) => typeof value === 'function' || (typeof value === 'object' && value !== null && '$$typeof' in value)`;

const realPath = (target) => realpathSync.native(target);

const tryRealPath = (target) => {
  try {
    return realPath(target);
  } catch {
    return target;
  }
};

const isInside = (root, target) => {
  const relative = path.relative(root, target);
  return relative !== '' && !relative.startsWith('..') && !path.isAbsolute(relative);
};

export function confineToSources(roots) {
  return {
    name: 'uigen-confine',
    setup(esbuild) {
      const allowed = roots.map(realPath);
      esbuild.onLoad({ filter: /.*/, namespace: 'file' }, (args) => {
        const target = tryRealPath(args.path);
        if (allowed.some((root) => isInside(root, target))) {
          return undefined;
        }
        return { errors: [{ text: `import "${path.basename(args.path)}" resolves outside the project sources and was refused` }] };
      });
    },
  };
}

export async function bundleEntry({ contents, resolveDir, globalName }) {
  try {
    const { outputFiles } = await build({
      stdin: { contents, resolveDir, loader: 'js' },
      bundle: true,
      write: false,
      format: 'iife',
      globalName,
      platform: 'browser',
      jsx: 'automatic',
      minify: true,
      define: { 'process.env.NODE_ENV': '"production"' },
      nodePaths: [NODE_MODULES],
      alias: { react: path.join(NODE_MODULES, 'react'), 'react-dom': path.join(NODE_MODULES, 'react-dom') },
      plugins: [confineToSources([resolveDir, NODE_MODULES])],
      logLevel: 'silent',
    });
    return outputFiles[0].text;
  } catch (error) {
    const first = error?.errors?.[0]?.text ?? (error instanceof Error ? error.message : 'unknown error');
    throw new ServiceError('render', `cannot bundle the package for the browser: ${first}`);
  }
}

const cachedBundles = new Map();

const stampOf = (file) =>
  stat(file).then(
    ({ mtimeMs, size }) => `${mtimeMs}:${size}`,
    () => 'absent',
  );

export async function cachedBundle({ stamps, ...entry }) {
  const key = JSON.stringify([entry.resolveDir, entry.globalName, entry.contents]);
  const stamp = (await Promise.all(stamps.map(stampOf))).join('|');
  const known = cachedBundles.get(key);
  cachedBundles.delete(key);
  const code = known?.stamp === stamp ? known.code : bundleEntry(entry);
  cachedBundles.set(key, { stamp, code });
  if (cachedBundles.size > MAX_CACHED_BUNDLES) {
    cachedBundles.delete(cachedBundles.keys().next().value);
  }
  code.catch(() => {
    if (cachedBundles.get(key)?.code === code) {
      cachedBundles.delete(key);
    }
  });
  return code;
}
