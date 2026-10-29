import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { validatePackageName, validateVersion } from './npm-name.mjs';
import { ServiceError } from './protocol.mjs';

const SOURCES_MANIFEST = { name: 'uigen-sources', private: true };
const NPM_PREFIX_LINE = /^npm (?:error|ERR!) /;
const FRAMEWORK_DEPENDENCIES = [
  ['react', 'react'],
  ['vue', 'vue'],
  ['svelte', 'svelte'],
  ['lit', 'web-components'],
];

export const sourcesDir = (dir) => path.join(dir, '.uigen', 'sources');

export const packageDir = (dir, name) => path.join(sourcesDir(dir), 'node_modules', ...name.split('/'));

export const invalid = (message) => new ServiceError('invalidRequest', message);

export const isDirectory = async (target) => {
  try {
    return (await stat(target)).isDirectory();
  } catch {
    return false;
  }
};

export async function checkProject(dir) {
  if (typeof dir !== 'string' || !path.isAbsolute(dir)) {
    throw invalid('"dir" must be an absolute path');
  }
  if (!(await isDirectory(dir))) {
    throw invalid('"dir" must be an existing folder');
  }
}

export function checkPackage(name) {
  const verdict = validatePackageName(name);
  if (!verdict.ok) {
    throw invalid(verdict.reason);
  }
}

function checkVersion(version) {
  const verdict = validateVersion(version, { allowLatest: true });
  if (!verdict.ok) {
    throw invalid(verdict.reason);
  }
}

const lastUsefulLine = (stderr) => {
  const lines = String(stderr)
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line !== '');
  const last = lines.at(-1);
  return last === undefined ? 'no output from npm' : last.replace(NPM_PREFIX_LINE, '');
};

const npmEnv = () => ({ ...process.env, npm_config_ignore_scripts: 'true' });

async function ensureSourcesManifest(dir) {
  await mkdir(sourcesDir(dir), { recursive: true });
  try {
    await writeFile(path.join(sourcesDir(dir), 'package.json'), `${JSON.stringify(SOURCES_MANIFEST, null, 2)}\n`, { flag: 'wx' });
  } catch (error) {
    if (error.code !== 'EEXIST') {
      throw error;
    }
  }
}

const dependencyNames = (manifest) =>
  ['peerDependencies', 'dependencies'].flatMap((field) => {
    const group = manifest[field];
    return typeof group === 'object' && group !== null ? [Object.keys(group)] : [];
  });

export function detectFramework(manifest) {
  for (const names of dependencyNames(manifest)) {
    for (const [dependency, framework] of FRAMEWORK_DEPENDENCIES) {
      if (names.includes(dependency)) {
        return framework;
      }
    }
  }
  return manifest.customElements === undefined ? 'unknown' : 'web-components';
}

async function readInstalledManifest(dir, name, version) {
  try {
    const manifest = JSON.parse(await readFile(path.join(packageDir(dir, name), 'package.json'), 'utf8'));
    if (typeof manifest === 'object' && manifest !== null && typeof manifest.version === 'string') {
      return manifest;
    }
  } catch {
    // reported below with the package and version
  }
  throw new ServiceError('install', `${name}@${version} was installed but its package.json cannot be read`);
}

export async function install(params, { runNpm }) {
  const { dir, package: name, version } = params;
  checkPackage(name);
  checkVersion(version);
  await checkProject(dir);
  await ensureSourcesManifest(dir);
  const args = ['install', '--ignore-scripts', '--save-exact', '--no-audit', '--no-fund', '--prefix', sourcesDir(dir), `${name}@${version}`];
  const { code, stderr } = await runNpm(args, { cwd: sourcesDir(dir), env: npmEnv() });
  if (code !== 0) {
    throw new ServiceError('install', `npm could not install ${name}@${version}: ${lastUsefulLine(stderr)}`);
  }
  const manifest = await readInstalledManifest(dir, name, version);
  return {
    package: name,
    version: manifest.version,
    license: typeof manifest.license === 'string' ? manifest.license : null,
    framework: detectFramework(manifest),
  };
}

export async function uninstall(params, { runNpm }) {
  const { dir, package: name } = params;
  checkPackage(name);
  await checkProject(dir);
  if (!(await isDirectory(packageDir(dir, name)))) {
    throw new ServiceError('notInstalled', `${name} is not installed in this project`);
  }
  const args = ['uninstall', '--ignore-scripts', '--no-audit', '--no-fund', '--prefix', sourcesDir(dir), name];
  const { code, stderr } = await runNpm(args, { cwd: sourcesDir(dir), env: npmEnv() });
  if (code !== 0) {
    throw new ServiceError('uninstall', `npm could not uninstall ${name}: ${lastUsefulLine(stderr)}`);
  }
  return { package: name };
}
