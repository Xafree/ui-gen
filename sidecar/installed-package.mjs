import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { checkPackage, checkProject, detectFramework, isDirectory, packageDir, sourcesDir } from './install.mjs';
import { ServiceError } from './protocol.mjs';

const isObject = (value) => typeof value === 'object' && value !== null && !Array.isArray(value);

const readJson = async (file) => JSON.parse(await readFile(file, 'utf8'));

export const insideRoot = (root, relative) => {
  const target = path.resolve(root, relative);
  return target.startsWith(`${root}${path.sep}`) ? target : null;
};

async function readCustomElements(root, manifest) {
  if (typeof manifest.customElements !== 'string') {
    return new Map();
  }
  const file = insideRoot(root, manifest.customElements);
  if (file === null) {
    return new Map();
  }
  let customElements;
  try {
    customElements = await readJson(file);
  } catch (error) {
    process.stderr.write(`custom elements manifest unreadable: ${error instanceof Error ? error.name : 'unknown'}\n`);
    return new Map();
  }
  const tags = new Map();
  const modules = isObject(customElements) && Array.isArray(customElements.modules) ? customElements.modules : [];
  for (const module of modules.filter(isObject)) {
    const declarations = Array.isArray(module.declarations) ? module.declarations : [];
    for (const declaration of declarations.filter(isObject)) {
      if (typeof declaration.tagName === 'string' && typeof module.path === 'string') {
        tags.set(declaration.tagName, module.path);
      }
    }
  }
  return tags;
}

export async function readInstalledPackage(dir, name) {
  checkPackage(name);
  await checkProject(dir);
  const root = packageDir(dir, name);
  if (!(await isDirectory(root))) {
    throw new ServiceError('notInstalled', `${name} is not installed in this project`);
  }
  let manifest;
  try {
    manifest = await readJson(path.join(root, 'package.json'));
  } catch {
    throw new ServiceError('notInstalled', `${name} is not installed in this project: its package.json cannot be read`);
  }
  if (!isObject(manifest) || typeof manifest.version !== 'string') {
    throw new ServiceError('notInstalled', `${name} is not installed in this project: its package.json has no version`);
  }
  return {
    name,
    root,
    sources: sourcesDir(dir),
    version: manifest.version,
    framework: detectFramework(manifest),
    tags: await readCustomElements(root, manifest),
  };
}
