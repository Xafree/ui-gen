import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import path from 'node:path';

export const NPM_TIMEOUT_MS = 10 * 60 * 1000;
const MAX_CAPTURED_BYTES = 64 * 1024;

const running = new Set();

const npmCliCandidates = () => {
  const nodeDir = path.dirname(process.execPath);
  return [
    process.env.npm_execpath,
    path.join(nodeDir, 'node_modules', 'npm', 'bin', 'npm-cli.js'),
    path.join(nodeDir, '..', 'lib', 'node_modules', 'npm', 'bin', 'npm-cli.js'),
  ].filter((candidate) => typeof candidate === 'string' && candidate.endsWith('npm-cli.js'));
};

function npmCommand(args) {
  const cli = npmCliCandidates().find((candidate) => existsSync(candidate));
  if (cli !== undefined) {
    return { command: process.execPath, args: [cli, ...args] };
  }
  return { command: process.platform === 'win32' ? 'npm.cmd' : 'npm', args };
}

const tail = (text) => (text.length > MAX_CAPTURED_BYTES ? text.slice(-MAX_CAPTURED_BYTES) : text);

export function killRunningNpm() {
  for (const child of running) {
    child.kill();
  }
}

export function runNpm(args, { cwd, env, timeoutMs = NPM_TIMEOUT_MS }) {
  return new Promise((resolve) => {
    const command = npmCommand(args);
    let stdout = '';
    let stderr = '';
    let child;
    try {
      child = spawn(command.command, command.args, { cwd, env, stdio: ['ignore', 'pipe', 'pipe'], timeout: timeoutMs, windowsHide: true });
    } catch (error) {
      resolve({ code: 1, stdout, stderr: `cannot start npm: ${error.message}` });
      return;
    }
    running.add(child);
    child.stdout.on('data', (chunk) => {
      stdout = tail(stdout + chunk);
    });
    child.stderr.on('data', (chunk) => {
      stderr = tail(stderr + chunk);
    });
    child.on('error', (error) => {
      running.delete(child);
      resolve({ code: 1, stdout, stderr: `${stderr}\ncannot run npm: ${error.message}` });
    });
    child.on('close', (code, signal) => {
      running.delete(child);
      resolve({ code: code ?? 1, stdout, stderr: signal === null ? stderr : `${stderr}\nnpm stopped by ${signal}` });
    });
  });
}
