import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

const manifestOf = (name) => ({
  name,
  version: '1.0.0',
  type: 'module',
  main: 'index.js',
  peerDependencies: { react: '>=18' },
});

export async function makeTempDir(prefix = 'uigen-review-') {
  const dir = await mkdtemp(path.join(os.tmpdir(), prefix));
  return { dir, remove: () => rm(dir, { recursive: true, force: true }) };
}

export async function writePackage(root, name, files) {
  const target = path.join(root, ...name.split('/'));
  await mkdir(target, { recursive: true });
  await writeFile(path.join(target, 'package.json'), `${JSON.stringify(manifestOf(name))}\n`);
  for (const [file, contents] of Object.entries(files)) {
    await writeFile(path.join(target, file), contents);
  }
  return target;
}

export async function makeSourcesProject(packages) {
  const { dir, remove } = await makeTempDir();
  const sources = path.join(dir, '.uigen', 'sources');
  const modules = path.join(sources, 'node_modules');
  await mkdir(modules, { recursive: true });
  await writeFile(path.join(sources, 'package.json'), `${JSON.stringify({ name: 'uigen-sources', private: true })}\n`);
  for (const [name, files] of Object.entries(packages)) {
    await writePackage(modules, name, files);
  }
  return { dir, sources, modules, remove };
}
