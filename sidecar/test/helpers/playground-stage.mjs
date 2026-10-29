import { cp, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const FIXTURE = fileURLToPath(new URL('../fixtures/playground/', import.meta.url));
export const FIXTURE_TRIPWIRE = path.join(FIXTURE, 'lib', 'tripwire.js');

export async function stageFixture(files = {}) {
  const root = await mkdtemp(path.join(os.tmpdir(), 'uigen-playground-'));
  const staging = path.join(root, '.uigen', 'playground');
  await cp(FIXTURE, staging, { recursive: true });
  for (const [relative, contents] of Object.entries(files)) {
    const target = path.join(staging, ...relative.split('/'));
    if (contents === null) {
      await rm(target, { force: true });
    } else {
      await mkdir(path.dirname(target), { recursive: true });
      await writeFile(target, contents);
    }
  }
  return { root, staging, libDir: path.join(staging, 'lib'), remove: () => rm(root, { recursive: true, force: true }) };
}
