import { after, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { readdir, readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { internalMessage } from '../protocol.mjs';

const SIDECAR_DIR = resolve(import.meta.dirname, '..');
const SERVICE = resolve(SIDECAR_DIR, 'service.mjs');
const ONE_MIB = 1024 * 1024;
const READ_TIMEOUT_MS = 15_000;
const EXIT_BUDGET_MS = 5_000;

const running = [];

after(() => {
  for (const service of running) {
    service.child.kill();
  }
});

const startService = () => {
  const child = spawn(process.execPath, [SERVICE], { stdio: ['pipe', 'pipe', 'pipe'] });
  const lines = [];
  const waiters = [];
  let buffered = '';
  let stdoutEnded = false;

  child.stderr.resume();
  child.stdin.on('error', () => {});
  child.stdout.setEncoding('utf8');
  child.stdout.on('data', (chunk) => {
    buffered += chunk;
    let newline = buffered.indexOf('\n');
    while (newline !== -1) {
      lines.push(buffered.slice(0, newline));
      buffered = buffered.slice(newline + 1);
      newline = buffered.indexOf('\n');
    }
    while (lines.length > 0 && waiters.length > 0) {
      waiters.shift().resolve(lines.shift());
    }
  });
  child.stdout.on('end', () => {
    stdoutEnded = true;
    for (const waiter of waiters.splice(0)) {
      waiter.reject(new Error('service stdout closed before a line arrived'));
    }
  });

  const exited = new Promise((resolveExit) => {
    child.on('exit', (code, signal) => resolveExit({ code, signal }));
    child.on('error', (error) => resolveExit({ code: null, signal: null, error }));
  });

  const readLine = () => {
    if (lines.length > 0) {
      return Promise.resolve(lines.shift());
    }
    if (stdoutEnded) {
      return Promise.reject(new Error('service stdout closed before a line arrived'));
    }
    return new Promise((resolveLine, rejectLine) => {
      const timer = setTimeout(() => rejectLine(new Error('no line from the service within 15 s')), READ_TIMEOUT_MS);
      waiters.push({
        resolve: (line) => {
          clearTimeout(timer);
          resolveLine(line);
        },
        reject: (error) => {
          clearTimeout(timer);
          rejectLine(error);
        },
      });
    });
  };

  const service = {
    child,
    exited,
    readLine,
    readJson: async () => JSON.parse(await readLine()),
    sendRaw: (text) => child.stdin.write(`${text}\n`),
    send: (message) => child.stdin.write(`${JSON.stringify(message)}\n`),
  };
  running.push(service);
  return service;
};

const startGreeted = async () => {
  const service = startService();
  const greeting = await service.readJson();
  return { service, greeting };
};

const assertErrorResponse = (response, kind) => {
  assert.equal(response.ok, false);
  assert.equal(response.error.kind, kind);
  assert.equal(typeof response.error.message, 'string');
  assert.notEqual(response.error.message.trim(), '');
};

const assertStillAnswers = async (service) => {
  service.send({ id: 900, method: 'shutdown', params: {} });
  const response = await service.readJson();
  assert.equal(response.id, 900);
  assert.equal(response.ok, true);
};

describe('sidecar protocol version 1: greeting and replies', () => {
  // covers: E2-1.1
  it('writes the protocol greeting as the first stdout line', async () => {
    const service = startService();
    const first = await service.readLine();
    const greeting = JSON.parse(first);

    assert.equal(greeting.protocol, 1);
    assert.equal(greeting.service, 'uigen-sidecar');
    assert.equal(typeof greeting.node, 'string');
    service.child.stdin.end();
    await service.exited;
  });

  // covers: E2-1.1
  it('answers a valid request on one JSON line carrying the same id', async () => {
    const { service } = await startGreeted();
    service.send({ id: 42, method: 'shutdown', params: {} });

    const line = await service.readLine();
    const response = JSON.parse(line);

    assert.equal(response.id, 42);
    assert.equal(response.ok, true);
    assert.deepEqual(response.result, {});
    await service.exited;
  });
});

describe('sidecar protocol version 1: invalid input', () => {
  // covers: E2-1.2
  it('rejects a line that is not JSON with invalidRequest and keeps serving', async () => {
    const { service } = await startGreeted();
    service.sendRaw('this is {not json');

    const response = await service.readJson();

    assertErrorResponse(response, 'invalidRequest');
    assert.match(response.error.message, /json/i);
    await assertStillAnswers(service);
  });

  // covers: E2-1.2
  it('rejects a request without an integer id with invalidRequest and keeps serving', async () => {
    const { service } = await startGreeted();

    service.send({ method: 'shutdown', params: {} });
    const missing = await service.readJson();
    service.send({ id: '7', method: 'shutdown', params: {} });
    const text = await service.readJson();
    service.send({ id: 1.5, method: 'shutdown', params: {} });
    const fractional = await service.readJson();

    for (const response of [missing, text, fractional]) {
      assertErrorResponse(response, 'invalidRequest');
      assert.match(response.error.message, /\bid\b/i);
    }
    await assertStillAnswers(service);
  });

  // covers: E2-1.2
  it('rejects an unknown method with invalidRequest naming the method and keeps serving', async () => {
    const { service } = await startGreeted();
    service.send({ id: 5, method: 'frobnicate', params: {} });

    const response = await service.readJson();

    assert.equal(response.id, 5);
    assertErrorResponse(response, 'invalidRequest');
    assert.match(response.error.message, /frobnicate/);
    await assertStillAnswers(service);
  });

  // covers: E2-1.2
  it('rejects a line over 1 MiB with tooLarge and keeps serving', async () => {
    const { service } = await startGreeted();
    const filler = 'a'.repeat(ONE_MIB + 16);
    service.sendRaw(JSON.stringify({ id: 6, method: 'install', params: { filler } }));

    const response = await service.readJson();

    assertErrorResponse(response, 'tooLarge');
    await assertStillAnswers(service);
  });
});

describe('sidecar protocol version 1: termination', () => {
  // covers: E2-1.3
  it('exits with code 0 within 5 s after a shutdown request', async () => {
    const { service } = await startGreeted();
    const started = Date.now();
    service.send({ id: 1, method: 'shutdown', params: {} });

    const response = await service.readJson();
    const { code } = await service.exited;

    assert.equal(response.id, 1);
    assert.equal(response.ok, true);
    assert.equal(code, 0);
    assert.ok(Date.now() - started < EXIT_BUDGET_MS, 'exited within 5 s');
  });

  // covers: E2-1.3
  it('exits with code 0 within 5 s when stdin is closed', async () => {
    const { service } = await startGreeted();
    const started = Date.now();
    service.child.stdin.end();

    const { code } = await service.exited;

    assert.equal(code, 0);
    assert.ok(Date.now() - started < EXIT_BUDGET_MS, 'exited within 5 s');
  });
});

describe('sidecar internal errors', () => {
  it('names the first line of the cause', () => {
    const error = new TypeError('browser is gone\nlong call log');
    assert.equal(internalMessage('inspect', error), 'internal error in "inspect": TypeError: browser is gone');
  });

  it('bounds a long cause and survives a thrown non-error', () => {
    assert.ok(internalMessage('inspect', new Error('x'.repeat(5000))).length < 400);
    assert.equal(internalMessage('inspect', 'text'), 'internal error in "inspect": unknown');
  });
});

describe('sidecar opens no network port', () => {
  // covers: E2-1.5
  it('has no server, listen, WebSocketServer or remote-debugging-port in any sidecar module', async () => {
    const entries = await readdir(SIDECAR_DIR, { withFileTypes: true });
    const modules = entries.filter((entry) => entry.isFile() && entry.name.endsWith('.mjs')).map((entry) => entry.name);
    const forbidden = [/createServer/, /\blisten\s*\(/, /WebSocketServer/, /remote-debugging-port/];

    assert.ok(modules.includes('service.mjs'), 'service.mjs exists');
    assert.ok(modules.includes('npm-name.mjs'), 'npm-name.mjs exists');
    for (const name of modules) {
      const source = await readFile(resolve(SIDECAR_DIR, name), 'utf8');
      for (const pattern of forbidden) {
        assert.ok(!pattern.test(source), `${name} must not match ${pattern}`);
      }
    }
  });
});
