import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import path from 'node:path';

export const NPM_TIMEOUT_MS = 10 * 60 * 1000;
const MAX_CAPTURED_BYTES = 64 * 1024;

const running = new Set();

const npmCliCandidates = ({ execPath, npmExecPath }) => {
  const nodeDir = path.dirname(execPath);
  return [
    npmExecPath,
    path.join(nodeDir, 'node_modules', 'npm', 'bin', 'npm-cli.js'),
    path.join(nodeDir, '..', 'lib', 'node_modules', 'npm', 'bin', 'npm-cli.js'),
  ].filter((candidate) => typeof candidate === 'string' && candidate.endsWith('npm-cli.js'));
};

export function npmCommand(args, { execPath = process.execPath, npmExecPath = process.env.npm_execpath, exists = existsSync } = {}) {
  const cli = npmCliCandidates({ execPath, npmExecPath }).find((candidate) => exists(candidate));
  return cli === undefined ? null : { command: execPath, args: [cli, ...args] };
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
    if (command === null) {
      resolve({ code: 1, stdout: '', stderr: 'npm-cli.js not found next to node' });
      return;
    }
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
