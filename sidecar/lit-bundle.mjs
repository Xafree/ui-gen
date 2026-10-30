import { existsSync } from 'node:fs';
import path from 'node:path';
import { build } from 'esbuild';
import { NODE_MODULES, confineToSources } from './bundle.mjs';
import { ServiceError } from './protocol.mjs';

const ABSOLUTE_PATH = /"(?:[A-Za-z]:)?[\\/][^"]*"|'(?:[A-Za-z]:)?[\\/][^']*'/g;
const FALLBACK_TSCONFIG = { compilerOptions: { experimentalDecorators: true, useDefineForClassFields: false } };

const insideLib = (relative) => relative !== '' && !relative.startsWith('..') && !path.isAbsolute(relative);

export const maskPaths = (text) => text.replace(ABSOLUTE_PATH, '"<absolute path>"');

function describeFailure(error) {
  const first = error?.errors?.[0];
  if (first === undefined) {
    return error instanceof Error ? error.message : 'unknown error';
  }
  const cause = maskPaths(first.text);
  const file = first.location?.file;
  return typeof file === 'string' && insideLib(file) ? `${file.split(path.sep).join('/')}: ${cause}` : cause;
}

export async function bundleLitPackage({ libDir }) {
  const root = path.resolve(libDir);
  const entry = path.join(root, 'index.ts');
  if (!existsSync(entry)) {
    throw new ServiceError('notInstalled', 'the playground library is not staged (lib/index.ts is missing)');
  }
  const tsconfig = path.join(root, 'tsconfig.json');
  try {
    const { outputFiles } = await build({
      entryPoints: [entry],
      absWorkingDir: root,
      bundle: true,
      write: false,
      format: 'iife',
      platform: 'browser',
      minify: true,
      define: { 'process.env.NODE_ENV': '"production"' },
      nodePaths: [NODE_MODULES],
      alias: { lit: path.join(NODE_MODULES, 'lit') },
      ...(existsSync(tsconfig) ? { tsconfig } : { tsconfigRaw: FALLBACK_TSCONFIG }),
      plugins: [confineToSources([root, NODE_MODULES])],
      logLevel: 'silent',
    });
    return { code: outputFiles[0].text };
  } catch (error) {
    throw new ServiceError('render', `cannot bundle the playground library: ${describeFailure(error)}`);
  }
}
