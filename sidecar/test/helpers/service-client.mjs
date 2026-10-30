import { spawn, spawnSync } from 'node:child_process';
import { cp, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const SERVICE = fileURLToPath(new URL('../../service.mjs', import.meta.url));
const PACKAGES = fileURLToPath(new URL('../fixtures/packages/', import.meta.url));
const LAUNCH_SPY = pathToFileURL(fileURLToPath(new URL('./launch-spy.mjs', import.meta.url))).href;
const REQUEST_TIMEOUT_MS = 90_000;

const killTree = (pid) => {
  if (process.platform === 'win32') {
    spawnSync('taskkill', ['/T', '/F', '/PID', String(pid)], { windowsHide: true });
  } else {
    process.kill(pid, 'SIGKILL');
  }
};

export async function makeProject(packages) {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'uigen-capture-'));
  const sources = path.join(dir, '.uigen', 'sources');
  await mkdir(path.join(sources, 'node_modules'), { recursive: true });
  await writeFile(path.join(sources, 'package.json'), `${JSON.stringify({ name: 'uigen-sources', private: true })}\n`);
  for (const name of packages) {
    await cp(path.join(PACKAGES, name), path.join(sources, 'node_modules', name), { recursive: true });
  }
  return { dir, remove: () => rm(dir, { recursive: true, force: true }) };
}

export async function startService({ launchLog } = {}) {
  const env = { ...process.env };
  if (launchLog !== undefined) {
    env.UIGEN_LAUNCH_LOG = launchLog;
    env.NODE_OPTIONS = `--import=${LAUNCH_SPY}`;
  }
  const child = spawn(process.execPath, [SERVICE], { stdio: ['pipe', 'pipe', 'pipe'], env, windowsHide: true });
  const exited = new Promise((resolve) => {
    child.once('exit', (code, signal) => resolve({ code, signal }));
  });
  const lines = [];
  const waiters = [];
  createInterface({ input: child.stdout }).on('line', (line) => {
    const waiter = waiters.shift();
    if (waiter === undefined) {
      lines.push(line);
    } else {
      waiter(line);
    }
  });

  const nextLine = () =>
    new Promise((resolve, reject) => {
      if (lines.length > 0) {
        resolve(lines.shift());
        return;
      }
      const timer = setTimeout(() => reject(new Error(`no line from the service within ${REQUEST_TIMEOUT_MS} ms`)), REQUEST_TIMEOUT_MS);
      waiters.push((line) => {
        clearTimeout(timer);
        resolve(line);
      });
    });

  let nextId = 1;
  const request = async (method, params) => {
    const id = nextId;
    nextId += 1;
    child.stdin.write(`${JSON.stringify({ id, method, params })}\n`);
    const response = JSON.parse(await nextLine());
    if (response.id !== id) {
      throw new Error(`response id ${response.id} does not match request id ${id}`);
    }
    return response;
  };

  const stop = async () => {
    if (child.exitCode === null && child.signalCode === null && child.pid !== undefined) {
      killTree(child.pid);
    }
    await exited;
  };

  const greeting = JSON.parse(await nextLine());
  return { greeting, request, exited, stop, pid: child.pid };
}
