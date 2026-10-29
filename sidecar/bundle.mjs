import path from 'node:path';
import { build } from 'esbuild';
import { ServiceError } from './protocol.mjs';

const NODE_MODULES = path.join(import.meta.dirname, 'node_modules');

export const IS_COMPONENT = `(value) => typeof value === 'function' || (typeof value === 'object' && value !== null && '$$typeof' in value)`;

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
      logLevel: 'silent',
    });
    return outputFiles[0].text;
  } catch (error) {
    const first = error?.errors?.[0]?.text ?? (error instanceof Error ? error.message : 'unknown error');
    throw new ServiceError('render', `cannot bundle the package for the browser: ${first}`);
  }
}
