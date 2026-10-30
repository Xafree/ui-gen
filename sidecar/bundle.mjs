import { realpathSync } from 'node:fs';
import path from 'node:path';
import { build } from 'esbuild';
import { ServiceError } from './protocol.mjs';

export const NODE_MODULES = path.join(import.meta.dirname, 'node_modules');
const FOLLOW_UP = Symbol('follow-up');

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
      esbuild.onResolve({ filter: /.*/ }, async (args) => {
        if (args.pluginData === FOLLOW_UP) {
          return undefined;
        }
        const resolved = await esbuild.resolve(args.path, {
          importer: args.importer,
          namespace: args.namespace,
          resolveDir: args.resolveDir,
          kind: args.kind,
          pluginData: FOLLOW_UP,
        });
        if (resolved.errors.length > 0 || resolved.external || resolved.namespace !== 'file') {
          return resolved;
        }
        const target = tryRealPath(resolved.path);
        if (!allowed.some((root) => isInside(root, target))) {
          return { errors: [{ text: `import "${args.path}" resolves outside the project sources and was refused` }] };
        }
        return resolved;
      });
    },
  };
}

export async function bundleEntry({ contents, resolveDir }) {
  try {
    const { outputFiles } = await build({
      stdin: { contents, resolveDir, loader: 'js' },
      bundle: true,
      write: false,
      format: 'iife',
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
