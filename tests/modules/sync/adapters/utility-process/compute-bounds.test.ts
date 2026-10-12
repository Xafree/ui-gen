import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  ComputeClient,
  ComputeUnavailableError,
  type ComputeProcess,
} from '../../../../../src/modules/sync/adapters/utility-process/compute-client';
import {
  MAX_COMPUTE_TEXT_LENGTH,
  MAX_TSCONFIG_LENGTH,
  readComputeRequest,
  refusalOf,
  type ComputeRequest,
} from '../../../../../src/modules/sync/adapters/utility-process/compute-protocol';
import type { AnalyzerStatus } from '../../../../../src/modules/sync/ports/component-analyzer';

class SilentProcess implements ComputeProcess {
  readonly sent: ComputeRequest[] = [];
  killed = false;
  #message: (message: unknown) => void = () => undefined;

  send(request: ComputeRequest): void {
    this.sent.push(request);
  }

  onMessage(listener: (message: unknown) => void): void {
    this.#message = listener;
  }

  onExit(): void {
    return undefined;
  }

  kill(): void {
    this.killed = true;
  }

  becomeReady(): void {
    this.#message({ outcome: 'ready' });
  }
}

function setUp() {
  const processes: SilentProcess[] = [];
  const statuses: AnalyzerStatus[] = [];
  const client = new ComputeClient({
    spawn: () => {
      const process = new SilentProcess();
      processes.push(process);
      return process;
    },
    onStatus: (status) => {
      statuses.push(status);
    },
    requestTimeoutMs: 1_000,
  });
  client.start();
  const process = processes[0];
  if (process === undefined) throw new Error('No process was spawned');
  process.becomeReady();
  return { client, process, statuses };
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('a request out of the bounds of the analysis process', () => {
  it.each([
    ['a text', (client: ComputeClient) => client.analyze('x'.repeat(MAX_COMPUTE_TEXT_LENGTH + 1))],
    ['a model text', (client: ComputeClient) => client.readModel('x'.repeat(MAX_COMPUTE_TEXT_LENGTH + 1))],
    ['a source', (client: ComputeClient) => client.transpile('x'.repeat(MAX_COMPUTE_TEXT_LENGTH + 1), null, 'a.ts')],
    ['a tsconfig', (client: ComputeClient) => client.transpile('a', 'x'.repeat(MAX_TSCONFIG_LENGTH + 1), 'a.ts')],
  ])('rejects %s that is too large at once, without sending, killing or restarting', async (_label, call) => {
    const { client, process, statuses } = setUp();

    const rejection = await call(client).then(
      () => null,
      (error: unknown) => error,
    );

    expect(rejection).toBeInstanceOf(ComputeUnavailableError);
    expect(process.sent).toEqual([]);
    expect(process.killed).toBe(false);
    expect(statuses).toEqual(['ready']);
  });

  it('still sends a request of exactly the maximum size', async () => {
    const { client, process } = setUp();

    void client.analyze('x'.repeat(MAX_COMPUTE_TEXT_LENGTH)).catch(() => undefined);
    await vi.advanceTimersByTimeAsync(0);

    expect(process.sent).toHaveLength(1);
  });
});

describe('the refusal of a request the host cannot serve', () => {
  it.each([
    ['an oversized analysis text', { id: 7, operation: 'analyze', text: 'x'.repeat(MAX_COMPUTE_TEXT_LENGTH + 1) }],
    ['an unknown operation', { id: 7, operation: 'format', text: 'a' }],
    ['an oversized tsconfig', { id: 7, operation: 'transpile', text: 'a', tsconfig: 'x'.repeat(MAX_TSCONFIG_LENGTH + 1), fileName: 'a.ts' }],
    ['an empty file name', { id: 7, operation: 'transpile', text: 'a', tsconfig: null, fileName: '' }],
  ])('fails the id of %s', (_label, request) => {
    expect(readComputeRequest(request)).toBeNull();
    expect(refusalOf(request)).toEqual({ outcome: 'failed', id: 7 });
  });

  it.each([null, 'text', [], {}, { id: -1 }, { id: 1.5 }, { id: '7' }, { id: Number.MAX_SAFE_INTEGER + 1 }])(
    'has no id to answer in %j',
    (value) => {
      expect(refusalOf(value)).toBeNull();
    },
  );
});
