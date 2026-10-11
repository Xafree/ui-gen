import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  ComputeClient,
  ComputeUnavailableError,
  type ComputeProcess,
} from '../../../../../src/modules/sync/adapters/utility-process/compute-client';
import type { ComputeRequest } from '../../../../../src/modules/sync/adapters/utility-process/compute-protocol';
import type { AnalyzerStatus } from '../../../../../src/modules/sync/ports/component-analyzer';
import { modelOf } from '../../application/components/component-doubles';

class FakeProcess implements ComputeProcess {
  readonly sent: ComputeRequest[] = [];
  killed = false;
  failToSend = false;
  #message: (message: unknown) => void = () => undefined;
  #exit: () => void = () => undefined;

  send(request: ComputeRequest): void {
    if (this.failToSend) throw new Error('channel closed');
    this.sent.push(request);
  }

  onMessage(listener: (message: unknown) => void): void {
    this.#message = listener;
  }

  onExit(listener: () => void): void {
    this.#exit = listener;
  }

  kill(): void {
    this.killed = true;
  }

  say(message: unknown): void {
    this.#message(message);
  }

  becomeReady(): void {
    this.say({ outcome: 'ready' });
  }

  die(): void {
    this.#exit();
  }

  answerLast(extra: Record<string, unknown> = {}): void {
    const request = this.sent.at(-1);
    if (request === undefined) throw new Error('Nothing was sent');
    this.say({ outcome: 'analysis', id: request.id, model: modelOf(request.text), highlight: [], ...extra });
  }
}

class Harness {
  readonly processes: FakeProcess[] = [];
  readonly statuses: AnalyzerStatus[] = [];
  spawnError: Error | null = null;
  readonly client = new ComputeClient({
    spawn: () => {
      if (this.spawnError !== null) throw this.spawnError;
      const process = new FakeProcess();
      this.processes.push(process);
      return process;
    },
    onStatus: (status) => {
      this.statuses.push(status);
    },
    requestTimeoutMs: 1_000,
    restartBaseDelayMs: 100,
    restartMaxDelayMs: 400,
  });

  get current(): FakeProcess {
    const process = this.processes.at(-1);
    if (process === undefined) throw new Error('No process was spawned');
    return process;
  }
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

async function rejectionOf(promise: Promise<unknown>): Promise<unknown> {
  return promise.then(
    () => null,
    (error: unknown) => error,
  );
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

describe('starting the analysis process', () => {
  it('spawns one process, and announces ready once it says so', () => {
    const harness = new Harness();

    harness.client.start();
    harness.client.start();
    expect(harness.processes).toHaveLength(1);
    expect(harness.statuses).toEqual([]);

    harness.current.becomeReady();
    harness.current.becomeReady();

    expect(harness.statuses).toEqual(['ready']);
  });

  it('answers the analysis of a text once the process is ready', async () => {
    const harness = new Harness();
    harness.client.start();
    const analysis = harness.client.analyze('tag=hz-a');

    harness.current.becomeReady();
    await vi.advanceTimersByTimeAsync(0);
    harness.current.answerLast();

    await expect(analysis).resolves.toMatchObject({ model: { tag: 'hz-a' }, highlight: [] });
    expect(harness.current.sent).toEqual([{ id: 1, operation: 'analyze', text: 'tag=hz-a' }]);
  });

  it('answers the model of a text', async () => {
    const harness = new Harness();
    harness.client.start();
    harness.current.becomeReady();

    const model = harness.client.readModel('tag=hz-b');
    await vi.advanceTimersByTimeAsync(0);
    harness.current.say({ outcome: 'model', id: 1, model: modelOf('tag=hz-b') });

    await expect(model).resolves.toMatchObject({ tag: 'hz-b' });
    expect(harness.current.sent[0]?.operation).toBe('read-model');
  });

  it('refuses requests while stopped', async () => {
    const harness = new Harness();

    await expect(harness.client.analyze('x')).rejects.toThrow('stopped');
  });
});

describe('losing the analysis process', () => {
  it('announces the restart, rejects what was pending, and spawns again', async () => {
    const harness = new Harness();
    harness.client.start();
    harness.current.becomeReady();
    const pending = rejectionOf(harness.client.analyze('tag=hz-a'));
    await vi.advanceTimersByTimeAsync(0);

    harness.current.die();

    expect(harness.statuses).toEqual(['ready', 'restarting']);
    expect(await pending).toBeInstanceOf(ComputeUnavailableError);
    expect(harness.processes).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(100);
    expect(harness.processes).toHaveLength(2);
    harness.current.becomeReady();
    expect(harness.statuses).toEqual(['ready', 'restarting', 'ready']);
  });

  it('serves a request made during the restart once the new process is ready', async () => {
    const harness = new Harness();
    harness.client.start();
    harness.current.becomeReady();
    harness.current.die();

    const analysis = harness.client.analyze('tag=hz-c');
    await vi.advanceTimersByTimeAsync(100);
    harness.current.becomeReady();
    await vi.advanceTimersByTimeAsync(0);
    harness.current.answerLast();

    await expect(analysis).resolves.toMatchObject({ model: { tag: 'hz-c' } });
  });

  it('waits longer between restarts while the process keeps dying, and starts over once ready', async () => {
    const harness = new Harness();
    harness.client.start();

    harness.current.die();
    await vi.advanceTimersByTimeAsync(99);
    expect(harness.processes).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(harness.processes).toHaveLength(2);

    harness.current.die();
    await vi.advanceTimersByTimeAsync(199);
    expect(harness.processes).toHaveLength(2);
    await vi.advanceTimersByTimeAsync(1);
    expect(harness.processes).toHaveLength(3);

    harness.current.die();
    await vi.advanceTimersByTimeAsync(400);
    expect(harness.processes).toHaveLength(4);
    harness.current.die();
    await vi.advanceTimersByTimeAsync(399);
    expect(harness.processes).toHaveLength(4);
    await vi.advanceTimersByTimeAsync(1);
    expect(harness.processes).toHaveLength(5);

    harness.current.becomeReady();
    harness.current.die();
    await vi.advanceTimersByTimeAsync(100);
    expect(harness.processes).toHaveLength(6);
  });

  it('keeps trying when the process cannot be spawned', async () => {
    const error = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const harness = new Harness();
    harness.spawnError = new Error('no executable');

    harness.client.start();
    await vi.advanceTimersByTimeAsync(100);
    harness.spawnError = null;
    await vi.advanceTimersByTimeAsync(200);

    expect(error).toHaveBeenCalledTimes(2);
    expect(harness.processes).toHaveLength(1);
  });

  it('ignores the exit of a process it already replaced', () => {
    const harness = new Harness();
    harness.client.start();
    const first = harness.current;
    first.becomeReady();
    first.die();

    first.die();

    expect(harness.statuses).toEqual(['ready', 'restarting']);
  });
});

describe('a request that goes wrong', () => {
  it('kills the process and rejects when the answer takes too long', async () => {
    const harness = new Harness();
    harness.client.start();
    harness.current.becomeReady();
    const outcome = rejectionOf(harness.client.analyze('tag=hz-a'));
    await vi.advanceTimersByTimeAsync(1_000);

    expect(messageOf(await outcome)).toContain('timed out');
    expect(harness.current.killed).toBe(true);
  });

  it('rejects when the process never becomes ready', async () => {
    const harness = new Harness();
    harness.client.start();
    const outcome = rejectionOf(harness.client.analyze('tag=hz-a'));
    await vi.advanceTimersByTimeAsync(1_000);

    expect(messageOf(await outcome)).toContain('not ready');
    harness.current.becomeReady();
    await vi.advanceTimersByTimeAsync(0);
    expect(harness.current.sent).toEqual([]);
  });

  it('rejects when the process reports a failure', async () => {
    const harness = new Harness();
    harness.client.start();
    harness.current.becomeReady();
    const outcome = rejectionOf(harness.client.analyze('tag=hz-a'));
    await vi.advanceTimersByTimeAsync(0);

    harness.current.say({ outcome: 'failed', id: 1 });

    expect(messageOf(await outcome)).toContain('analysis failed');
  });

  it('rejects when the answer is not the kind that was asked', async () => {
    const harness = new Harness();
    harness.client.start();
    harness.current.becomeReady();
    const model = rejectionOf(harness.client.readModel('tag=hz-a'));
    const analysis = rejectionOf(harness.client.analyze('tag=hz-a'));
    await vi.advanceTimersByTimeAsync(0);

    harness.current.say({ outcome: 'analysis', id: 1, model: modelOf('tag=hz-a'), highlight: [] });
    harness.current.say({ outcome: 'model', id: 2, model: modelOf('tag=hz-a') });

    expect(messageOf(await model)).toContain('unexpected reply');
    expect(messageOf(await analysis)).toContain('unexpected reply');
  });

  it('rejects when the message cannot be sent', async () => {
    const error = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const harness = new Harness();
    harness.client.start();
    harness.current.becomeReady();
    harness.current.failToSend = true;

    const outcome = await rejectionOf(harness.client.analyze('tag=hz-a'));

    expect(messageOf(outcome)).toContain('unreachable');
    expect(error).toHaveBeenCalledOnce();
  });

  it('warns about an unreadable message and ignores an unknown answer', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const harness = new Harness();
    harness.client.start();
    harness.current.becomeReady();
    const analysis = harness.client.analyze('tag=hz-a');
    await vi.advanceTimersByTimeAsync(0);

    harness.current.say('garbage');
    harness.current.say({ outcome: 'failed', id: 99 });
    harness.current.answerLast();

    expect(warn).toHaveBeenCalledOnce();
    await expect(analysis).resolves.toMatchObject({ model: { tag: 'hz-a' } });
  });

  it('ignores an answer that comes from another process', async () => {
    const harness = new Harness();
    harness.client.start();
    const first = harness.current;
    first.becomeReady();
    const analysis = rejectionOf(harness.client.analyze('tag=hz-a'));
    await vi.advanceTimersByTimeAsync(0);
    const request = first.sent[0];

    first.die();
    await vi.advanceTimersByTimeAsync(100);
    harness.current.becomeReady();
    harness.current.say({ outcome: 'analysis', id: request?.id, model: modelOf('x'), highlight: [] });

    expect(await analysis).toBeInstanceOf(ComputeUnavailableError);
  });
});

describe('stopping the analysis process', () => {
  it('kills it, rejects what was pending, and never restarts', async () => {
    const harness = new Harness();
    harness.client.start();
    harness.current.becomeReady();
    const pending = rejectionOf(harness.client.analyze('tag=hz-a'));
    await vi.advanceTimersByTimeAsync(0);
    const running = harness.current;

    harness.client.stop();
    running.die();
    await vi.advanceTimersByTimeAsync(10_000);

    expect(await pending).toBeInstanceOf(ComputeUnavailableError);
    expect(running.killed).toBe(true);
    expect(harness.processes).toHaveLength(1);
    expect(harness.statuses).toEqual(['ready']);
  });

  it('cancels a restart that was waiting', async () => {
    const harness = new Harness();
    harness.client.start();
    harness.current.die();

    harness.client.stop();
    await vi.advanceTimersByTimeAsync(10_000);

    expect(harness.processes).toHaveLength(1);
  });

  it('ignores a ready notice that arrives after the stop', () => {
    const harness = new Harness();
    harness.client.start();
    const starting = harness.current;

    harness.client.stop();
    starting.becomeReady();

    expect(harness.statuses).toEqual([]);
  });
});
