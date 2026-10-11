import { realpath, stat } from 'node:fs/promises';
import { isAbsolute, join, relative, sep } from 'node:path';

function isInside(root: string, candidate: string): boolean {
  const path = relative(root, candidate);
  return path !== '' && path !== '..' && !path.startsWith(`..${sep}`) && !isAbsolute(path);
}

export async function resolveConfinedFile(root: string, segments: readonly string[]): Promise<string | null> {
  try {
    const realRoot = await realpath(root);
    const realFile = await realpath(join(realRoot, ...segments));
    if (!isInside(realRoot, realFile)) return null;
    return (await stat(realFile)).isFile() ? realFile : null;
  } catch {
    return null;
  }
}
